//! Kernel clock discipline state via `adjtimex(2)`.
//!
//! This reports what the kernel knows about its own clock: whether a time
//! daemon (chrony, ntpd, systemd-timesyncd, ptp4l) currently disciplines it,
//! the estimated offset and error bounds, and the frequency correction. It
//! works with any daemon and needs no privileges or subprocess.

use serde::Serialize;

use super::{sample, CollectFuture, Collector, CollectorError, CollectorResult};

/// `STA_NANO`: offset and precision are in nanoseconds instead of microseconds.
const STA_NANO: i32 = 0x2000;
/// `STA_UNSYNC`: the clock is not synchronized to a reference.
const STA_UNSYNC: i32 = 0x0040;
/// Return value of `adjtimex` when the clock is in the error state.
const TIME_ERROR: i32 = 5;
/// `freq` is in parts per million with 16 fractional bits.
const FREQ_SCALE: f64 = 65536.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NtpStats {
    /// `true` when a daemon disciplines the clock and the kernel is not in the error state.
    pub synchronized: bool,
    /// Kernel clock state: 0 ok, 1 insert leap second, 2 delete leap second,
    /// 3 leap second in progress, 4 leap second occurred, 5 clock not synchronized.
    pub clock_state: i32,
    /// Raw `STA_*` status bits.
    pub status: i32,
    /// Current offset between the system clock and the reference.
    pub offset_ms: f64,
    /// Maximum error bound maintained by the kernel.
    pub max_error_ms: f64,
    pub estimated_error_ms: f64,
    /// Frequency correction applied to the clock.
    pub frequency_ppm: f64,
    /// TAI minus UTC in seconds, if the daemon has set it.
    pub tai_offset: i32,
}

#[derive(Default)]
pub struct NtpCollector;

pub fn from_timex(
    state: i32,
    status: i32,
    offset: i64,
    maxerror: i64,
    esterror: i64,
    freq: i64,
    tai: i32,
) -> NtpStats {
    let offset_us = if status & STA_NANO != 0 {
        offset as f64 / 1000.0
    } else {
        offset as f64
    };
    NtpStats {
        synchronized: state != TIME_ERROR && status & STA_UNSYNC == 0,
        clock_state: state,
        status,
        offset_ms: offset_us / 1000.0,
        max_error_ms: maxerror as f64 / 1000.0,
        estimated_error_ms: esterror as f64 / 1000.0,
        frequency_ppm: freq as f64 / FREQ_SCALE,
        tai_offset: tai,
    }
}

// `c_long` fields are i32 on 32-bit targets and i64 on 64-bit targets.
#[allow(clippy::useless_conversion)]
fn read_timex() -> CollectorResult<NtpStats> {
    // SAFETY: `timex` is a plain C struct for which all-zero is a valid value;
    // `modes == 0` makes adjtimex a read-only query that needs no privileges.
    let mut buf: libc::timex = unsafe { std::mem::zeroed() };
    // SAFETY: `buf` is a valid, exclusively borrowed timex struct.
    let state = unsafe { libc::adjtimex(&mut buf) };
    if state < 0 {
        return Err(CollectorError::Command {
            command: "adjtimex",
            reason: std::io::Error::last_os_error().to_string(),
        });
    }
    Ok(from_timex(
        state,
        buf.status,
        i64::from(buf.offset),
        i64::from(buf.maxerror),
        i64::from(buf.esterror),
        i64::from(buf.freq),
        buf.tai,
    ))
}

impl Collector for NtpCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(read_timex()?)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_units_and_sync_flags() {
        let s = from_timex(0, STA_NANO, 1_500_000, 2000, 500, 3 * 65536, 37);
        assert!(s.synchronized);
        assert_eq!(s.offset_ms, 1.5);
        assert_eq!(s.max_error_ms, 2.0);
        assert_eq!(s.estimated_error_ms, 0.5);
        assert_eq!(s.frequency_ppm, 3.0);
        assert_eq!(s.tai_offset, 37);

        let s = from_timex(TIME_ERROR, STA_UNSYNC, -250, 0, 0, 0, 0);
        assert!(!s.synchronized);
        assert_eq!(s.offset_ms, -0.25, "microsecond offset without STA_NANO");
    }

    #[test]
    fn reads_live_kernel_state() {
        let s = read_timex().unwrap();
        assert!((0..=5).contains(&s.clock_state));
    }
}
