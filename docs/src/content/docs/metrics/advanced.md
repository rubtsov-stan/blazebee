---
title: Advanced Metrics
description: Collectors included in the large build only.
---

These collectors cover hardware, kernel subsystems and network internals
that not every host exposes. A collector whose kernel interface is missing
fails each cycle; the failure is logged once at `warn`, then every 120
cycles, and does not affect the other collectors.

### `arp` — `/proc/net/arp`

`{"entries": [...]}`: `ip_address`, `hw_type`, `flags`, `hw_address`,
`mask`, `device`.

### `softnet` — `/proc/net/softnet_stat`

`{"stats": [...]}` per CPU (`cpu` is the line index): `processed`,
`dropped`, `time_squeeze`, `received_rps`, `flow_limit_count`.

### `conntrack` — `nf_conntrack_count`, `nf_conntrack_max` sysctls

`entries`, `max`. Reports "nf_conntrack is not loaded" when the module is
absent.

### `ipvs` — `/proc/net/ip_vs_stats`

Cumulative totals (the file's hexadecimal third line): `connections`,
`incoming_packets`, `outgoing_packets`, `incoming_bytes`, `outgoing_bytes`.

### `pressure` — `/proc/pressure/{cpu,memory,io}`

`{"stats": [...]}` per resource: `metric_type`, `some_avg10`, `some_avg60`,
`some_avg300`, `some_total_us`, and `full_*` equivalents (`null` where the
kernel does not report a `full` line). Requires `CONFIG_PSI` (`psi=1` on some
distributions).

### `thermal` — `/sys/class/thermal/thermal_zone*`

`{"zones": [...]}`: `zone_id`, `zone_type`, `temperature` (°C, millidegree
resolution). Zones that fail to read in a cycle are omitted.

### `hwmon` — `/sys/class/hwmon/*/temp*_input`

`{"sensors": [...]}`: `name` (`hwmonN`), `chip`, `sensor` (`tempN`),
`label`, `value` (°C), `unit` (`"C"`).

### `entropy` — `/proc/sys/kernel/random/entropy_avail`

`available_entropy` in bits.

### `filefd` — `/proc/sys/fs/file-nr`

`allocated`, `maximum`.

### `schedstat` — `/proc/schedstat`

`{"stats": [...]}` per CPU: `cpu`, `running_ns`, `run_delay_ns`,
`timeslices`.

### `stat` — `/proc/stat`

`boot_time` (Unix time), `total_forks`, `total_interrupts`.

### `power` — `/sys/class/power_supply`

`{"stats": [...]}` per supply: `name`, `supply_type`, `status`, `online`,
`capacity_percent`, `energy_now`, `energy_full`, `power_now`, `charge_now`,
`charge_full`, `current_now`, `voltage_now`. Attributes a supply does not
expose are `null`. Units are the kernel's micro-units (µWh, µW, µAh, µA, µV).

### `edac` — `/sys/devices/system/edac/mc`

`{"stats": [...]}` per memory controller: `controller`,
`correctable_errors`, `uncorrectable_errors`, `dimm_label` (comma-separated
labels when exposed).

### `mdraid` — `/proc/mdstat`

`{"stats": [...]}` per array: `device`, `level`, `state`, `active_devices`,
`total_devices`, `failed_devices`, `spare_devices`, `degraded`,
`sync_action` (`recovery`, `resync`, `reshape`, `check` or `null`),
`sync_progress` (percent or `null`).

## Building

```bash
cargo build --release --no-default-features --features large
```

or a specific subset:

```bash
cargo build --release --no-default-features \
  --features "collector-cpu,collector-ram,collector-pressure,collector-thermal"
```

## Permissions

All of the above are readable by an unprivileged user on a default kernel.
The systemd unit shipped with the package allows the `adjtimex` syscall
and `AF_UNIX` sockets so that the `ntp` and `systemd` collectors work inside
the sandbox; `ProtectClock=` would block the former.
