---
title: Custom Metrics
description: Adding a collector to the binary.
---

Collectors are part of the binary; there is no plugin mechanism. Adding one
takes a module, a registry entry and a Cargo feature.

## 1. Implement the collector

Create `src/core/collectors/mything.rs`:

```rust
use serde::Serialize;

use super::{
    procfs::{fields, next_field, ProcFile},
    sample, CollectFuture, Collector, CollectorResult,
};

const PATH: &str = "/proc/mything";

#[derive(Debug, Serialize)]
pub struct MyThing {
    pub value: u64,
}

pub struct MyThingCollector {
    file: ProcFile,
}

impl Default for MyThingCollector {
    fn default() -> Self {
        Self { file: ProcFile::new(PATH) }
    }
}

/// Pure parser; test it against captured kernel output.
pub fn parse(content: &str) -> CollectorResult<MyThing> {
    let mut f = fields(content);
    Ok(MyThing { value: next_field(&mut f, PATH, "value")? })
}

impl Collector for MyThingCollector {
    fn collect(&mut self) -> CollectFuture<'_> {
        Box::pin(async move { Ok(sample(parse(self.file.read()?)?)) })
    }
}
```

Rules the scheduler relies on:

- `collect` is called at most once per interval from a single task; keep
  buffers and state in `self`.
- Do not block the thread. Kernel file reads are fine; anything that can
  wait on hardware, the network or another process must use
  `tokio::task::spawn_blocking` or `tokio::process` with a deadline (see
  `filesys.rs` and `sysmd.rs`).
- Return an error instead of a partial or guessed sample. Errors are logged
  with rate limiting and do not affect other collectors.
- Serialize with `serde`; the output type can be any `Serialize + Send`.

## 2. Register it

In `src/core/collectors/mod.rs`, declare the module and add a registry
entry behind the feature:

```rust
#[cfg(feature = "collector-mything")]
pub mod mything;

// in REGISTRY
#[cfg(feature = "collector-mything")]
descriptor!("mything", mything::MyThingCollector),
```

In `Cargo.toml` add `collector-mything = []` and, if it belongs to a
preset, list it under `standard` or `large`.

## 3. Test it

Put a captured copy of the kernel file under `tests/fixtures/proc/` and
test `parse` against it, plus malformed input. `cargo test --features
collector-mything` must pass on Linux and on macOS; collectors that need a
Linux-only syscall are gated with `target_os = "linux"` in `mod.rs` (see
`ntp`).

## 4. Enable it

```toml
[[metrics.collectors.enabled]]
name = "mything"
[metrics.collectors.enabled.metadata]
topic = "metrics/mything"
```
