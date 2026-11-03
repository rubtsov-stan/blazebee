//! BlazeBee: a Linux system metrics collector that publishes to MQTT.
//!
//! * [`config`] loads and validates the TOML configuration.
//! * [`core::collectors`] reads kernel interfaces (`/proc`, `/sys`) and turns
//!   them into serializable samples; every collector is behind a Cargo feature.
//! * [`core::scheduler`] samples all enabled collectors at a fixed interval
//!   and publishes the results while the broker connection is up.
//! * [`logger`] configures `tracing` output for the console and journald.
//!
//! The binary in `src/main.rs` wires these together on a single-threaded
//! Tokio runtime.

pub mod config;
pub mod core;
pub mod logger;
