# BlazeBee

[![License](https://img.shields.io/github/license/rubtsov-stan/blazebee)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.86+-orange.svg)](https://www.rust-lang.org/)
[![MQTT](https://img.shields.io/badge/mqtt-3.1.1-blue)](https://mqtt.org/)

BlazeBee is a small Linux agent that samples kernel interfaces (`/proc`,
`/sys`, `adjtimex`) at a fixed interval and publishes the raw counters as
JSON, MessagePack or CBOR messages to an MQTT broker. It is built for
long-running, unattended operation on servers, edge devices and containers.

- One thread, no locks: a single-threaded Tokio runtime drives the sampling
  loop, the MQTT event loop and signal handling.
- Bounded memory: samples are never buffered beyond a fixed-size queue;
  collection pauses while the broker is unreachable and resumes with fresh
  values.
- Reconnects forever with capped, jittered backoff; only unrecoverable errors
  (rejected credentials, TLS misconfiguration) stop the process.
- Retained `<base_topic>/status` topic (`online`/`offline`) backed by an MQTT
  last will, so consumers can tell a silent agent from a dead one.
- Clean shutdown on `SIGTERM` and `SIGINT`: final status, DISCONNECT, exit 0.
- 27 collectors, each behind a Cargo feature; three presets (`minimal`,
  `standard`, `large`) for binaries, Debian packages and container images.

## Install

**Debian package** (amd64, arm64; variants `minimal`, `standard`, `large`):

```bash
sudo apt install ./blazebee_<version>_<arch>.deb
sudo editor /etc/blazebee/config.toml      # set the broker address
sudo systemctl restart blazebee
journalctl -u blazebee -f
```

The service runs under a dynamic unprivileged user with a strict systemd
sandbox; see `contrib/systemd/blazebee.service`.

**Container** (image variants `minimal`, `latest` = `standard`, `large`):

```bash
docker run --rm --network host \
  -v "$PWD/config.toml:/etc/blazebee/config.toml:ro" \
  rubtsovs/blazebee:latest
```

**From source** (Rust 1.86 or newer):

```bash
cargo build --release --no-default-features --features standard
BLAZEBEE_CONFIG=config.example.toml ./target/release/blazebee
```

## Configuration

`config.example.toml` documents every key. The file is located through
`BLAZEBEE_CONFIG` or `/etc/blazebee/config.toml`; `${NAME}` references are
replaced with environment variables, which keeps credentials out of the file:

```toml
[transport]
host = "broker.example.net"
port = 8883
base_topic = "blazebee"
username = "blazebee"
password = "${MQTT_PASSWORD}"

[transport.tls]
enabled = true                      # system trust store, or set ca_cert_path

[metrics]
collection_interval = 5

[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "metrics/cpu"
qos = 0
retain = true
```

Unknown keys and invalid values are rejected at startup with the key name in
the error message.

## Collectors

| Preset     | Collectors                                                                 |
|------------|----------------------------------------------------------------------------|
| `minimal`  | `cpu`, `ram`, `disk`, `uptime`, `load_average`, `processes`                 |
| `standard` | minimal + `network`, `netstat`, `sockstat`, `vmstat`, `filesystem`, `systemd`, `ntp` |
| `large`    | standard + `arp`, `softnet`, `conntrack`, `ipvs`, `pressure`, `thermal`, `hwmon`, `entropy`, `filefd`, `schedstat`, `stat`, `power`, `edac`, `mdraid` |

Every collector publishes raw kernel counters; rates are left to the
consumer. Field-level documentation is in the [metrics reference][docs].

## How it works

```
/proc, /sys ──▶ collectors ──▶ scheduler ──▶ publisher ──▶ rumqttc ──▶ broker
                (parse)        (interval,     (encode,      (event loop,
                               timeouts)     compress)     reconnect)
```

1. The configuration is loaded, expanded and validated.
2. The transport connects in the background and reports its state through a
   watch channel.
3. Every `collection_interval` seconds, while the connection is up, the
   scheduler runs each enabled collector (bounded to 10 s each), encodes the
   sample and queues it without blocking. Failures are isolated per collector
   and logged with rate limiting.
4. On `SIGTERM`/`SIGINT` the scheduler stops, `offline` is published and the
   connection is closed with a DISCONNECT packet.

Details are in the [architecture document][docs].

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --features large -- -D warnings
cargo test --workspace --features large
cargo bench --features large          # parser and encoder micro-benchmarks
cargo deb --variant standard          # Debian package (cargo install cargo-deb)
make docker-local-standard            # container image via buildx
```

Tests use real `/proc` snapshots from `tests/fixtures`; collectors reading
`/sys` are tested against synthetic directory trees.

## License

Apache License 2.0; see [LICENSE](LICENSE).

[docs]: https://rubtsov-stan.github.io/blazebee/
