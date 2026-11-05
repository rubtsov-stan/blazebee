---
title: System Metrics
description: Collectors included in the minimal and standard builds, with their sources and fields.
---

Every collector publishes one message per cycle containing raw kernel
values. Counters are cumulative since boot; compute rates on the consumer
side from two consecutive samples. Field names below are the JSON keys.

## Minimal build

### `cpu` — `/proc/stat`

`{"stats": [...]}` with one entry for the aggregate `cpu` line and one per
logical CPU. Values are clock ticks (`USER_HZ`, usually 100 per second).

`cpu`, `user`, `nice`, `system`, `idle`, `iowait`, `irq`, `softirq`,
`steal`, `guest`, `guest_nice`

### `ram` — `/proc/meminfo`

Kibibytes. `mem_total`, `mem_free`, `mem_available`, `buffers`, `cached`,
`swap_total`, `swap_free`.

### `disk` — `/proc/diskstats`

`{"stats": [...]}` with one entry per block device and partition.
`device`, `reads_completed`, `reads_merged`, `read_bytes`, `read_time_ms`,
`writes_completed`, `writes_merged`, `written_bytes`, `write_time_ms`,
`io_in_progress`, `io_time_ms`, `weighted_io_time_ms`. Byte fields are
converted from the kernel's 512-byte sectors.

### `uptime` — `/proc/uptime`

`uptime_seconds`, `idle_time_seconds` (summed over all CPUs),
`boot_time_seconds` (Unix time derived from the wall clock).

### `load_average` — `/proc/loadavg`

`one_minute`, `five_minutes`, `fifteen_minutes`, `running_processes`,
`total_processes`.

### `processes` — `/proc/stat`

`total_processes` (forks since boot), `running_processes`,
`blocked_processes`, `context_switches`.

## Standard build

### `network` — `/proc/net/dev`

`{"stats": [...]}` per interface: `interface`, `receive_bytes`,
`receive_packets`, `receive_errors`, `receive_drops`, `transmit_bytes`,
`transmit_packets`, `transmit_errors`, `transmit_drops`.

### `netstat` — `/proc/net/netstat`

`{"entries": [{"protocol": "TcpExt", "stat_name": "...", "value": n}, ...]}`
for every counter in the `TcpExt`, `IpExt` and other groups.

### `sockstat` — `/proc/net/sockstat`

`{"entries": [{"protocol": "TCP", "metric": "inuse", "value": n}, ...]}`.
The `sockets: used` line appears with protocol `sockets`.

### `vmstat` — `/proc/vmstat`

`{"entries": [{"key": "pgfault", "value": n}, ...]}` for every counter.

### `filesystem` — `/proc/self/mounts` and `statvfs(2)`

`{"stats": [...]}` per mounted filesystem with a backing device:
`device`, `mount_point`, `fs_type`, `total_bytes`, `used_bytes`,
`available_bytes` (for unprivileged users), `total_inodes`, `used_inodes`,
`available_inodes`. Pseudo filesystems (`proc`, `sysfs`, `tmpfs`,
`overlay`, ...) and network filesystems (`nfs`, `cifs`, `fuse.*`, ...) are
skipped. A mount whose `statvfs` call does not return within 2 s is skipped
until that call completes, so a hung mount cannot stall the agent.

### `systemd` — `systemctl list-units --all --plain`

`{"stats": [...]}` per unit: `unit_name`, `load_state`, `active_state`,
`sub_state`, `description`. Requires `systemctl` on the host and access to
the system D-Bus; the command is bounded to 5 s. Output can reach tens of
kilobytes on hosts with many units.

### `ntp` — `adjtimex(2)`

Kernel clock discipline as maintained by whichever time daemon runs
(chrony, ntpd, systemd-timesyncd, ptp4l): `synchronized`, `clock_state`
(0 ok ... 5 unsynchronized), `status` (raw `STA_*` bits), `offset_ms`,
`max_error_ms`, `estimated_error_ms`, `frequency_ppm`, `tai_offset`. No
privileges or subprocess are needed. Linux only.

## Enabling collectors

```toml
[[metrics.collectors.enabled]]
name = "network"
[metrics.collectors.enabled.metadata]
topic = "metrics/network"
qos = 0
retain = true
```

A collector that is enabled in the file but not compiled into the binary is
reported at startup together with the list of available names.
