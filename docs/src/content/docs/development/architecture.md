---
title: Architecture
description: How BlazeBee is structured, how data flows and how failures are handled.
---

BlazeBee is a single process with one OS thread of its own. A Tokio
`current_thread` runtime drives three concerns: the sampling loop, the MQTT
event loop and signal handling. There are no locks and no shared mutable
state; the components communicate through a bounded request queue and a
watch channel.

## Components

```
config ─▶ main ─┬─▶ Scheduler ──(Publisher::publish)──▶ rumqttc queue ─▶ Driver task ─▶ broker
                │        ▲                                                   │
                │        └──────────── watch<ConnectionState> ◀──────────────┘
                └─▶ signals (SIGTERM, SIGINT)
```

**Configuration** (`src/config`) locates the TOML file, substitutes
`${NAME}` with environment variables, parses it with unknown keys rejected
and validates every section. Collector names are checked against the
registry compiled into the binary, so a typo or a collector missing from the
build is reported before anything starts.

**Collectors** (`src/core/collectors`) implement one trait:

```rust
pub trait Collector: Send {
    fn collect(&mut self) -> CollectFuture<'_>; // boxed future of CollectorResult<Sample>
}
```

A `Sample` is a boxed `erased_serde::Serialize`, which lets the transport
encode any collector output with the configured format. Collectors keep
their state (`&mut self`): the read buffer for a `/proc` file is allocated
once, the filesystem collector remembers mounts whose `statvfs` call never
returned. Each collector module exposes a pure `parse` function so parsing
is tested against captured kernel output without touching the host.

The registry is a static slice of `(name, constructor)` pairs, one entry per
`collector-*` feature. Disabled collectors are not compiled.

**Scheduler** (`src/core/scheduler.rs`) owns the collectors and the
publisher. A `tokio::time::interval` with `MissedTickBehavior::Skip` fires
every `collection_interval` seconds. Each tick:

1. If the connection state is fatal, the loop returns and the process exits
   with status 1.
2. If the broker is not connected, the cycle is skipped. This is logged once
   when the pause starts and once, with the number of skipped cycles, when
   collection resumes.
3. Otherwise each collector runs in turn under a 10 s timeout, its sample is
   encoded and queued. A failure (collect, timeout, encode, full queue) is
   counted per collector; the first failure and every 120th are logged at
   `warn`, the rest at `debug`, and recovery is logged at `info`.

Collectors run sequentially on the runtime thread. `/proc` reads are served
from kernel memory and take microseconds, so offloading them would cost more
than it saves. The two operations that can block, `statvfs` and
`systemctl`, run on a blocking thread and as a child process respectively,
both with deadlines.

**Transport** (`crates/mqtt/v3`) wraps `rumqttc`:

- `Transport::connect` validates the transport configuration, builds the
  client with credentials, TLS and a last-will message, and spawns the
  driver task.
- The driver polls the event loop. On `CONNACK` it publishes `online`
  (retained) to `<base_topic>/status` and resets the backoff. On an error
  it classifies the cause: rejected credentials, protocol or client-id
  rejection, TLS failures and local permission errors are fatal; everything
  else waits `1 s × 2ⁿ` (capped at 60 s, ±20 % jitter) and retries, forever.
- `Publisher::publish` encodes a value (JSON, MessagePack or CBOR, optional
  gzip or zstd above a size threshold), checks it against
  `max_packet_size` and calls `try_publish`. The queue between publishers and
  the event loop is bounded by `request_channel_capacity`; a full queue is an
  error for that sample, never a stall.
- `Transport::shutdown` publishes `offline`, queues DISCONNECT and polls
  until the packet has been written or 3 s have passed.

## Delivery policy

Metrics are latest-value samples. A sample taken while the broker is
unreachable would be stale by the time it could be delivered, so none is
taken. QoS 1 and 2 publishes that were already queued when a connection
drops are retransmitted by `rumqttc` on reconnect when `clean_session =
false`, exactly as the protocol specifies; with `clean_session = true` they
are dropped with the session. Memory use is therefore independent of outage
length.

## Lifecycle

```
load config ─▶ init logging ─▶ connect transport ─▶ run scheduler
      │              │                 │                   │
    exit 2         exit 2       exit 1 on setup error      │
                                                           ├─ SIGTERM/SIGINT ─▶ offline + DISCONNECT ─▶ exit 0
                                                           └─ fatal connection error ─▶ exit 1
```

Configuration and logging errors are printed to stderr before the logger
exists. Under systemd, `Restart=on-failure` restarts the service after a
fatal transport error but not after a clean stop.

## Resource profile

Measured on Linux 7.1 (aarch64, 6 vCPUs) with all 27 collectors enabled at a
1 s interval against a local Mosquitto broker, 60 s run, compared with the
previous multi-threaded design built from the same commit:

| Metric                         | Previous | Current |
|--------------------------------|----------|---------|
| Threads                        | 50       | 2       |
| Resident memory (VmRSS)        | 7.4 MiB  | 4.3–4.6 MiB |
| Virtual size (VmSize)          | 3.1 GiB  | 74 MiB  |
| CPU time per 60 s              | 0.58 s   | 0.13–0.19 s |
| System calls per second        | ~1180    | ~320    |
| Heap allocations per cycle     | ~2800    | ~1100   |
| Shutdown on SIGTERM            | killed, exit 143, status left `online` | clean, exit 0, status `offline`, under 5 ms |

A 10-minute run at the same interval kept VmHWM at 4.3 MiB, one thread and
ten open descriptors throughout.

At the default 5 s interval the CPU cost is proportionally lower. Steady
state allocates per cycle only the output structures, the encoded payload
and, when enabled, the compression buffer; the read buffers are reused.
