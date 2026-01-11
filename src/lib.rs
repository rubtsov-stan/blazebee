//! blazebee — lightweight system metrics collector and MQTT publisher
//!
//! This crate provides a modular, efficient monitoring agent that collects
//! Linux system metrics from `/proc` and `/sys` filesystems and publishes them
//! via MQTT. It is designed for long-running operation with minimal resource
//! consumption, graceful shutdown support, and configurable logging.
//!
//! ## Modules
//!
//! * `config` — Configuration structures, loading, validation, and defaults.
//!   Supports TOML configuration files with validation via the `validator` crate.
//!
//! * `core` — Core runtime components:
//!   - Metric collection executor
//!   - Readiness state management
//!   - Publisher abstraction
//!   - Collector registry and traits
//!
//! * `logger` — Centralized logging initialization using `tracing`.
//!   Supports console output in multiple formats (compact, pretty, JSON)
//!   and optional systemd journald integration.
//!
//! ## Features
//!
//! * `balzebee-mqtt-v4` — Enables MQTT transport using the `blazebee_mqtt_v4` crate
//!   (default: enabled).
//!
//! Additional conditional features control individual collectors and
//! platform-specific functionality.

pub mod config;
pub mod core;
pub mod logger;
