# Changelog

All notable changes to BlazeBee will be documented in this file.

## [Unreleased]

### Added
- Initial release of BlazeBee metrics collector
- Core collector framework with modular architecture
- MQTT transport implementation with v4 support
- Comprehensive set of system metric collectors

### Changed
- 

### Deprecated
- 

### Removed
- 

### Fixed
- 

### Security
- 

## [0.1.0] - 2026-01-01

### Added
- Initial public release of BlazeBee
- Core architecture with configurable collectors
- Support for multiple build types (minimal, standard, large)
- TOML-based configuration with validation
- Comprehensive logging with multiple output formats
- MQTT transport with configurable QoS and retention
- Support for JSON, MessagePack, and CBOR serialization
- Zstd compression for network payloads
- Cross-platform Docker images (AMD64, ARM64)
- Systemd journal integration
- Graceful shutdown handling
- Collector registry with runtime selection
- Multiple system metric collectors:
  - CPU usage and load metrics
  - Memory statistics and usage
  - Disk space and I/O metrics
  - Network interface statistics
  - Process information
  - System uptime tracking
  - Load average metrics
  - Virtual memory statistics
  - Network statistics (ARP, netstat, sockstat, softnet, conntrack)
  - Filesystem metrics
  - Systemd service states
  - NTP synchronization status
  - Thermal and hardware monitoring
  - Pressure stall information
  - Entropy pool statistics
  - File descriptor usage
  - Power management statistics
  - Error Detection and Correction (EDAC) metrics
  - Software RAID (mdraid) status

### Changed
- 

### Deprecated
- 

### Removed
- 

### Fixed
- 

### Security
- TLS support for MQTT connections
- Secure credential handling via environment variables
