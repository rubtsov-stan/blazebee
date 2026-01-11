# Quick Start Guide

Get BlazeBee up and running quickly with this step-by-step guide. This tutorial covers installation, basic configuration, and initial deployment scenarios.

## Prerequisites

Before installing BlazeBee, ensure you have:

- A Linux system with `/proc` and `/sys` filesystems mounted
- An accessible MQTT broker (for MQTT functionality)
- Rust 1.86+ (if building from source)
- Docker (for containerized deployment)

## Installation Options

BlazeBee offers multiple installation methods to suit different deployment scenarios:

### Docker Deployment (Recommended)

The easiest way to deploy BlazeBee is using our official Docker images:

```bash
# Pull the latest standard build
docker pull blazebee/blazebee:standard

# Run with host networking (for accessing local MQTT broker)
docker run --rm -it --network host blazebee/blazebee:standard
```

### Binary Installation

Download pre-built binaries for your architecture:

```bash
# For AMD64
wget https://github.com/blazebee/blazebee/releases/download/v0.1.0/blazebee-linux-amd64
chmod +x blazebee-linux-amd64

# For ARM64
wget https://github.com/blazebee/blazebee/releases/download/v0.1.0/blazebee-linux-arm64
chmod +x blazebee-linux-arm64
```

### Building from Source

Clone the repository and build:

```bash
git clone https://github.com/blazebee/blazebee.git
cd blazebee

# Build with standard features
make build TYPE=standard

# Or build with Docker
make docker TYPE=standard
```

## Basic Configuration

Create a basic configuration file to connect to your MQTT broker:

```toml
# config.toml
[transport]
host = "your-mqtt-broker.example.com"
port = 1883
client_id = "blazebee-${HOSTNAME}"

[metrics]
collection_interval = 5
refresh_interval = 60

# Enable basic metrics
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "metrics/cpu"
qos = 0
retain = true

[[metrics.collectors.enabled]]
name = "ram"
[metrics.collectors.enabled.metadata]
topic = "metrics/ram"
qos = 0
retain = true

[[metrics.collectors.enabled]]
name = "disk"
[metrics.collectors.enabled.metadata]
topic = "metrics/disk"
qos = 0
retain = true
```

## Running BlazeBee

### With Configuration File

```bash
# Using environment variable
BLAZEBEE_CONFIG=./config.toml ./blazebee

# Or mount config in Docker
docker run --rm -it \
  --network host \
  -v $(pwd)/config.toml:/etc/blazebee/config.toml:ro \
  blazebee/blazebee:standard
```

### With Different Build Types

Choose the appropriate build type for your needs:

- **Minimal**: Core metrics only (CPU, RAM, disk, network, processes, uptime, load average)
- **Standard**: Includes network statistics and additional collectors
- **Large**: Comprehensive monitoring including thermal, power, and advanced system metrics

```bash
# Use different Docker tags
docker run --rm -it --network host blazebee/blazebee:minimal
docker run --rm -it --network host blazebee/blazebee:large
```

## Verifying Installation

Once running, verify that BlazeBee connects to your MQTT broker and begins publishing metrics:

1. Check the logs for successful MQTT connection
2. Monitor your MQTT broker for messages on the configured topics
3. Look for metrics appearing on topics like `metrics/cpu`, `metrics/ram`, etc.

Example log output:
```
INFO Starting blazebee version 0.1.0...
INFO MQTT transport enabled
INFO Starting MQTT client...
INFO MQTT client started
INFO State monitoring started
INFO Starting metrics collection executor...
```

## Next Steps

After completing the quick start:

1. Customize your configuration to include additional metrics
2. Set up authentication for your MQTT broker
3. Configure TLS for secure communication
4. Explore advanced collector options
5. Set up monitoring dashboards to visualize the collected data

Continue reading the Architecture and Usage Guides sections for more detailed information on configuring and optimizing BlazeBee for your specific use case.