---
title: Custom Metrics
description: Extend BlazeBee by implementing and integrating user-defined metric collectors.
---

BlazeBee is designed as an extensible metrics platform. Beyond built-in system and advanced collectors, it allows developers to add **custom collectors** that gather arbitrary data and publish it through the same unified pipeline.

Custom metrics are first-class citizens: they follow the same lifecycle, configuration model, serialization, and transport logic as native collectors.

This page describes the internal extension mechanism and the exact steps required to implement and integrate custom metrics.

## Purpose

This guide explains:
- The internal collector abstraction
- How custom collectors are implemented
- How collectors are registered and discovered
- How build-time features control availability
- How custom collectors are enabled at runtime

It assumes familiarity with Rust and asynchronous programming.

## Collector Model

All metrics in BlazeBee are produced by components implementing a single abstraction: **`DataProducer`**.

Key properties of the model:

- Collectors are **asynchronous**
- Collectors are **thread-safe**
- Collectors are **registered at compile time**
- Collectors are **enabled or disabled at runtime via config**

Each collector:
- Produces structured data
- Does not handle transport or serialization directly
- Is isolated from other collectors

## Core Trait: `DataProducer`

Every collector must implement the `DataProducer` trait.

```rust
use super::types::CollectorResult;

#[async_trait::async_trait]
pub trait DataProducer: Send + Sync + 'static {
    type Output: Send + Sync + 'static;

    async fn produce(&self) -> CollectorResult<Self::Output>;
}
````

### Trait Semantics

* `Send + Sync`
  Required for safe execution across async tasks.

* `'static`
  Allows collectors to be stored in a global registry without lifetime coupling.

* `Output`
  The data structure returned by the collector. It is later serialized and published.

* `produce()`
  Invoked periodically by the runtime scheduler. This method **must not block**.

Errors returned from `produce()` are logged and isolated to the collector.

## Implementing a Custom Collector

### Step 1: Define the Collector Structure

```rust
struct CustomCollector;
```

This structure usually holds:

* Configuration
* Cached state
* External client handles (HTTP, IPC, etc.)

### Step 2: Define the Output Type

```rust
struct CustomOutput {
    value: i64,
    status: String,
}
```

The output type should:

* Be serializable
* Represent a single snapshot of collected data
* Avoid heavy allocations where possible

### Step 3: Implement `DataProducer`

```rust
#[cfg(all(feature = "my-custom-feature", target_os = "linux"))]
#[async_trait::async_trait]
impl DataProducer for CustomCollector {
    type Output = CustomOutput;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Custom collection logic
        Ok(CustomOutput {
            value: 42,
            status: "ok".to_string(),
        })
    }
}
```

### Notes

* Feature gates control compilation
* OS constraints prevent invalid builds
* I/O must be async-friendly
* Blocking syscalls should be avoided or wrapped carefully

## Collector Registration

Collectors are discovered via a compile-time registry.

To register a collector:

```rust
use crate::register_collector;

register_collector!(CustomCollector, "my-custom-name");
```

### Registration Behavior

* Associates a **string identifier** with the collector
* The identifier is later used in `config.toml`
* Registration occurs at compile time
* Duplicate names are rejected

The name must be:

* Stable
* Lowercase
* Unique across all collectors

## Build-Time Integration

Custom collectors are enabled through Cargo features.

### Step 1: Define a Feature

In `Cargo.toml`:

```toml
[features]
my-custom-feature = []
```

Optionally attach it to a preset:

```toml
standard = ["my-custom-feature"]
large = ["my-custom-feature"]
```

### Step 2: Build BlazeBee

```bash
cargo build --release --features "blazebee-mqtt-v3 my-custom-feature"
```

Or using Docker:

```bash
make docker TYPE=custom FEATURES="my-custom-feature"
```

If the feature is not enabled:

* The collector is not compiled
* It cannot be referenced in configuration

## Runtime Configuration

Once compiled in, the collector is enabled via `config.toml`.

```toml
[[metrics.collectors.enabled]]
name = "my-custom-name"
[metrics.collectors.enabled.metadata]
topic  = "metrics/custom"
qos    = 1
retain = true
```

### Configuration Semantics

* `name` must match the registered identifier
* `topic` defines the MQTT destination
* `qos` and `retain` control delivery semantics
* Absence from config means the collector is inactive

## Execution Lifecycle

1. BlazeBee starts
2. Configuration is parsed
3. Enabled collectors are instantiated
4. Each collector is scheduled independently
5. `produce()` is called at configured intervals
6. Output is serialized
7. Payload is published via transport
8. Errors are logged without stopping the runtime

Custom collectors participate fully in this lifecycle.

## Design Guidelines

When implementing custom collectors:

* Keep collection logic minimal
* Avoid global state
* Fail fast and return structured errors
* Prefer structured output over raw strings
* Assume collectors may run for long periods

Collectors should never:

* Panic
* Block indefinitely
* Perform heavy synchronous I/O

## Summary

Custom metrics extend BlazeBee beyond system monitoring into application-specific and domain-specific observability.

By implementing `DataProducer`, registering the collector, enabling it via features, and configuring it at runtime, developers can integrate bespoke metrics seamlessly into BlazeBee’s pipeline without modifying core logic.
