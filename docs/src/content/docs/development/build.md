---
title: Building from Source
description: Compile BlazeBee from source for custom or development use.
---

### Overview

BlazeBee is built using the standard Rust toolchain and relies on Cargo feature flags to control enabled collectors, transport implementations, and binary footprint. This allows producing binaries tailored for constrained edge devices, standard servers, or full-featured monitoring nodes.

This page documents all supported build paths and feature combinations.

### Prerequisites

### Required

- **Rust toolchain (stable)**  
  Installed via `rustup`. The project targets the stable channel and does not require nightly features.

- **Cargo**  
  Comes bundled with Rust. Used for dependency resolution, feature management, and builds.

- **Git**  
  Required to clone the repository.

### Optional

- **Docker**  
  Required only for containerized builds using the provided `Makefile`.

### Repository Layout (Build-Relevant)

- `Cargo.toml` — defines features, dependencies, and build profiles  
- `src/` — core application and collectors  
- `collectors/` — metric collectors grouped by feature sets  
- `Dockerfile` — multi-stage build for minimal runtime images  
- `Makefile` — wrapper for Docker builds and feature presets  

### Building the Binary

### Clone the Repository

```bash
git clone https://github.com/rubtsov-stan/blazebee.git
cd blazebee
````

### Standard Cargo Build

```bash
cargo build --release --features <FEATURES>
```

The resulting binary is located at:

```text
target/release/blazebee
```

The `--release` flag enables compiler optimizations and should be used for all production builds.

### Feature Flags

BlazeBee relies heavily on Cargo features to control compilation. Features are **explicit** and **additive**.

### Transport Features

| Feature            | Description                               |
| ------------------ | ----------------------------------------- |
| `blazebee-mqtt-v3` | Enables MQTT v3 transport using `rumqttc` |

Exactly one transport feature must be enabled.

### Preset Feature Groups

Preset features enable predefined sets of collectors and dependencies.

### `minimal`

* Core runtime
* Basic system metrics (CPU, memory, uptime)
* Lowest binary size
* Intended for edge devices and constrained environments

```bash
cargo build --release --features "blazebee-mqtt-v3 minimal"
```

### `standard`

* Default recommended configuration
* CPU, memory, disk, network, load, processes
* Balanced coverage and resource usage

```bash
cargo build --release --features "blazebee-mqtt-v3 standard"
```

### `large`

* All available collectors
* Includes advanced and kernel-level metrics
* Highest binary size and permissions requirements

```bash
cargo build --release --features "blazebee-mqtt-v3 large"
```

### Notes on Features

* Presets internally enable multiple collector-specific flags.
* Feature sets are resolved at **compile time**; unused collectors are not included in the binary.
* Some collectors in `large` may require:

  * Linux-only environments
  * Access to `/proc`, `/sys`
  * Elevated privileges

### Custom Feature Combinations

You may define custom builds by explicitly listing features:

```bash
cargo build --release --features "blazebee-mqtt-v3 cpu ram disk network"
```

This approach is intended for advanced use cases where preset groups are insufficient.

### Docker Builds

### Build Using Makefile

BlazeBee provides a `Makefile` abstraction over Docker builds.

```bash
make docker TYPE=standard
```

#### Supported Docker Variants

| TYPE       | Description                        |
| ---------- | ---------------------------------- |
| `minimal`  | Minimal collectors, smallest image |
| `standard` | Recommended default                |
| `large`    | All collectors enabled             |

#### Build Characteristics

* Multi-stage Docker build
* Static or near-static release binary
* Minimal runtime image (no Rust toolchain inside container)
* Feature selection passed as build arguments

The resulting image contains only:

* `blazebee` binary
* Required system libraries
* Default config path `/etc/blazebee/config.toml`

#### Build Profiles

BlazeBee uses Cargo’s default `release` profile:

* `opt-level = 3`
* Debug symbols disabled
* Panic strategy optimized for performance

Custom profiles can be defined in `Cargo.toml` if needed for debugging or profiling.

### Platform Support

* **Linux**: Fully supported (primary target)
* **macOS**: Buildable, limited collector availability
* **Windows**: Not supported (collectors rely on Linux system interfaces)

### Summary

* BlazeBee is built via Cargo with explicit feature flags.
* Preset feature groups (`minimal`, `standard`, `large`) cover most use cases.
* Docker builds are reproducible and optimized for deployment.
* Feature selection directly impacts binary size, permissions, and runtime behavior.

This build system enables precise control over functionality without runtime overhead.
