---
title: MQTT Configuration
description: Broker connection, authentication, TLS, encoding and delivery behaviour.
---

BlazeBee speaks MQTT 3.1.1 through the [`rumqttc`](https://github.com/bytebeamio/rumqtt)
client. All settings live under `[transport]`.

## Connection

| Key                        | Default              | Notes |
|----------------------------|----------------------|-------|
| `host`                     | `"localhost"`        | Host name or IP address. |
| `port`                     | `1883`               | `8883` is the usual TLS port. |
| `base_topic`               | `"blazebee_mqtt_v3"` | Prefix for every topic, including `status`. May be empty. Set it explicitly; the example file uses `"blazebee"`. |
| `client_id`                | `""`                 | Empty means `blazebee-<hostname>`. Keep it stable when `clean_session = false`. |
| `username`, `password`     | unset                | Sent in CONNECT. Use `password = "${MQTT_PASSWORD}"` to read it from the environment. `password` requires `username`. |
| `keep_alive`               | `60`                 | Seconds, 5 .. 65535. |
| `clean_session`            | `false`              | `false` keeps the session and queued QoS 1/2 messages on the broker across reconnects. |
| `max_inflight`             | `10`                 | Unacknowledged QoS 1/2 publishes allowed at once. |
| `max_packet_size`          | `1048576`            | Bytes, 64 .. 268435455. Larger payloads are dropped with an error instead of being sent. |
| `request_channel_capacity` | `64`                 | Bounded queue between collectors and the network. Must be larger than the number of enabled collectors. |

## Status topic and last will

On every successful connection the agent publishes `online` (retained,
QoS 1) to `<base_topic>/status`. The CONNECT packet carries a last will
that publishes `offline` to the same topic if the connection drops without a
DISCONNECT; a clean shutdown publishes `offline` itself. Subscribe to this
topic to distinguish an idle agent from an unreachable host.

## TLS

```toml
[transport.tls]
enabled = true
# ca_cert_path     = "/etc/ssl/certs/ca-certificates.crt"
# client_cert_path = "/etc/blazebee/client.crt"
# client_key_path  = "/etc/blazebee/client.key"
```

- TLS is used when `enabled = true` or when `ca_cert_path` is set.
- Without `ca_cert_path`, the broker certificate is verified against the
  system trust store (`SSL_CERT_FILE` in the container image).
- `ca_cert_path` points to a PEM bundle for private CAs.
- `client_cert_path` and `client_key_path` (unencrypted PEM) enable mutual
  TLS and must be set together. All files are checked at startup.

TLS handshake failures are treated as unrecoverable: the process exits with
status 1 rather than retrying with a configuration that cannot work.

## Serialization and compression

```toml
[transport.serialization]
format = "json"               # json | msgpack | cbor (default json)
compression = "none"          # none | gzip | zstd (default none)
compression_threshold = 1024  # bytes; smaller payloads are sent as is
```

gzip reduces the larger payloads (`vmstat`, `netstat`) by roughly 80 %; on
an aarch64 server core it costs about 110 µs for a 7 KB message, zstd about
35 µs with a similar ratio.

MessagePack uses positional (array) encoding of structs; CBOR encodes
structs as maps keyed by field name. MQTT 3.1.1 has no content-type
metadata, so consumers must know the configured format and compression.

## Framing

```toml
[transport.framing]
enabled = false
```

When enabled, every payload is split into frames that fit
`max_packet_size`. Each frame starts with a 21-byte header: magic `FR`,
version `1`, format (`0` json, `1` msgpack, `2` cbor), QoS, a 64-bit stream
id, a 32-bit frame index and the 32-bit frame count, all big-endian. Frames
are published to the sample's topic in order. Leave this off unless the
consumer reassembles frames; with it on, even a small payload carries the
header.

## Connection behaviour

- **Reconnection**: after a lost or refused connection the agent waits 1 s,
  then doubles the delay up to 60 s, with ±20 % jitter, and keeps retrying
  for as long as the process runs. A successful connection resets the delay.
- **Fatal errors**: bad credentials (`BadUserNamePassword`, `NotAuthorized`),
  rejected protocol version or client id, TLS errors and local permission
  problems stop the process with exit status 1. Under systemd this triggers
  `Restart=on-failure`.
- **While disconnected**: no samples are taken. Publishes that were already
  queued are retransmitted after reconnect according to their QoS and the
  session setting.
- **Shutdown**: `offline` is published, DISCONNECT is sent and the process
  waits up to 3 s for the broker to take it.

## Example with a private CA and credentials

```toml
[transport]
host = "mqtt.internal"
port = 8883
base_topic = "site-a/blazebee"
username = "blazebee"
password = "${MQTT_PASSWORD}"

[transport.tls]
ca_cert_path = "/etc/blazebee/ca.pem"
```

Run with `MQTT_PASSWORD` in the environment. systemd units can supply it
through `EnvironmentFile=` or a credential; containers through `-e` or a
secret mount.
