# BlazeBee: Lightweight System Metrics Collector

BlazeBee is a high-performance, lightweight system metrics collector written in Rust that efficiently gathers Linux system metrics and publishes them via MQTT. Designed for reliability and minimal resource consumption, it's ideal for monitoring infrastructure in distributed systems and IoT environments.

## Overview

BlazeBee collects system metrics from `/proc` and `/sys` filesystems and publishes them through MQTT, providing real-time visibility into system performance. The agent is built with modularity in mind, allowing users to enable only the collectors they need while maintaining optimal performance.

## Key Features

- **Lightweight**: Minimal resource footprint optimized for long-running operations
- **Modular**: Selective collector activation based on your monitoring needs
- **MQTT Integration**: Built-in support for publishing metrics via MQTT protocol
- **Rust Powered**: Memory-safe, high-performance implementation
- **Flexible Configuration**: TOML-based configuration with validation
- **Multiple Build Types**: Minimal, standard, and large builds with varying collector sets
- **Cross-platform**: Runs on various Linux distributions and architectures
- **Production Ready**: Graceful shutdown, comprehensive logging, and systemd integration

## Supported Metrics

BlazeBee supports a comprehensive range of system metrics through its modular collector system:

### Core System Metrics
- CPU utilization and load averages
- Memory usage and statistics
- Disk space and I/O metrics
- Network interface statistics
- Process counts and information
- System uptime tracking

### Advanced Metrics
- Thermal sensors and hardware monitoring
- Pressure stall information
- Systemd service states
- NTP synchronization status
- Power management statistics
- File descriptor usage
- System entropy pool

## Why Choose BlazeBee?

BlazeBee stands out in the monitoring ecosystem by combining the performance benefits of Rust with the flexibility of MQTT publishing. Whether you're managing a small IoT fleet or a large-scale infrastructure, BlazeBee provides the right balance of features and efficiency.

The modular design allows you to customize the agent to your specific needs, reducing resource consumption while maintaining comprehensive monitoring capabilities.