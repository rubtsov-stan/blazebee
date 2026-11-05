//! Small helpers for reading and tokenizing `/proc` and `/sys` files.
//!
//! Reads are synchronous: these files are served from kernel memory and
//! complete in microseconds, so hopping to a blocking thread would cost more
//! than the read itself. Collectors keep a [`ProcFile`] per path so the read
//! buffer is allocated once and reused on every cycle.

use std::{
    fs::File,
    io::Read,
    path::Path,
    str::{FromStr, SplitAsciiWhitespace},
};

use super::{error::CollectorError, CollectorResult};

/// A `/proc` or `/sys` file with a reusable read buffer.
pub struct ProcFile {
    path: &'static str,
    buf: String,
}

impl ProcFile {
    pub const fn new(path: &'static str) -> Self {
        Self {
            path,
            buf: String::new(),
        }
    }

    pub fn path(&self) -> &'static str {
        self.path
    }

    /// Reads the whole file into the internal buffer and returns it.
    pub fn read(&mut self) -> CollectorResult<&str> {
        self.buf.clear();
        read_into(Path::new(self.path), &mut self.buf)
            .map_err(|e| CollectorError::read(self.path, e))?;
        Ok(&self.buf)
    }
}

/// Reads `path` into `buf` (appending), reusing the buffer's capacity.
///
/// `/proc` files report a zero size, so the standard `read_to_string` first
/// issues `statx` and `lseek` to size its buffer; this loop skips both and
/// needs exactly one `read` per chunk plus one for EOF. The buffer grows to
/// the file size on the first call and is not reallocated afterwards.
pub fn read_into(path: &Path, buf: &mut String) -> std::io::Result<()> {
    const CHUNK: usize = 8192;
    let mut file = File::open(path)?;
    let start = buf.len();
    let mut bytes = std::mem::take(buf).into_bytes();
    let result = (|| loop {
        let len = bytes.len();
        let want = (bytes.capacity() - len).max(CHUNK);
        bytes.resize(len + want, 0);
        let n = file.read(&mut bytes[len..])?;
        bytes.truncate(len + n);
        if n == 0 {
            return Ok(());
        }
    })();
    match String::from_utf8(bytes) {
        Ok(text) => {
            *buf = text;
            result
        }
        Err(err) => {
            let mut text = err.into_bytes();
            text.truncate(start);
            // The prefix was valid UTF-8 (it came from `buf`), so this cannot fail.
            *buf = String::from_utf8(text).unwrap_or_default();
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "file is not valid UTF-8",
            ))
        }
    }
}

/// Reads a small file into a fresh `String`, trimmed.
pub fn read_trimmed(path: &Path) -> std::io::Result<String> {
    let mut s = String::with_capacity(64);
    read_into(path, &mut s)?;
    let end = s.trim_end().len();
    s.truncate(end);
    Ok(s)
}

/// Reads a file that contains a single integer, e.g. a sysctl.
pub fn read_number<T: FromStr>(path: &Path, what: &'static str) -> CollectorResult<T> {
    let text =
        read_trimmed(path).map_err(|e| CollectorError::read(path.display().to_string(), e))?;
    parse(&text, path.display().to_string(), what)
}

/// Parses `token` as `T`, attributing failures to `what` in `path`.
pub fn parse<T: FromStr>(
    token: &str,
    path: impl Into<String>,
    what: &'static str,
) -> CollectorResult<T> {
    token
        .parse()
        .map_err(|_| CollectorError::parse(path, what, format!("invalid value '{token}'")))
}

/// Parses the next whitespace-separated field, failing if it is absent or malformed.
pub fn next_field<T: FromStr>(
    fields: &mut SplitAsciiWhitespace<'_>,
    path: &'static str,
    what: &'static str,
) -> CollectorResult<T> {
    match fields.next() {
        Some(token) => parse(token, path, what),
        None => Err(CollectorError::missing(path, what)),
    }
}

/// Splits a `/proc` line into fields; convenience wrapper that documents intent.
#[inline]
pub fn fields(line: &str) -> SplitAsciiWhitespace<'_> {
    line.split_ascii_whitespace()
}

/// Decodes the `\ooo` octal escapes used in `/proc/mounts` for whitespace
/// and backslashes in mount points and device names.
pub fn unescape_mount_field(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let value =
                (bytes[i + 1] - b'0') * 64 + (bytes[i + 2] - b'0') * 8 + (bytes[i + 3] - b'0');
            out.push(value);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_field_reports_missing_and_invalid() {
        let mut f = fields("12 x");
        assert_eq!(next_field::<u64>(&mut f, "/p", "a").unwrap(), 12);
        assert!(matches!(
            next_field::<u64>(&mut f, "/p", "b"),
            Err(CollectorError::Parse { what: "b", .. })
        ));
        assert!(matches!(
            next_field::<u64>(&mut f, "/p", "c"),
            Err(CollectorError::Missing { what: "c", .. })
        ));
    }

    #[test]
    fn unescapes_mount_fields() {
        assert_eq!(unescape_mount_field("/mnt/my\\040disk"), "/mnt/my disk");
        assert_eq!(unescape_mount_field("/plain"), "/plain");
        assert_eq!(unescape_mount_field("a\\134b"), "a\\b");
        assert_eq!(unescape_mount_field("trailing\\04"), "trailing\\04");
    }

    #[test]
    fn read_into_handles_large_files_and_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big");
        let content: String = (0..5000).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&big, &content).unwrap();
        let mut buf = String::from("prefix:");
        read_into(&big, &mut buf).unwrap();
        assert_eq!(buf, format!("prefix:{content}"));

        let bad = dir.path().join("bad");
        std::fs::write(&bad, [b'o', b'k', 0xff, 0xfe]).unwrap();
        let mut buf = String::from("keep");
        let err = read_into(&bad, &mut buf).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(buf, "keep");
    }

    #[test]
    fn procfile_reads_real_file_and_reuses_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, "hello\n").unwrap();
        let leaked: &'static str = Box::leak(path.to_string_lossy().into_owned().into_boxed_str());
        let mut file = ProcFile::new(leaked);
        assert_eq!(file.read().unwrap(), "hello\n");
        let cap = file.buf.capacity();
        assert_eq!(file.read().unwrap(), "hello\n");
        assert_eq!(file.buf.capacity(), cap);
        assert!(matches!(
            ProcFile::new("/nonexistent/x").read(),
            Err(CollectorError::Read { .. })
        ));
    }
}
