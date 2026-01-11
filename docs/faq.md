# FAQ & Common Scenarios

This section addresses frequently asked questions and common scenarios when using BlazeBee for system monitoring.

## Frequently Asked Questions

### General Questions

**Q: What systems does BlazeBee support?**
A: BlazeBee is designed for Linux systems that have `/proc` and `/sys` filesystems mounted. It supports various architectures including AMD64 and ARM64, making it suitable for servers, embedded devices, and IoT applications.

**Q: How does BlazeBee compare to other monitoring agents?**
A: BlazeBee offers several advantages:
- Written in Rust for memory safety and performance
- Modular collector system allowing minimal resource usage
- Direct access to system metrics via `/proc` and `/sys`
- Lightweight binary with small memory footprint
- Built-in MQTT integration for real-time metrics publishing

**Q: What are the resource requirements for BlazeBee?**
A: Resource requirements vary based on enabled collectors:
- Minimal build: ~5-10MB RAM, minimal CPU
- Standard build: ~10-20MB RAM, low CPU
- Large build: ~20-30MB RAM, moderate CPU during collection

**Q: Does BlazeBee support other protocols besides MQTT?**
A: Currently, BlazeBee focuses on MQTT as its primary transport mechanism. The architecture is designed to be extensible, so additional transport protocols could be added in the future.

### Configuration Questions

**Q: How do I configure multiple MQTT brokers?**
A: BlazeBee currently supports connecting to a single MQTT broker. For multiple destinations, you can use MQTT broker bridging or run multiple instances of BlazeBee.

**Q: Can I use environment variables in configuration files?**
A: Yes, BlazeBee supports environment variable substitution, such as `${HOSTNAME}` in the client_id field. Additionally, you can use the `BLAZEBEE_CONFIG` environment variable to specify the configuration file path.

**Q: How do I disable specific collectors?**
A: Simply remove or comment out the collector configuration from the `[[metrics.collectors.enabled]]` section in your configuration file.

**Q: What happens if a collector fails?**
A: BlazeBee implements error isolation - if one collector fails, it doesn't affect others. The failed collector will be logged, and collection continues for other enabled collectors.

### Performance Questions

**Q: How frequently should I collect metrics?**
A: Collection frequency depends on your use case:
- Infrastructure monitoring: 10-30 seconds
- Performance monitoring: 1-5 seconds
- IoT devices: 30-60 seconds or more
- Consider the trade-off between data granularity and resource usage

**Q: Can I reduce resource usage further?**
A: Yes, you can:
- Use the minimal build with only essential collectors
- Increase collection intervals
- Reduce logging level
- Use compression for network traffic

## Common Scenarios

### Scenario 1: Infrastructure Monitoring

**Use Case**: Monitoring a fleet of servers in a data center

**Configuration Approach**:
- Use standard or large build depending on required metrics
- Set collection interval to 10-15 seconds for real-time visibility
- Enable all relevant collectors for comprehensive monitoring
- Configure TLS for secure MQTT communication
- Use hostname-based topic structure for organization

**Example Configuration**:
```toml
[metrics]
collection_interval = 10
refresh_interval = 60

# Enable comprehensive monitoring
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "datacenter/servers/${HOSTNAME}/cpu"

[[metrics.collectors.enabled]]
name = "ram"
[metrics.collectors.enabled.metadata]
topic = "datacenter/servers/${HOSTNAME}/memory"

# Add other relevant collectors...
```

### Scenario 2: IoT Device Monitoring

**Use Case**: Monitoring resource-constrained IoT devices

**Configuration Approach**:
- Use minimal build to reduce binary size and memory usage
- Set longer collection intervals (30-60 seconds) to reduce resource usage
- Enable only essential collectors (CPU, RAM, disk, uptime)
- Use JSON format with compression for efficient network usage
- Consider using MQTT QoS 0 to reduce overhead

**Example Configuration**:
```toml
[logger]
level = "warn"  # Reduce logging overhead

[metrics]
collection_interval = 60  # Longer intervals for IoT
refresh_interval = 120

# Minimal collectors
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "iot/devices/${HOSTNAME}/cpu"
qos = 0  # Lower QoS for efficiency

[[metrics.collectors.enabled]]
name = "ram"
[metrics.collectors.enabled.metadata]
topic = "iot/devices/${HOSTNAME}/memory"
```

### Scenario 3: High-Frequency Performance Monitoring

**Use Case**: Monitoring performance-critical applications requiring high-resolution metrics

**Configuration Approach**:
- Use large build for comprehensive metrics
- Set short collection intervals (1-2 seconds)
- Enable pressure stall information and thermal metrics
- Use binary serialization (MessagePack/CBOR) for efficiency
- Configure higher QoS for reliability
- Monitor resource usage to avoid performance impact

**Example Configuration**:
```toml
[logger]
level = "error"  # Minimize logging impact

[metrics]
collection_interval = 1
refresh_interval = 5

# Enable all performance-related collectors
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "performance/${HOSTNAME}/cpu"
qos = 2  # Highest reliability

[[metrics.collectors.enabled]]
name = "pressure"
[metrics.collectors.enabled.metadata]
topic = "performance/${HOSTNAME}/pressure"
```

### Scenario 4: Edge Computing Monitoring

**Use Case**: Monitoring edge computing nodes with intermittent connectivity

**Configuration Approach**:
- Configure persistent sessions with `clean_session = false`
- Use retained messages for persistent state
- Implement local buffering if MQTT broker is unavailable
- Use longer keep-alive intervals to handle network instability
- Consider using a local MQTT broker as a relay

**Example Configuration**:
```toml
[transport]
host = "local-mqtt.example.com"
port = 1883
client_id = "edge-${HOSTNAME}"
keep_alive = 120  # Longer keep-alive for unstable connections
clean_session = false  # Preserve session state

[transport.serialization]
format = "json"
compression = "zstd"
compression_threshold = 512  # Compress smaller payloads
```

## Troubleshooting Common Issues

### Connection Problems

**Problem**: Unable to connect to MQTT broker
**Solutions**:
1. Verify broker hostname and port
2. Check firewall rules and network connectivity
3. Ensure TLS certificates are properly configured
4. Verify authentication credentials

**Problem**: Intermittent connection drops
**Solutions**:
1. Adjust keep_alive interval
2. Check broker load and capacity
3. Verify network stability
4. Consider using a more resilient MQTT broker setup

### Performance Issues

**Problem**: High CPU usage
**Solutions**:
1. Increase collection intervals
2. Disable unnecessary collectors
3. Check for collectors accessing slow filesystems
4. Consider using a more selective collector configuration

**Problem**: High memory usage
**Solutions**:
1. Use a minimal build
2. Reduce logging verbosity
3. Check for memory leaks in collectors
4. Monitor for any collectors with memory growth

### Data Issues

**Problem**: Missing metrics
**Solutions**:
1. Verify collector configuration and names
2. Check collector availability in the build
3. Ensure required system files are accessible
4. Review logs for collector errors

**Problem**: Inconsistent data
**Solutions**:
1. Check collection intervals for consistency
2. Verify timestamp synchronization
3. Review MQTT broker for message delivery issues
4. Consider QoS settings for message reliability

## Best Practices

### Security Best Practices

- Use TLS encryption for all MQTT communications
- Implement certificate-based authentication where possible
- Store sensitive information (passwords) in environment variables
- Run BlazeBee with minimal required privileges
- Regularly rotate authentication credentials

### Operational Best Practices

- Implement proper logging and monitoring of the monitoring system
- Use appropriate collection intervals for your use case
- Regularly update to the latest stable version
- Test configuration changes in a staging environment
- Plan for graceful updates and restarts

### Scalability Best Practices

- Distribute load across multiple MQTT brokers if needed
- Use topic hierarchies for efficient message routing
- Implement data retention policies in your MQTT broker
- Consider using MQTT broker clustering for high availability
- Monitor the monitoring system's resource usage

These scenarios and answers should help you effectively deploy and manage BlazeBee in various environments. Remember to test configurations in a non-production environment first.