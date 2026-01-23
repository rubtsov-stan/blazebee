# BlazeBee: Lightning-Fast System Metrics Collector

[![License](https://img.shields.io/github/license/rubtsov-stan/blazebee)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.86+-orange.svg)](https://www.rust-lang.org/)
[![MQTT](https://img.shields.io/badge/mqtt-v3-blue)](https://mqtt.org/)
[![Docker](https://img.shields.io/badge/docker-automated-blue)](https://hub.docker.com/r/blazebee/blazebee)

> **Fast, lightweight, and reliable** - The Rust-powered system metrics collector that sends real-time data to your MQTT infrastructure.

## Why BlazeBee?

BlazeBee is a next-generation system metrics collector designed for the modern infrastructure landscape. Built with Rust for memory safety and performance, it efficiently gathers Linux system metrics and publishes them via MQTT, providing real-time visibility into your system's performance.

### Key Advantages

- **Lightning Fast**: Written in Rust for optimal performance and minimal overhead
- **Modular Design**: Enable only the collectors you need, reducing resource consumption
- **MQTT Native**: Built-in MQTT integration with configurable QoS and retention
- **Lightweight**: Minimal resource footprint suitable for resource-constrained environments
- **Flexible**: Multiple build types (minimal, standard, large) for different use cases
- **Cross-Platform**: Runs on AMD64 and ARM64 architectures with Docker support

## Perfect For

- **DevOps Teams**: Real-time infrastructure monitoring with minimal setup
- **IoT Deployments**: Lightweight monitoring for resource-constrained devices
- **Edge Computing**: Distributed monitoring with MQTT-based data aggregation
- **System Administrators**: Comprehensive system metrics without complexity
- **Cloud-Native Environments**: Containerized deployments with Kubernetes support

### Build from Source

```bash
git clone  https://rubtsov-stan.github.io/blazebee/blazebee.git
cd blazebee
make docker TYPE=standard
```

## Supported Metrics

BlazeBee offers a comprehensive set of system metrics through its modular collector system:

### Core System Metrics
- **CPU**: Usage, frequency, load averages, and scheduling statistics
- **Memory**: RAM usage, swap statistics, and memory pressure
- **Disk**: Storage space, I/O statistics, and filesystem metrics
- **Network**: Interface statistics, bandwidth, and connection metrics
- **Processes**: Process counts, system load, and process states
- **Uptime**: System uptime and boot time information

### Advanced Metrics
- **Thermal**: Temperature sensors and thermal zones
- **Power**: Power management and battery statistics
- **Pressure**: Pressure stall information for CPU, memory, and I/O
- **Systemd**: Service states and unit information
- **NTP**: Time synchronization status and offset
- **Hardware**: Hardware monitoring via HWMON sensors
- **Network Stats**: ARP tables, netstat, sockstat, and connection tracking
- **File Descriptors**: Usage statistics and limits

## Architecture

BlazeBee follows a modular architecture that separates concerns:

- **Configuration Layer**: TOML-based configuration with validation
- **Collection Layer**: Modular collectors for different system metrics
- **Transport Layer**: MQTT-based publishing with configurable serialization
- **Execution Layer**: Async runtime for efficient metric collection

## Documentation

Complete documentation is available at [blazebee.github.io/docs](https://rubtsov-stan.github.io/blazebee)

## Security & Reliability

- **TLS Support**: Encrypted MQTT connections for secure data transmission
- **Authentication**: Username/password and certificate-based authentication
- **Graceful Shutdown**: Proper cleanup on termination signals
- **Error Isolation**: Collector failures don't affect other collectors
- **Memory Safe**: Built with Rust to prevent memory-related vulnerabilities

## License

BlazeBee is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

BlazeBee wouldn't be possible without the excellent Rust ecosystem and the following libraries:

- `rumqttc` - MQTT client implementation
- `tokio` - Async runtime
- `serde` - Serialization framework
- `tracing` - Application-level tracing
---
