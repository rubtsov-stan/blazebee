# Changelog

## Unreleased

This release reworks the runtime, the transport and most collectors. Several
configuration keys changed; the loader now rejects unknown keys, so an old
file fails at startup with a message naming the offending key.

### Breaking changes

- `metrics.collection_interval` is read from `[metrics]` as documented. The
  previous location `[metrics.collectors]` was never read, so the interval
  silently stayed at 5 s.
- Unknown configuration keys are errors.
- `transport.serialization.preset` was removed. Set `format`, `compression`
  and `compression_threshold` explicitly. The default is plain JSON. (The
  old loader applied gzip when the whole section was absent and no
  compression when it was present, regardless of the keys inside.)
- `transport.max_packet_size` is a plain integer up to 268435455 (default
  1 MiB, previously capped at 65535); `transport.request_channel_capacity`
  defaults to 64 (previously 1) and must exceed the number of enabled
  collectors.
- `logger.timestamp_format` accepts `rfc3339` or `unix`; the `Custom` form is
  gone. `[logger.journald]` no longer requires `identifier`.
- The `blazebee-mqtt-v3` Cargo feature was removed; MQTT is always built in.
  Default features now select the `standard` collector set instead of an
  empty one. The misspelled `collector-filedfd` feature is `collector-filefd`.
- Sample schemas corrected or extended (additive unless noted):
  `cpu` adds `steal`, `guest`, `guest_nice`; `processes` adds
  `blocked_processes`; `disk` adds `weighted_io_time_ms` and reports
  `read_bytes`/`written_bytes` in bytes (previously 512-byte sectors under a
  byte name); `thermal` reports fractional degrees and adds `zone_type`;
  `hwmon` reports every `temp*_input` and adds `chip`, `sensor`, `label`;
  `pressure` adds `some_total_us`/`full_total_us`; `softnet` adds
  `received_rps`, `flow_limit_count`; `power` adds `online`, `charge_*`,
  `current_now`; `mdraid` adds `degraded`, `sync_action`; `schedstat`
  replaces the meaningless `runnable_ns` with `timeslices`; `ntp` now
  reports kernel clock discipline (`clock_state`, `status`, `offset_ms`,
  `max_error_ms`, `estimated_error_ms`, `frequency_ppm`, `tai_offset`)
  instead of fields that were always zero.

### Fixed

- `network`: transmit counters were read from the receive `fifo`, `frame`,
  `compressed` and `multicast` columns.
- `schedstat`: `running_ns`/`run_delay_ns` were read from the wrong columns.
- `ipvs`: counters are hexadecimal and the per-second lines overwrote the
  totals.
- `mdraid`: device counts were derived from substring counts; geometry is
  now parsed from `[n/m]`, failed/spare flags from the member list, and
  recovery/resync/check progress is reported.
- `ntp`: `timedatectl` output was parsed with keys that do not exist, so the
  collector always reported an unsynchronized clock with zero offset.
- `systemd`: failed units were mis-parsed because of the bullet column;
  `systemctl` now runs with `--plain --full` and a 5 s timeout.
- `filesystem`: `used_bytes` was computed from the available (non-root)
  space; mount points with escaped characters were skipped; `statvfs` ran on
  the runtime thread and could hang the agent on a stuck mount.
- `conntrack`: the collector counted the lines of `/proc/net/nf_conntrack`
  (O(flows), privileged) instead of reading `nf_conntrack_count`.
- Configuration validation never ran for the metrics and transport sections;
  `collection_interval = 0` produced a busy loop at full CPU.
- `username`/`password` and `${ENV}` substitution were documented but not
  implemented; credentials are now sent and secrets stay out of the file.
- After roughly 11 minutes of broker unavailability the connection task gave
  up and panicked while the process kept running without ever publishing
  again. Reconnection is now unbounded with a 60 s cap.
- A disconnect request queued by the supervisor was replayed after
  reconnecting, which could flap the connection.
- No last will was registered and `offline` was never published, so the
  status topic stayed `online` after a crash or stop.
- `SIGTERM` was not handled: systemd stops killed the process (exit 143)
  without a DISCONNECT. Both `SIGTERM` and `SIGINT` now shut down cleanly
  with exit code 0.
- The agent subscribed to every topic it published to and received its own
  messages back.
- The crate did not compile on non-Linux hosts or with `--no-default-features`.
- journald initialization failures aborted startup exactly when console
  logging was available as a fallback.
- `[logger.journald]` without `identifier` failed to parse.
- The Debian package referenced a `blazebee` system user it never created;
  the unit now uses `DynamicUser`.

### Changed

- Single-threaded runtime: one OS thread instead of one worker per CPU plus
  a blocking pool for every `/proc` read.
- Collectors read files synchronously into reusable buffers and parse without
  intermediate vectors; each collector call is bounded by a 10 s timeout.
- While the broker is unreachable collection is paused (logged once) and
  resumed with a count of skipped cycles; nothing is buffered beyond the
  bounded transport queue.
- Payloads larger than `max_packet_size` are rejected by the publisher
  instead of triggering a reconnect.
- Compression runs synchronously with `flate2`/`zstd`; CBOR uses the
  maintained `ciborium` crate instead of the archived `serde_cbor`.
- Transport crate reduced to connection driver, publisher, serializer and
  framer; `async-compression`, `async-stream`, `async-trait`, `bytes`,
  `futures-*`, `rustls 0.21`, `rustls-pemfile`, `tokio-util`, `uuid`,
  `validator` and the duplicate `thiserror 1` were dropped. The main crate
  dropped `config`, `console`, `directories`, `futures`, `inventory`,
  `once_cell`, `time`, `uuid`, `validator`, `bytes`, `tokio-util`,
  `async-compression`, `async-trait` and the unused `tempdir`/`tokio-test`/`tracing-test`.
- Release binaries are stripped; `RUST_LOG` overrides `logger.level`.
- systemd unit hardened (`DynamicUser`, `CapabilityBoundingSet=`,
  `SystemCallFilter`, `ProtectProc`, ...); `AF_UNIX` allowed for journald.
- Container build uses `--locked --no-default-features` and sets
  `SSL_CERT_FILE` for the system trust store.

### Added

- `transport.tls.enabled` (system trust store without a CA file).
- Parser tests against real `/proc` snapshots, scheduler and transport
  tests, criterion benchmarks (`cargo bench --features large`).
- CI workflow: format, clippy, tests, per-feature builds, MSRV check and a
  Debian package install test.

## 0.1.1

Initial public release.
