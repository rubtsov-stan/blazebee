//! Capacity of mounted filesystems via `statvfs(2)`.
//!
//! Pseudo filesystems (no backing device) and network filesystems are
//! skipped. `statvfs` can block indefinitely on a hung mount, so each call
//! runs on a blocking thread with a short deadline; a mount that does not
//! answer is remembered as stuck and skipped until its outstanding call
//! returns, so at most one thread is ever parked per stuck mount.

use std::{collections::HashMap, ffi::CString, io, mem::MaybeUninit, time::Duration};

use async_trait::async_trait;
use serde::Serialize;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use super::{
    procfs::{fields, unescape_mount_field, ProcFile},
    Collector, CollectorResult, Sample,
};

const PATH: &str = "/proc/self/mounts";

/// Deadline for a single `statvfs` call before the mount is treated as stuck.
pub const STATVFS_TIMEOUT: Duration = Duration::from_secs(2);

const SKIPPED_FS_TYPES: &[&str] = &[
    "autofs",
    "binfmt_misc",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "debugfs",
    "devpts",
    "devtmpfs",
    "efivarfs",
    "fusectl",
    "hugetlbfs",
    "mqueue",
    "nsfs",
    "overlay",
    "proc",
    "procfs",
    "pstore",
    "ramfs",
    "rpc_pipefs",
    "securityfs",
    "selinuxfs",
    "squashfs",
    "sysfs",
    "tmpfs",
    "tracefs",
    // network filesystems: slow or hanging statvfs is common
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "afs",
    "ceph",
    "glusterfs",
    "9p",
    "lustre",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FilesystemStats {
    pub device: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    /// Blocks in use, including space reserved for root.
    pub used_bytes: u64,
    /// Space available to unprivileged users.
    pub available_bytes: u64,
    pub total_inodes: u64,
    pub used_inodes: u64,
    pub available_inodes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilesystemBuffer {
    pub stats: Vec<FilesystemStats>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub device: String,
    pub mount_point: String,
    pub fs_type: String,
}

/// Parses `/proc/self/mounts` and keeps only mounts worth measuring. When a
/// mount point appears several times the last entry wins, matching what a
/// process sees at that path.
pub fn parse_mounts(content: &str) -> Vec<Mount> {
    let mut by_path: HashMap<String, Mount> = HashMap::new();
    let mut order = Vec::new();
    for line in content.lines() {
        let mut f = fields(line);
        let (Some(device), Some(mount_point), Some(fs_type)) = (f.next(), f.next(), f.next())
        else {
            continue;
        };
        if !device.starts_with('/')
            || SKIPPED_FS_TYPES.contains(&fs_type)
            || fs_type.starts_with("fuse")
        {
            continue;
        }
        let mount_point = unescape_mount_field(mount_point);
        if by_path
            .insert(
                mount_point.clone(),
                Mount {
                    device: unescape_mount_field(device),
                    mount_point: mount_point.clone(),
                    fs_type: fs_type.to_string(),
                },
            )
            .is_none()
        {
            order.push(mount_point);
        }
    }
    order
        .into_iter()
        .filter_map(|p| by_path.remove(&p))
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct Usage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub total_inodes: u64,
    pub used_inodes: u64,
    pub available_inodes: u64,
}

// Field widths differ between libc targets (u32 on 32-bit and macOS, u64 on
// 64-bit Linux); the conversions are identity on some targets and widening on others.
#[allow(clippy::useless_conversion)]
fn statvfs(mount_point: &str) -> io::Result<Usage> {
    let path = CString::new(mount_point)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
    let mut stat = MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is a valid NUL-terminated string and `stat` points to
    // writable memory of the correct type; statvfs fully initializes it on
    // success, which is the only case where it is read.
    let rc = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: rc == 0 guarantees the struct was initialized by the kernel.
    let stat = unsafe { stat.assume_init() };
    let frsize = u64::from(stat.f_frsize);
    let blocks = u64::from(stat.f_blocks);
    let bfree = u64::from(stat.f_bfree);
    let bavail = u64::from(stat.f_bavail);
    let files = u64::from(stat.f_files);
    let ffree = u64::from(stat.f_ffree);
    let favail = u64::from(stat.f_favail);
    Ok(Usage {
        total_bytes: blocks.saturating_mul(frsize),
        used_bytes: blocks.saturating_sub(bfree).saturating_mul(frsize),
        available_bytes: bavail.saturating_mul(frsize),
        total_inodes: files,
        used_inodes: files.saturating_sub(ffree),
        available_inodes: favail,
    })
}

pub struct FilesystemCollector {
    mounts: ProcFile,
    /// `statvfs` calls that exceeded the deadline, keyed by mount point.
    stuck: HashMap<String, JoinHandle<io::Result<Usage>>>,
}

impl Default for FilesystemCollector {
    fn default() -> Self {
        Self {
            mounts: ProcFile::new(PATH),
            stuck: HashMap::new(),
        }
    }
}

impl FilesystemCollector {
    async fn usage(&mut self, mount_point: &str) -> Option<io::Result<Usage>> {
        if let Some(handle) = self.stuck.get_mut(mount_point) {
            if !handle.is_finished() {
                debug!(mount_point, "statvfs still blocked; skipping");
                return None;
            }
            let handle = self.stuck.remove(mount_point).expect("present");
            return handle.await.ok();
        }
        let path = mount_point.to_string();
        let mut handle = tokio::task::spawn_blocking(move || statvfs(&path));
        match tokio::time::timeout(STATVFS_TIMEOUT, &mut handle).await {
            Ok(result) => result.ok(),
            Err(_) => {
                warn!(mount_point, timeout = ?STATVFS_TIMEOUT, "statvfs did not return; marking mount as stuck");
                self.stuck.insert(mount_point.to_string(), handle);
                None
            }
        }
    }
}

#[async_trait]
impl Collector for FilesystemCollector {
    async fn collect(&mut self) -> CollectorResult<Sample> {
        let mounts = parse_mounts(self.mounts.read()?);
        let mut stats = Vec::with_capacity(mounts.len());
        for mount in mounts {
            match self.usage(&mount.mount_point).await {
                Some(Ok(u)) => stats.push(FilesystemStats {
                    device: mount.device,
                    mount_point: mount.mount_point,
                    fs_type: mount.fs_type,
                    total_bytes: u.total_bytes,
                    used_bytes: u.used_bytes,
                    available_bytes: u.available_bytes,
                    total_inodes: u.total_inodes,
                    used_inodes: u.used_inodes,
                    available_inodes: u.available_inodes,
                }),
                Some(Err(err)) => {
                    debug!(mount_point = %mount.mount_point, %err, "statvfs failed; skipping")
                }
                None => {}
            }
        }
        Ok(Box::new(FilesystemBuffer { stats }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/proc/mounts"
    ));

    #[test]
    fn filters_pseudo_and_network_filesystems() {
        let content = "\
proc /proc proc rw 0 0
/dev/sda1 / ext4 rw,relatime 0 0
tmpfs /run tmpfs rw 0 0
/dev/sdb1 /mnt/my\\040disk xfs rw 0 0
server:/export /mnt/nfs nfs4 rw 0 0
//srv/share /mnt/smb cifs rw 0 0
/dev/fuse /mnt/fuse fuse.sshfs rw 0 0
overlay / overlay rw 0 0
/dev/sda2 / btrfs rw 0 0
";
        let mounts = parse_mounts(content);
        let paths: Vec<&str> = mounts.iter().map(|m| m.mount_point.as_str()).collect();
        assert_eq!(paths, ["/", "/mnt/my disk"]);
        assert_eq!(mounts[0].device, "/dev/sda2", "last mount at a path wins");
        assert_eq!(mounts[0].fs_type, "btrfs");
    }

    #[test]
    fn real_container_mounts_parse() {
        let mounts = parse_mounts(FIXTURE);
        assert!(mounts.iter().all(|m| m.device.starts_with('/')));
    }

    #[test]
    fn statvfs_of_root_works() {
        let u = statvfs("/").unwrap();
        assert!(u.total_bytes > 0);
        assert!(u.used_bytes <= u.total_bytes);
        assert!(u.available_bytes <= u.total_bytes);
        assert!(statvfs("/definitely/not/here").is_err());
    }

    #[tokio::test]
    async fn collector_runs_on_this_host() {
        let mut c = FilesystemCollector::default();
        let sample = c.collect().await;
        // On Linux /proc/self/mounts exists; elsewhere the read error is expected.
        if cfg!(target_os = "linux") {
            sample.unwrap();
        }
    }
}
