---
title: Advanced Metrics
description: In-depth metrics for specialized monitoring and diagnostics.
---

Advanced metrics in BlazeBee are designed for environments where basic system visibility is insufficient. These collectors expose low-level kernel, hardware, and service-specific signals that are critical for troubleshooting performance degradation, capacity issues, and hardware anomalies.

Advanced collectors are **disabled by default** and must be explicitly enabled at build time and in configuration.

### Purpose

This page describes:
- What advanced collectors are available
- What subsystems they observe
- When and why they should be used
- How they integrate into the standard metrics pipeline

These metrics are intended for experienced operators and diagnostic workflows.

### Available Advanced Collectors

#### Thermal

**Collector name:** `thermal`

- Reads temperature data from thermal zones
- Sources include `/sys/class/thermal`
- Reports per-sensor temperatures in degrees

**Use cases:**
- Detect overheating CPUs or SoCs
- Monitor passive cooling efficiency
- Prevent thermal throttling

---

#### Power

**Collector name:** `power`

- Exposes battery and power-supply information
- Reads from `/sys/class/power_supply`
- Includes charge level, voltage, current, and status

**Use cases:**
- Edge devices
- Battery-backed systems
- Power consumption diagnostics

---

#### Pressure (PSI)

**Collector name:** `pressure`

- Reports Linux PSI (Pressure Stall Information)
- Covers CPU, memory, and IO pressure
- Indicates how often tasks are stalled due to resource contention

**Use cases:**
- Diagnosing latency spikes
- Capacity planning
- Identifying hidden resource saturation

---

#### Systemd

**Collector name:** `systemd`

- Queries systemd unit states
- Tracks service health and activation status
- Requires systemd-based Linux distributions

**Use cases:**
- Service availability monitoring
- Detecting failed or flapping units
- Infrastructure observability without agents

---

#### NTP

**Collector name:** `ntp`

- Reports clock offset and synchronization state
- Uses system time sources
- Indicates drift relative to reference clocks

**Use cases:**
- Distributed systems
- Time-sensitive workloads
- Debugging clock skew issues

---

#### Hardware Monitoring (HWMON)

**Collector name:** `hwmon`

- Reads hardware sensors via `/sys/class/hwmon`
- Includes voltages, fan speeds, temperatures

**Use cases:**
- Bare-metal monitoring
- Detecting failing components
- Environmental diagnostics

---

#### Network Statistics (Extended)

**Collector names:** `arp`, `netstat`, `conntrack`

- ARP cache statistics
- Kernel network counters
- Connection tracking table usage

**Use cases:**
- Network debugging
- Detecting connection leaks
- Firewall and NAT diagnostics

---

### File Descriptors

**Collector name:** `filefd`

- Reports file descriptor limits and usage
- Reads from `/proc/sys/fs` and process tables

**Use cases:**
- Preventing FD exhaustion
- Debugging connection-heavy services
- Capacity tuning

---

### Additional Advanced Collectors

Depending on build features, BlazeBee may also support:

- `mdraid` — RAID array health
- `edac` — memory error detection
- `schedstat` — scheduler statistics
- `entropy` — kernel entropy pool
- `filesystem` — detailed FS stats
- `powercap` — RAPL power limits

### Enabling Advanced Metrics

### Build-Time Requirement

Advanced collectors are included only when built with the `large` feature set or explicit collector features.

```bash
cargo build --release --features "blazebee-mqtt-v3 large"
````

### Runtime Configuration

Each advanced collector must be enabled in `config.toml`:

```toml
[[metrics.collectors.enabled]]
name = "pressure"
[metrics.collectors.enabled.metadata]
topic  = "metrics/pressure"
qos    = 1
retain = true
```

* One block per collector
* Topic structure is fully user-defined
* QoS and retain flags apply per collector

### Operational Considerations

* Some collectors require elevated privileges
* Linux-only functionality
* Increased I/O and parsing overhead
* Recommended for diagnostic or targeted deployments

### Summary

Advanced metrics provide deep visibility into system internals. They are powerful but should be enabled selectively. When used appropriately, they enable early detection of failures, performance bottlenecks, and hardware issues that are invisible to standard monitoring.
