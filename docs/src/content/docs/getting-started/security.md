---
title: Security
description: Transport security, credentials handling and process isolation.
---

## What the agent does

BlazeBee reads kernel files, runs `systemctl list-units` when the `systemd`
collector is enabled, and opens one outbound TCP (or TLS) connection to the
broker. It never listens on a socket, never subscribes to topics and never
writes to disk.

## Transport

- Enable TLS for any broker that is not on the same host:
  `[transport.tls] enabled = true` verifies the broker certificate against
  the system trust store; `ca_cert_path` selects a private CA;
  `client_cert_path`/`client_key_path` add mutual TLS.
- Certificate verification cannot be disabled.
- TLS failures stop the process instead of retrying.

## Credentials

- `username`/`password` are sent in the CONNECT packet; use TLS with them.
- Reference secrets as `${MQTT_PASSWORD}` and provide them through the
  environment (`EnvironmentFile=` under systemd, secrets in containers).
  An unset variable aborts startup, so a placeholder is never sent.
- The password type is redacted in debug output and the configuration is
  never logged.

## Process isolation

The shipped unit applies: `DynamicUser`, empty capability bounding set,
`NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome`, `PrivateTmp`,
`PrivateDevices`, `ProtectKernelTunables/Modules/Logs`,
`ProtectControlGroups`, `ProtectHostname`, `ProtectProc=invisible`,
`RestrictNamespaces`, `RestrictRealtime`, `RestrictSUIDSGID`,
`LockPersonality`, `MemoryDenyWriteExecute`, `UMask=0077`,
`SystemCallArchitectures=native`,
`SystemCallFilter=@system-service @clock` and
`RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK`.

`@clock` is required for the read-only `adjtimex` query of the `ntp`
collector; without `CAP_SYS_TIME` the clock cannot be modified.

## Resource bounds

- One sample per collector per interval; nothing is buffered while the
  broker is down.
- The queue towards the network holds at most `request_channel_capacity`
  messages.
- Each collector call is limited to 10 s; `statvfs` to 2 s per mount;
  `systemctl` to 5 s.
- Payloads above `max_packet_size` are dropped with an error.
- The agent never decompresses or parses broker data.

## Dependencies

TLS is provided by `rustls` through `rumqttc`. Compression uses `flate2`
(pure Rust) and the `zstd` C library. Run `cargo audit` or `cargo deny` as
part of release preparation; the lock file pins every dependency.
