//! Metric collectors and the static registry that maps configuration names
//! to implementations.
//!
//! A collector reads one kernel interface and returns a serializable sample.
//! Collectors are stateful (`&mut self`) so they can reuse read buffers and
//! remember facts such as stuck mounts; the scheduler drives them one at a
//! time, so no synchronization is needed. Every collector sits behind a
//! `collector-*` Cargo feature; the `minimal`, `standard` and `large` features
//! are presets that enable groups of them.

use std::{future::Future, pin::Pin};

pub mod error;
pub mod procfs;

#[cfg(feature = "collector-arp")]
pub mod arp;
#[cfg(feature = "collector-loadavg")]
pub mod avg;
#[cfg(feature = "collector-conntrack")]
pub mod conntrack;
#[cfg(feature = "collector-cpu")]
pub mod cpu;
#[cfg(feature = "collector-disk")]
pub mod disk;
#[cfg(feature = "collector-edac")]
pub mod edac;
#[cfg(feature = "collector-entropy")]
pub mod entropy;
#[cfg(feature = "collector-filefd")]
pub mod filefd;
#[cfg(feature = "collector-filesystem")]
pub mod filesys;
#[cfg(feature = "collector-hwmon")]
pub mod hwmon;
#[cfg(feature = "collector-ipvs")]
pub mod ipvs;
#[cfg(feature = "collector-mdraid")]
pub mod mdraid;
#[cfg(feature = "collector-netstat")]
pub mod netstat;
#[cfg(feature = "collector-network")]
pub mod network;
#[cfg(all(feature = "collector-ntp", target_os = "linux"))]
pub mod ntp;
#[cfg(feature = "collector-power")]
pub mod powerstat;
#[cfg(feature = "collector-pressure")]
pub mod pressure;
#[cfg(feature = "collector-processes")]
pub mod processes;
#[cfg(feature = "collector-ram")]
pub mod ram;
#[cfg(feature = "collector-schedstat")]
pub mod schedstat;
#[cfg(feature = "collector-sockstat")]
pub mod sockstat;
#[cfg(feature = "collector-softnet")]
pub mod softnet;
#[cfg(feature = "collector-stat")]
pub mod stat;
#[cfg(feature = "collector-systemd")]
pub mod sysmd;
#[cfg(feature = "collector-thermal")]
pub mod thermal;
#[cfg(feature = "collector-uptime")]
pub mod uptime;
#[cfg(feature = "collector-vmstat")]
pub mod vmstat;

pub use error::CollectorError;

pub type CollectorResult<T> = Result<T, CollectorError>;

/// One sample, ready to be serialized by the transport.
pub type Sample = Box<dyn erased_serde::Serialize + Send>;

/// Future returned by [`Collector::collect`].
pub type CollectFuture<'a> = Pin<Box<dyn Future<Output = CollectorResult<Sample>> + Send + 'a>>;

/// Boxes a collector output as a [`Sample`]; keeps `collect` bodies short.
pub fn sample<T: erased_serde::Serialize + Send + 'static>(value: T) -> Sample {
    Box::new(value)
}

pub trait Collector: Send {
    /// Takes one sample. Called at most once per interval from a single task.
    fn collect(&mut self) -> CollectFuture<'_>;
}

/// Registry entry: the configuration name and a constructor.
pub struct Descriptor {
    pub name: &'static str,
    pub build: fn() -> Box<dyn Collector>,
}

#[allow(unused_macros)]
macro_rules! descriptor {
    ($name:literal, $ty:ty) => {
        Descriptor {
            name: $name,
            build: || Box::new(<$ty>::default()),
        }
    };
}

/// All collectors compiled into this binary.
pub static REGISTRY: &[Descriptor] = &[
    #[cfg(feature = "collector-arp")]
    descriptor!("arp", arp::ArpCollector),
    #[cfg(feature = "collector-conntrack")]
    descriptor!("conntrack", conntrack::ConntrackCollector),
    #[cfg(feature = "collector-cpu")]
    descriptor!("cpu", cpu::CpuCollector),
    #[cfg(feature = "collector-disk")]
    descriptor!("disk", disk::DiskCollector),
    #[cfg(feature = "collector-edac")]
    descriptor!("edac", edac::EdacCollector),
    #[cfg(feature = "collector-entropy")]
    descriptor!("entropy", entropy::EntropyCollector),
    #[cfg(feature = "collector-filefd")]
    descriptor!("filefd", filefd::FileFdCollector),
    #[cfg(feature = "collector-filesystem")]
    descriptor!("filesystem", filesys::FilesystemCollector),
    #[cfg(feature = "collector-hwmon")]
    descriptor!("hwmon", hwmon::HwmonCollector),
    #[cfg(feature = "collector-ipvs")]
    descriptor!("ipvs", ipvs::IpvsCollector),
    #[cfg(feature = "collector-loadavg")]
    descriptor!("load_average", avg::LoadAverageCollector),
    #[cfg(feature = "collector-mdraid")]
    descriptor!("mdraid", mdraid::MdRaidCollector),
    #[cfg(feature = "collector-netstat")]
    descriptor!("netstat", netstat::NetstatCollector),
    #[cfg(feature = "collector-network")]
    descriptor!("network", network::NetworkCollector),
    #[cfg(all(feature = "collector-ntp", target_os = "linux"))]
    descriptor!("ntp", ntp::NtpCollector),
    #[cfg(feature = "collector-power")]
    descriptor!("power", powerstat::PowerCollector),
    #[cfg(feature = "collector-pressure")]
    descriptor!("pressure", pressure::PressureCollector),
    #[cfg(feature = "collector-processes")]
    descriptor!("processes", processes::ProcessCollector),
    #[cfg(feature = "collector-ram")]
    descriptor!("ram", ram::MemoryCollector),
    #[cfg(feature = "collector-schedstat")]
    descriptor!("schedstat", schedstat::SchedstatCollector),
    #[cfg(feature = "collector-sockstat")]
    descriptor!("sockstat", sockstat::SockstatCollector),
    #[cfg(feature = "collector-softnet")]
    descriptor!("softnet", softnet::SoftnetCollector),
    #[cfg(feature = "collector-stat")]
    descriptor!("stat", stat::StatCollector),
    #[cfg(feature = "collector-systemd")]
    descriptor!("systemd", sysmd::SystemdCollector),
    #[cfg(feature = "collector-thermal")]
    descriptor!("thermal", thermal::ThermalCollector),
    #[cfg(feature = "collector-uptime")]
    descriptor!("uptime", uptime::UptimeCollector),
    #[cfg(feature = "collector-vmstat")]
    descriptor!("vmstat", vmstat::VmstatCollector),
];

pub fn find(name: &str) -> Option<&'static Descriptor> {
    REGISTRY.iter().find(|d| d.name == name)
}

pub fn names() -> impl Iterator<Item = &'static str> {
    REGISTRY.iter().map(|d| d.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_unique_and_lookup_works() {
        let mut names: Vec<&str> = names().collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate collector names");
        for name in names {
            let d = find(name).unwrap();
            assert_eq!(d.name, name);
            let _collector = (d.build)();
        }
        assert!(find("does-not-exist").is_none());
    }
}
