---
title: Building from Source
description: Toolchain, feature flags and release builds.
---

## Requirements

- Rust 1.86 or newer (`rustup toolchain install stable`).
- A C compiler: `zstd-sys` and `ring` (TLS) compile C sources.
- Linux for running; the code also builds and its tests run on macOS.

## Build

```bash
git clone https://github.com/rubtsov-stan/blazebee.git
cd blazebee
cargo build --release --no-default-features --features standard
./target/release/blazebee            # expects BLAZEBEE_CONFIG or /etc/blazebee/config.toml
```

The release profile uses LTO, a single codegen unit and stripped symbols.

## Features

Collectors are selected at compile time. The default feature set is
`standard`; pass `--no-default-features` when choosing another preset so
the sets do not add up.

| Feature    | Contents |
|------------|----------|
| `minimal`  | `collector-cpu`, `collector-ram`, `collector-disk`, `collector-uptime`, `collector-loadavg`, `collector-processes` |
| `standard` | `minimal` + `collector-network`, `collector-netstat`, `collector-sockstat`, `collector-vmstat`, `collector-filesystem`, `collector-systemd`, `collector-ntp` |
| `large`    | `standard` + `collector-arp`, `collector-softnet`, `collector-conntrack`, `collector-ipvs`, `collector-pressure`, `collector-thermal`, `collector-hwmon`, `collector-entropy`, `collector-filefd`, `collector-schedstat`, `collector-stat`, `collector-power`, `collector-edac`, `collector-mdraid` |

Individual `collector-*` features can be combined freely:

```bash
cargo build --release --no-default-features --features "collector-cpu,collector-ram"
```

A binary built without any collector starts, reports that the configured
collectors are unavailable and exits with status 2.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --features large -- -D warnings
cargo test --workspace --features large
cargo bench --features large
```

## Packages and images

```bash
cargo install cargo-deb
cargo deb --variant standard         # target/debian/*.deb, also: minimal, large

make docker-local-standard           # local amd64 image via buildx
make docker-standard                 # multi-arch build and push
```

The container image is built from `rust:1.86-alpine` with the musl target
and copied into `scratch` together with the CA bundle. Pass
`--build-arg CARGO_FEATURES=large` for another variant.

## Cross-compilation

The Debian workflow builds natively on amd64 and arm64 runners. For local
cross builds use [`cross`](https://github.com/cross-rs/cross) or a
container for the target architecture; the C dependencies need a matching
toolchain.
