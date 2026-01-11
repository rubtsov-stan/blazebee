# BlazeBee: Lightning-Fast System Metrics Collector

[![License](https://img.shields.io/github/license/blazebee/blazebee)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.86+-orange.svg)](https://www.rust-lang.org/)
[![MQTT](https://img.shields.io/badge/mqtt-v4-blue)](https://mqtt.org/)
[![Docker](https://img.shields.io/badge/docker-automated-blue)](https://hub.docker.com/r/blazebee/blazebee)

> **Fast, lightweight, and reliable** - The Rust-powered system metrics collector that sends real-time data to your MQTT infrastructure.

## 🚀 Why BlazeBee?

BlazeBee is a next-generation system metrics collector designed for the modern infrastructure landscape. Built with Rust for memory safety and performance, it efficiently gathers Linux system metrics and publishes them via MQTT, providing real-time visibility into your system's performance.

### Key Advantages

- **⚡ Lightning Fast**: Written in Rust for optimal performance and minimal overhead
- **📦 Modular Design**: Enable only the collectors you need, reducing resource consumption
- **🔗 MQTT Native**: Built-in MQTT integration with configurable QoS and retention
- **⚖️ Lightweight**: Minimal resource footprint suitable for resource-constrained environments
- **🔧 Flexible**: Multiple build types (minimal, standard, large) for different use cases
- **🌐 Cross-Platform**: Runs on AMD64 and ARM64 architectures with Docker support

## 🎯 Perfect For

- **DevOps Teams**: Real-time infrastructure monitoring with minimal setup
- **IoT Deployments**: Lightweight monitoring for resource-constrained devices
- **Edge Computing**: Distributed monitoring with MQTT-based data aggregation
- **System Administrators**: Comprehensive system metrics without complexity
- **Cloud-Native Environments**: Containerized deployments with Kubernetes support

## 🛠️ Quick Start

### Docker Deployment (Recommended)

```bash
# Run with default configuration
docker run --rm -it --network host blazebee/blazebee:standard

# Mount custom configuration
docker run --rm -it \
  --network host \
  -v /path/to/config.toml:/etc/blazebee/config.toml:ro \
  blazebee/blazebee:standard
```

### Binary Installation

```bash
# Download and run (AMD64)
wget https://rubtsov-stan.github.io/blazebee/releases/download/v0.1.0/blazebee-linux-amd64
chmod +x blazebee-linux-amd64
./blazebee-linux-amd64
```

### Build from Source

```bash
git clone  https://rubtsov-stan.github.io/blazebee/blazebee.git
cd blazebee
make docker TYPE=standard
```

## 📊 Supported Metrics

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

## 🏗️ Architecture

BlazeBee follows a modular architecture that separates concerns:

- **Configuration Layer**: TOML-based configuration with validation
- **Collection Layer**: Modular collectors for different system metrics
- **Transport Layer**: MQTT-based publishing with configurable serialization
- **Execution Layer**: Async runtime for efficient metric collection

## 📚 Documentation

Complete documentation is available at [blazebee.github.io/docs](https://rubtsov-stan.github.io/blazebee) covering:

- [Installation Guide]( https://rubtsov-stan.github.io/blazebee/quickstart)
- [Configuration Reference]( https://rubtsov-stan.github.io/blazebee/architecture)
- [Deployment Scenarios]( https://rubtsov-stan.github.io/blazebee/guides)
- [Contributing Guidelines]( https://rubtsov-stan.github.io/blazebee/contributing)

## 🔒 Security & Reliability

- **TLS Support**: Encrypted MQTT connections for secure data transmission
- **Authentication**: Username/password and certificate-based authentication
- **Graceful Shutdown**: Proper cleanup on termination signals
- **Error Isolation**: Collector failures don't affect other collectors
- **Memory Safe**: Built with Rust to prevent memory-related vulnerabilities

## 🤝 Contributing

We welcome contributions from the community! Check out our [Contributing Guide]( https://rubtsov-stan.github.io/blazebee/contributing) to get started.

- **Bug Reports**: Please use the GitHub issue tracker
- **Feature Requests**: Open an issue to discuss new features
- **Code Contributions**: Fork, implement, and submit a pull request
- **Documentation**: Help improve our docs and examples

## 🆘 Support & Community

- **Documentation**: [https://rubtsov-stan.github.io/blazebee/docs]( https://rubtsov-stan.github.io/blazebee/docs)
- **Issues**: [GitHub Issues]( https://rubtsov-stan.github.io/blazebee/issues)
- **Discussions**: [GitHub Discussions]( https://rubtsov-stan.github.io/blazebee/discussions)

## 📄 License

BlazeBee is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

BlazeBee wouldn't be possible without the excellent Rust ecosystem and the following libraries:

- `rumqttc` - MQTT client implementation
- `tokio` - Async runtime
- `serde` - Serialization framework
- `tracing` - Application-level tracing

---

<div align="center">

**[🚀 Get Started Now]( https://rubtsov-stan.github.io/blazebee/quickstart)** &nbsp;&nbsp;|&nbsp;&nbsp; **[📖 Read Documentation]( https://rubtsov-stan.github.io/blazebee/docs)** &nbsp;&nbsp;|&nbsp;&nbsp; **[🐛 Report Issue]( https://rubtsov-stan.github.io/blazebee/issues)**

⭐ If you find BlazeBee useful, please give it a star!

</div>