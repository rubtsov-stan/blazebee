---
title: Configuration Guide
description: Every configuration key, its default and its constraints.
---

BlazeBee reads one TOML file. The path comes from the `BLAZEBEE_CONFIG`
environment variable, or `/etc/blazebee/config.toml` when the variable is
unset. `config.example.toml` in the repository is a complete, commented
example.

## Loading rules

- `${NAME}` inside any string value is replaced with the environment
  variable `NAME`; keys and comments are not touched. An unset variable is an
  error, so a literal `${MQTT_PASSWORD}` can never reach the broker. `$${`
  produces a literal `${`; a plain `$` is left alone.
- Unknown keys are rejected. The error message names the key.
- Every value is validated at startup; the process exits with status 2 and
  a message on stderr if anything is wrong.
- `RUST_LOG`, when set, overrides `logger.level`.

## `[logger]`

| Key                | Default   | Notes |
|--------------------|-----------|-------|
| `level`            | `"info"`  | `trace`, `debug`, `info`, `warn`, `error`, `off`, or a `tracing` directive list such as `"info,rumqttc=warn"`. |
| `timestamp_format` | `"rfc3339"` | `rfc3339` or `unix` (seconds since the epoch with microseconds). |

### `[logger.console]`

| Key               | Default     | Notes |
|-------------------|-------------|-------|
| `enabled`         | `true`      | Log to stdout. |
| `format`          | `"compact"` | `compact`, `pretty` or `json`. |
| `show_target`     | `false`     | Include the module path. |
| `show_thread_ids` | `false`     | |
| `show_spans`      | `false`     | Emit span close events with timing. |
| `ansi_colors`     | `true`      | Disable when stdout is not a terminal. |

### `[logger.journald]`

| Key          | Default      | Notes |
|--------------|--------------|-------|
| `enabled`    | `false`      | Write to systemd-journald directly. Leave off when stdout already goes to the journal. |
| `identifier` | `"blazebee"` | `SYSLOG_IDENTIFIER` of the entries. |

If journald is enabled but unavailable and the console is enabled, a warning
is printed and logging continues on the console. If journald is the only
output and unavailable, startup fails.

## `[transport]`

See the [MQTT configuration](mqtt) page for the connection, TLS,
serialization and framing keys.

## `[metrics]`

| Key                   | Default | Notes |
|-----------------------|---------|-------|
| `collection_interval` | `5`     | Seconds between sampling cycles, at least 1. |

### `[[metrics.collectors.enabled]]`

One table per collector. At least one is required; a name may appear once.

| Key               | Notes |
|-------------------|-------|
| `name`            | Registered collector name (`cpu`, `ram`, ...). Must be compiled into the binary; the error lists the available names otherwise. |
| `metadata.topic`  | Topic relative to `transport.base_topic`. Must not be empty or contain `+`, `#`. |
| `metadata.qos`    | `0`, `1` or `2` (default `0`). |
| `metadata.retain` | Retain the latest sample on the broker (default `false`). |

```toml
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "metrics/cpu"
qos = 0
retain = true
```

With `base_topic = "blazebee"` this publishes to `blazebee/metrics/cpu`.

## Upgrading from 0.1.x

Version 0.2.0 rejects unknown keys and moved or removed a few of them:

| 0.1.x | 0.2.0 |
|-------|-------|
| `[metrics.collectors] collection_interval` | `[metrics] collection_interval` |
| `[transport.serialization] preset` | removed; set `format`, `compression`, `compression_threshold` |
| `[logger] timestamp_format = "Custom(...)"` | `rfc3339` or `unix` |
| `client_id` empty → random UUID | `client_id` empty → `blazebee-<hostname>` |
| `max_packet_size` ≤ 65535 | up to 268435455, default 1048576 |

Run the new binary once against the old file; each rejected key is reported
with its location. The full list of behaviour changes is in the project
`CHANGELOG.md`.

## Minimal file

```toml
[transport]
host = "broker.example.net"

[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "metrics/cpu"
```

Everything else takes its default. The client id becomes
`blazebee-<hostname>`.
