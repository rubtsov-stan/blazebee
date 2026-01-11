# Architecture & Design

Understanding the internal architecture of BlazeBee helps you configure and optimize it effectively for your specific use cases. This document details the core components, data flow, and design principles that make BlazeBee a reliable metrics collector.

## High-Level Architecture

BlazeBee follows a modular architecture with clear separation of concerns:

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Configuration │────│   Core Runtime   │────│   Publishers    │
│   (TOML)        │    │                  │    │   (MQTT)        │
└─────────────────┘    │  ┌─────────────┐ │    └─────────────────┘
                       │  │   Executor  │ │
                       │  └─────────────┘ │
                       │  ┌─────────────┐ │
                       │  │  Collectors │ │
                       │  │   Registry  │ │
                       │  └─────────────┘ │
                       │  ┌─────────────┐ │
                       │  │  Readiness  │ │
                       │  │   Manager   │ │
                       │  └─────────────┘ │
                       └──────────────────┘
```

## Core Components

### Configuration System

The configuration system manages all aspects of BlazeBee's behavior through a TOML-based configuration file. It includes:

- **Logging Configuration**: Console output formats, journald integration, and log levels
- **Transport Configuration**: MQTT connection parameters, TLS settings, and authentication
- **Metrics Configuration**: Collection intervals, enabled collectors, and metadata
- **Validation**: Built-in validation using the `validator` crate ensures configuration correctness

### Collector Framework

BlazeBee's collector system is its core strength, providing modular metric collection:

#### Collector Types

**Core Collectors** (Available in all builds):
- `cpu`: CPU utilization, frequency, and load metrics
- `ram`: Memory usage, swap statistics, and memory pressure
- `disk`: Storage space, I/O statistics, and disk usage
- `network`: Interface statistics, bandwidth, and connection metrics
- `processes`: Process counts, system load, and process states
- `uptime`: System uptime and boot time information
- `load_average`: Load average over 1, 5, and 15 minutes

**Advanced Collectors** (Standard and Large builds):
- `vmstat`: Virtual memory statistics and page cache information
- `arp`: ARP table and network mapping data
- `netstat`: Network connection statistics and socket information
- `sockstat`: Socket usage statistics
- `filesystem`: Detailed filesystem metrics and mount information
- `systemd`: Service state and unit information
- `ntp`: Time synchronization status and offset information

**Specialized Collectors** (Large builds):
- `thermal`: Temperature sensor readings and thermal zones
- `hwmon`: Hardware monitoring data from various sensors
- `pressure`: Pressure stall information for CPU, memory, and I/O
- `entropy`: Kernel entropy pool statistics
- `filefd`: File descriptor usage and limits
- `power`: Power management and battery statistics
- `edac`: Error Detection and Correction memory information
- `mdraid`: Software RAID status and statistics

### Executor Component

The executor orchestrates the entire collection process:

- **Scheduling**: Manages collection intervals and timing
- **Coordination**: Coordinates between collectors and publishers
- **Lifecycle Management**: Handles graceful startup and shutdown
- **Error Handling**: Implements robust error handling and recovery

### Publisher Abstraction

The publisher component abstracts the transport mechanism:

- **MQTT Publisher**: Primary transport for metrics publication
- **Serialization**: Support for JSON, MessagePack, and CBOR formats
- **Compression**: Zstd compression with configurable thresholds
- **QoS Control**: Configurable MQTT Quality of Service levels
- **Retain Flags**: Configurable message retention for persistent metrics

## Data Flow

### Collection Process

1. **Initialization**: Configuration is loaded and validated
2. **Collector Registration**: Enabled collectors are registered in the collector registry
3. **MQTT Connection**: Client connects to the configured MQTT broker
4. **Subscription Setup**: Subscriptions are established for configured topics
5. **Collection Loop**: Executors begin periodic collection cycles

### Metric Publication

1. **Collection**: Individual collectors gather metrics from system sources
2. **Processing**: Metrics are formatted according to serialization configuration
3. **Compression**: Data is compressed if size exceeds threshold
4. **Publication**: Metrics are published to MQTT topics with configured QoS
5. **Logging**: Publication events are logged for monitoring and debugging

## Design Principles

### Performance Optimization

BlazeBee implements several performance optimizations:

- **Zero-copy Serialization**: Efficient serialization using Serde
- **Async Processing**: Non-blocking I/O using Tokio runtime
- **Memory Efficiency**: Minimal allocations during collection cycles
- **Selective Loading**: Only enabled collectors consume resources

### Reliability Features

- **Graceful Shutdown**: Proper cleanup on SIGTERM/SIGINT
- **Connection Recovery**: Automatic reconnection to MQTT brokers
- **Error Isolation**: Collector failures don't affect other collectors
- **Health Monitoring**: Readiness state management for orchestration

### Security Considerations

- **TLS Support**: Encrypted connections to MQTT brokers
- **Authentication**: Username/password and certificate-based auth
- **Minimal Permissions**: Runs with least required privileges
- **Input Validation**: All configuration values are validated

## Build Variants

BlazeBee supports three build variants to optimize for different use cases:

### Minimal Build
- Core system metrics only
- Smallest binary size
- Lowest resource consumption
- Ideal for constrained environments

### Standard Build
- Core metrics plus network statistics
- Moderate binary size
- Balanced feature set
- Recommended for most deployments

### Large Build
- Complete set of collectors
- Largest binary size
- Comprehensive monitoring
- Suitable for full system visibility

## Platform Support

BlazeBee is designed for Linux systems and leverages:

- **Procfs/Sysfs**: Direct access to system metrics via `/proc` and `/sys`
- **Cross-compilation**: Support for multiple architectures (AMD64, ARM64)
- **Containerization**: Optimized for containerized deployments
- **Systemd Integration**: Journald logging and service management