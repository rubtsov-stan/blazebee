# Usage Guides

This section provides detailed guidance on configuring and using BlazeBee for various monitoring scenarios. Each guide addresses specific use cases and configurations to help you get the most out of the system.

## Configuration Management

### Basic Configuration Structure

A typical BlazeBee configuration file uses TOML format and contains three main sections:

```toml
# Logging configuration
[logger]
level = "info"
timestamp_format = "unix"

[logger.console]
enabled = true
format = "compact"

# Transport configuration
[transport]
host = "mqtt.example.com"
port = 1883
client_id = "blazebee-${HOSTNAME}"

# Metrics configuration
[metrics]
collection_interval = 5
refresh_interval = 60
```

### Environment Variables

BlazeBee supports configuration via environment variables:

- `BLAZEBEE_CONFIG`: Path to configuration file
- `MQTT_PASSWORD`: MQTT broker password (more secure than config file)
- `${HOSTNAME}`: Dynamic hostname substitution in client ID

### Advanced Configuration Patterns

#### Conditional Configuration

Use different configurations based on environment:

```toml
[transport]
# Production
host = "prod-mqtt.example.com"
# Development
# host = "localhost"
```

#### Multiple Topic Publishing

Configure different collectors to publish to different topics:

```toml
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "servers/web01/cpu"
qos = 1
retain = true

[[metrics.collectors.enabled]]
name = "ram"
[metrics.collectors.enabled.metadata]
topic = "servers/web01/memory"
qos = 1
retain = true
```

## Deployment Strategies

### Containerized Deployment

Deploy BlazeBee using Docker with various strategies:

#### Host Network Mode

For direct access to system metrics and local MQTT broker:

```bash
docker run -d \
  --name blazebee \
  --network host \
  --restart unless-stopped \
  -v /etc/blazebee/config.toml:/etc/blazebee/config.toml:ro \
  blazebee/blazebee:standard
```

#### Privileged Container

For enhanced system access (if needed):

```bash
docker run -d \
  --name blazebee \
  --privileged \
  --restart unless-stopped \
  -v /etc/blazebee/config.toml:/etc/blazebee/config.toml:ro \
  blazebee/blazebee:standard
```

### Kubernetes Deployment

Deploy BlazeBee as a DaemonSet to monitor all cluster nodes:

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: blazebee-agent
spec:
  selector:
    matchLabels:
      name: blazebee-agent
  template:
    metadata:
      labels:
        name: blazebee-agent
    spec:
      hostNetwork: true
      containers:
      - name: blazebee
        image: blazebee/blazebee:standard
        env:
        - name: BLAZEBEE_CONFIG
          value: "/etc/blazebee/config.toml"
        volumeMounts:
        - name: config
          mountPath: /etc/blazebee/config.toml
          subPath: config.toml
      volumes:
      - name: config
        configMap:
          name: blazebee-config
```

### Systemd Service

Run BlazeBee as a systemd service:

```ini
[Unit]
Description=BlazeBee - Lightweight System Metrics Collector and MQTT Publisher
Documentation=https://rubtsov-stan.github.io/blazebee/
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/blazebee --config /etc/blazebee/config.toml
User=blazebee
Group=blazebee

NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=read-only
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_NETLINK
RestrictRealtime=true


WorkingDirectory=/var/lib/blazebee

ReadWritePaths=/var/lib/blazebee
ReadOnlyPaths=/etc/blazebee/config.toml

StandardOutput=journal
StandardError=journal
SyslogIdentifier=blazebee

LimitNOFILE=65535
LimitNPROC=512
TimeoutStartSec=30
TimeoutStopSec=10

Restart=on-failure
RestartSec=5s
StartLimitIntervalSec=60
StartLimitBurst=10

[Install]
WantedBy=multi-user.target
```

## Security Configuration

### MQTT Authentication

Configure secure MQTT connections:

```toml
[transport]
host = "secure-mqtt.example.com"
port = 8883  # TLS port
username = "blazebee-user"
# Use environment variable for password
# password = "secret"  # Not recommended

[transport.tls]
enabled = true
ca_file = "/etc/ssl/certs/ca-certificates.crt"
client_cert = "/etc/blazebee/client.crt"
client_key = "/etc/blazebee/client.key"
insecure_skip_verify = false
```

### Certificate-Based Authentication

For enhanced security, use certificate-based authentication:

```toml
[transport]
host = "mqtt.example.com"
port = 8883

[transport.tls]
enabled = true
ca_file = "/etc/ssl/certs/ca.pem"
client_cert = "/etc/ssl/certs/client-cert.pem"
client_key = "/etc/ssl/private/client-key.pem"
```

## Monitoring Scenarios

### Infrastructure Monitoring

Monitor critical infrastructure components:

```toml
[metrics]
collection_interval = 10  # Slightly longer for infrastructure

# Core metrics
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "infrastructure/hosts/${HOSTNAME}/cpu"

[[metrics.collectors.enabled]]
name = "ram"
[metrics.collectors.enabled.metadata]
topic = "infrastructure/hosts/${HOSTNAME}/memory"

[[metrics.collectors.enabled]]
name = "disk"
[metrics.collectors.enabled.metadata]
topic = "infrastructure/hosts/${HOSTNAME}/storage"
```

### IoT Device Monitoring

Optimize for resource-constrained devices:

```toml
# Use minimal build for IoT devices
[logger]
level = "warn"  # Reduce logging overhead

[metrics]
collection_interval = 30  # Less frequent collection
refresh_interval = 120

# Only essential metrics
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "iot/devices/${HOSTNAME}/cpu"

[[metrics.collectors.enabled]]
name = "ram"
[metrics.collectors.enabled.metadata]
topic = "iot/devices/${HOSTNAME}/memory"

[[metrics.collectors.enabled]]
name = "uptime"
[metrics.collectors.enabled.metadata]
topic = "iot/devices/${HOSTNAME}/uptime"
```

### High-Frequency Monitoring

For performance-critical applications:

```toml
[logger]
level = "error"  # Minimize logging impact

[metrics]
collection_interval = 1  # High-frequency collection
refresh_interval = 5

# Enable all performance-related collectors
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "performance/${HOSTNAME}/cpu"
qos = 2  # Highest reliability
```

## Data Serialization and Compression

### Format Selection

Choose the appropriate serialization format:

```toml
[transport.serialization]
format = "json"     # Human-readable, widely supported
# format = "msgpack" # Compact binary format
# format = "cbor"    # Even more compact, good for constrained networks

[transport.framing]
enabled = true  # Add framing for reliable transmission
```

### Compression Configuration

Configure compression for bandwidth optimization:

```toml
[transport.serialization]
format = "json"
compression = "zstd"              # Zstandard compression
compression_threshold = 1024      # Compress if payload > 1KB
```

## Troubleshooting and Debugging

### Log Analysis

Enable detailed logging for troubleshooting:

```toml
[logger]
level = "debug"  # More verbose logging

[logger.console]
enabled = true
format = "pretty"  # Human-readable format
show_target = true  # Show module names
```

### Collector Diagnostics

Identify problematic collectors:

```toml
# Temporarily disable collectors to isolate issues
[metrics]
collection_interval = 30  # Slower collection during debugging

# Enable only essential collectors
# Comment out others during troubleshooting
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic = "debug/${HOSTNAME}/cpu"
```

## Performance Tuning

### Resource Optimization

Adjust collection intervals based on system resources:

- **High-resource systems**: Shorter intervals (1-5 seconds)
- **Medium-resource systems**: Standard intervals (5-15 seconds)  
- **Low-resource systems**: Longer intervals (30-60 seconds)

### Collector Selection

Choose collectors based on your monitoring needs:

- **Basic monitoring**: cpu, ram, disk, network, uptime
- **Network monitoring**: Add netstat, sockstat, arp
- **Hardware monitoring**: Add thermal, hwmon, power
- **Advanced monitoring**: Add pressure, edac, mdraid

These usage guides provide practical examples for deploying BlazeBee in various scenarios. Adjust configurations based on your specific requirements and system constraints.