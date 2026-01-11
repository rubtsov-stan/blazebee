/// ARP (Address Resolution Protocol) collector module.
/// Provides functionality for collecting ARP table statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-arp` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
))]
pub mod arp;

/// System load average collector module.
/// Collects 1, 5, and 15-minute load averages.
///
/// Available when:
/// - `minimal`, `standard`, or `large` features are enabled on Linux, OR
/// - `collector-loadavg` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
))]
pub mod avg;

/// Connection tracking collector module.
/// Provides Netfilter conntrack statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-conntrack` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
))]
pub mod conntrack;

/// CPU statistics collector module.
/// Collects CPU usage, frequency, and core statistics.
///
/// Available when:
/// - `minimal`, `standard`, or `large` features are enabled on Linux, OR
/// - `collector-cpu` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
))]
pub mod cpu;

/// Disk and block device statistics collector module.
/// Collects disk I/O, throughput, and utilization metrics.
///
/// Available when:
/// - `minimal`, `standard`, or `large` features are enabled on Linux, OR
/// - `collector-disk` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
pub mod disk;

/// Entropy pool statistics collector module.
/// Collects available entropy in the system's random number generator.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-entropy` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
))]
pub mod entropy;

/// Error types and handling utilities.
/// Common error types used across all collectors.
pub mod error;

/// File descriptor statistics collector module.
/// Tracks open file descriptors and limits.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-filedfd` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
))]
pub mod filefd;

/// Hardware monitoring sensor collector module.
/// Collects temperature, voltage, fan speed, and other hardware sensor data.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-hwmon` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
pub mod hwmon;

/// IP Virtual Server statistics collector module.
/// Collects Linux Virtual Server (LVS) load balancing statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-ipvs` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
))]
pub mod ipvs;

/// Network socket statistics collector module.
/// Collects detailed socket and protocol statistics.
///
/// Available when:
/// - `standard` or `large` features are enabled on Linux, OR
/// - `collector-netstat` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
pub mod netstat;

/// Network interface statistics collector module.
/// Collects per-interface network traffic and error statistics.
///
/// Available when:
/// - `standard` or `large` features are enabled on Linux, OR
/// - `collector-network` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
pub mod network;

/// Pressure Stall Information (PSI) collector module.
/// Collects CPU, memory, and I/O pressure metrics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-pressure` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
pub mod pressure;

/// Process and task statistics collector module.
/// Collects system-wide process metrics and per-process statistics.
///
/// Available when:
/// - `minimal`, `standard`, or `large` features are enabled on Linux, OR
/// - `collector-processes` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
))]
pub mod processes;

/// Memory and RAM statistics collector module.
/// Collects physical memory, swap, and cache statistics.
///
/// Available when:
/// - `minimal`, `standard`, or `large` features are enabled on Linux, OR
/// - `collector-ram` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
))]
pub mod ram;

/// Collector registry and management module.
/// Central registry for managing and orchestrating all available collectors.
pub mod registry;

/// CPU scheduler statistics collector module.
/// Collects detailed CPU scheduler metrics and latency statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-schedstat` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
pub mod schedstat;

/// Socket type statistics collector module.
/// Collects statistics by socket type and protocol.
///
/// Available when:
/// - `standard` or `large` features are enabled on Linux, OR
/// - `collector-sockstat` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
pub mod sockstat;

/// Software network processing statistics collector module.
/// Collects per-CPU software network processing statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-softnet` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
pub mod softnet;

/// System-wide kernel statistics collector module.
/// Collects general system statistics from /proc/stat.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-stat` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
))]
pub mod stat;

/// Thermal zone statistics collector module.
/// Collects temperature and cooling device statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-thermal` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
pub mod thermal;

/// Core traits and interfaces.
/// Defines the common interfaces for all collectors and data producers.
pub mod traits;

/// Common types and result definitions.
/// Shared types, enums, and result types used throughout the library.
pub mod types;

/// System uptime and load collector module.
/// Collects system uptime and boot time information.
///
/// Available when:
/// - `minimal`, `standard`, or `large` features are enabled on Linux, OR
/// - `collector-uptime` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
))]
pub mod uptime;

/// Virtual memory statistics collector module.
/// Collects detailed virtual memory and paging statistics.
///
/// Available when:
/// - `standard` or `large` features are enabled on Linux, OR
/// - `collector-vmstat` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
pub mod vmstat;

/// Filesystem statistics collector module.
/// Collects mounted filesystem usage and capacity statistics.
///
/// Available when:
/// - `standard` or `large` features are enabled on Linux, OR
/// - `collector-filesystem` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
))]
pub mod filesys;

/// Systemd service and unit statistics collector module.
/// Collects Systemd unit status and resource usage statistics.
///
/// Available when:
/// - `standard` or `large` features are enabled on Linux, OR
/// - `collector-systemd` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
))]
pub mod sysmd;

/// Network Time Protocol statistics collector module.
/// Collects NTP synchronization and clock statistics.
///
/// Available when:
/// - `standard` or `large` features are enabled on Linux, OR
/// - `collector-ntp` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
))]
pub mod ntp;

/// Power management statistics collector module.
/// Collects CPU power states and energy consumption statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-power` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
pub mod powerstat;

/// Error Detection and Correction (EDAC) collector module.
/// Collects memory error correction and reliability statistics.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-edac` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
pub mod edac;

/// Software RAID (MD) statistics collector module.
/// Collects Linux software RAID device statistics and health information.
///
/// Available when:
/// - `large` feature is enabled on Linux, OR
/// - `collector-mdraid` feature is explicitly enabled on Linux
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
))]
pub mod mdraid;

// ----------------------------------------------------------------------------
// Re-exports for public API
// ----------------------------------------------------------------------------

/// ARP collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-arp", target_os = "linux")
))]
pub use arp::{ArpBuffer, ArpCollector, ArpEntry};
/// Load average collector implementation.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-loadavg", target_os = "linux")
))]
pub use avg::LoadAverageCollector;
/// Connection tracking collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-conntrack", target_os = "linux")
))]
pub use conntrack::{ConntrackCollector, ConntrackStats};
/// CPU statistics collector types and implementations.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-cpu", target_os = "linux")
))]
pub use cpu::{CpuBuffer, CpuCollector, CpuStats};
/// Disk and block device statistics collector types and implementations.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-disk", target_os = "linux")
))]
pub use disk::{DiskBuffer, DiskStats, DiskStatsCollector};
/// EDAC (Error Detection and Correction) collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-edac", target_os = "linux")
))]
pub use edac::{EdacBuffer, EdacCollector, EdacStats};
/// Entropy pool statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
))]
pub use entropy::{EntropyCollector, EntropyStats};
/// File descriptor statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filedfd", target_os = "linux")
))]
pub use filefd::{FileFdStats, FilefdCollector};
/// Filesystem statistics collector types and implementations.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-filesystem", target_os = "linux")
))]
pub use filesys::{FilesystemBuffer, FilesystemCollector, FilesystemStats};
/// Hardware monitoring sensor collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-hwmon", target_os = "linux")
))]
pub use hwmon::{HwmonBuffer, HwmonCollector, HwmonSensor};
/// IP Virtual Server statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ipvs", target_os = "linux")
))]
pub use ipvs::{IpvsCollector, IpvsStats};
/// Software RAID (MD) statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-mdraid", target_os = "linux")
))]
pub use mdraid::{MdRaidBuffer, MdRaidCollector, MdRaidStats};
/// Network socket statistics collector types and implementations.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-netstat", target_os = "linux")
))]
pub use netstat::{NetstatBuffer, NetstatCollector, NetstatEntry};
/// Network interface statistics collector types and implementations.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-network", target_os = "linux")
))]
pub use network::{NetworkBuffer, NetworkCollector, NetworkStats};
/// NTP synchronization statistics collector types and implementations.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ntp", target_os = "linux")
))]
pub use ntp::{NtpBuffer, NtpCollector, NtpStats};
/// Power management statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-power", target_os = "linux")
))]
pub use powerstat::{PowerBuffer, PowerCollector, PowerStats};
/// Pressure Stall Information (PSI) collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-pressure", target_os = "linux")
))]
pub use pressure::{PressureBuffer, PressureCollector, PressureStats};
/// Process statistics collector types and implementations.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-processes", target_os = "linux")
))]
pub use processes::{ProcessCollector, ProcessStats};
/// Memory statistics collector types and implementations.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-ram", target_os = "linux")
))]
pub use ram::{MemoryCollector, MemoryStats};
/// Collector registry for managing all available collectors.
pub use registry::CollectorRegistry;
/// CPU scheduler statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-schedstat", target_os = "linux")
))]
pub use schedstat::{SchedstatBuffer, SchedstatCollector, SchedstatCpu};
/// Socket type statistics collector types and implementations.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-sockstat", target_os = "linux")
))]
pub use sockstat::{SockstatBuffer, SockstatCollector, SockstatEntry};
/// Software network processing statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-softnet", target_os = "linux")
))]
pub use softnet::{SoftnetBuffer, SoftnetCollector, SoftnetStats};
/// System-wide kernel statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-stat", target_os = "linux")
))]
pub use stat::{StatCollector, StatGlobalStats};
/// Systemd service statistics collector types and implementations.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-systemd", target_os = "linux")
))]
pub use sysmd::{SystemdBuffer, SystemdCollector, SystemdUnitStats};
/// Thermal zone statistics collector types and implementations.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-thermal", target_os = "linux")
))]
pub use thermal::{ThermalBuffer, ThermalZoneCollector, ThermalZoneInfo};
/// Core trait for data producers and collectors.
pub use traits::DataProducer;
/// Common result type for collector operations.
pub use types::CollectorResult;
/// System uptime statistics collector types and implementations.
#[cfg(any(
    all(feature = "minimal", target_os = "linux"),
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-uptime", target_os = "linux")
))]
pub use uptime::{UptimeCollector, UptimeInfo};
/// Virtual memory statistics collector types and implementations.
#[cfg(any(
    all(feature = "standard", target_os = "linux"),
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-vmstat", target_os = "linux")
))]
pub use vmstat::{VmstatBuffer, VmstatCollector, VmstatEntry};
