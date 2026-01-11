# BLAZEBEE MQTT-v4 Manager

A production-ready async MQTT client for Rust with automatic reconnection, multiple serialization formats, and comprehensive error handling. Built on top of `rumqttc` to provide a higher-level, more ergonomic interface for MQTT operations.

## Features

- **Automatic Reconnection** - Exponential backoff with configurable limits
- **Connection State Monitoring** - Watch channel for real-time connection state updates
- **Multiple Serialization Formats** - JSON, MessagePack, CBOR support
- **Compression Support** - Gzip and Zstd compression for efficient bandwidth usage
- **TLS/SSL** - Client certificate authentication and CA verification
- **Configuration Management** - Load from TOML, JSON, YAML with validation
- **Comprehensive Error Handling** - Categorized error types for proper recovery
- **Async/Await** - Full tokio integration with non-blocking operations
- **Subscription Persistence** - Automatic resubscription on reconnection

## Quick Start

### Basic Setup

Add to your `Cargo.toml`:

```toml
[dependencies]
blazebee-mqtt-v4 = "0.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

### Minimal Example

```rust
use blazebee_mqtt_v4::{MqttManager, EndpointMetadata, Publisher};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct SensorReading {
    temperature: f32,
    humidity: f32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create MQTT manager
    let manager = MqttManager::new("localhost", 1883)?;
    let instance = std::sync::Arc::new(manager.build_and_start().await?);

    // Create publisher
    let publisher = Publisher::new(instance.clone());

    // Subscribe to topics
    instance.subscribe("sensor/+/data").await?;
    instance.start_monitoring().await?;

    // Publish a message
    let reading = SensorReading {
        temperature: 22.5,
        humidity: 55.0,
    };

    let metadata = EndpointMetadata {
        qos: 1,
        topic: "sensor/living_room".into(),
        retain: false,
    };

    publisher.publish(&reading, &metadata).await?;

    // Graceful shutdown
    instance.shutdown().await?;
    Ok(())
}
```

## Configuration

### From TOML File

Create `mqtt.toml`:

```toml
base_topic = "my-app"
host = "mqtt.example.com"
port = 8883
connection_timeout = 30
clean_session = false
keep_alive = 60
max_inflight = 100
reconnect_delay = 5
max_reconnect_attempts = 0

[tls]
ca_cert_path = "/etc/mqtt/ca.pem"
client_cert_path = "/etc/mqtt/client.crt"
client_key_path = "/etc/mqtt/client.key"

[serialization]
format = "msgpack"
compression = "zstd"
compression_threshold = 1024
```

Load it:

```rust
let config_text = std::fs::read_to_string("mqtt.toml")?;
let config: Config = toml::from_str(&config_text)?;
let manager = MqttManager::from_config(config)?;
```

### Programmatic Configuration

```rust
use blazebee_mqtt_v4::{Config, TlsConfig};

let config = Config {
    base_topic: "iot-app".into(),
    host: "broker.hivemq.com".into(),
    port: 8883,
    connection_timeout: 30,
    clean_session: false,
    max_inflight: 50,
    keep_alive: 60,
    client_id: "device_001".into(),
    max_packet_size: Some(65535),
    request_channel_capacity: Some(100),
    reconnect_delay: 5,
    max_reconnect_attempts: 0,
    reconnect_backoff_delimiter: 1,
    tls: Some(TlsConfig::with_ca_only("/etc/mqtt/ca.pem")),
};

let manager = MqttManager::from_config(config)?;
```

## Usage Patterns

### Pattern 1: Monitoring Connection State

```rust
use mqtt_manager::ConnectionState;

let mut state_rx = instance.supervisor().state_receiver();

tokio::spawn(async move {
    loop {
        if state_rx.changed().await.is_err() {
            break;
        }

        match state_rx.borrow().clone() {
            ConnectionState::Connected => {
                println!("Online");
            }
            ConnectionState::Disconnected(reason) => {
                eprintln!("Offline: {}", reason);
            }
            ConnectionState::Reconnecting(secs) => {
                println!("Reconnecting in {:.1}s", secs);
            }
            _ => {}
        }
    }
});
```

### Pattern 2: Publishing with Retry

```rust
// Simple publish with automatic retry on failure
async fn publish_with_retry(
    publisher: &Publisher,
    data: &SensorReading,
    metadata: &EndpointMetadata,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut delay = std::time::Duration::from_millis(100);
    
    for attempt in 1..=5 {
        match publisher.publish(data, metadata).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 5 => {
                eprintln!("Attempt {} failed: {}", attempt, e);
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }
}
```

### Pattern 3: Publisher Presets

Choose a preset configuration optimized for your use case:

```rust
// Development/debugging: JSON, no compression
let publisher = mqtt_manager::publisher::presets::minimal(instance);

// Compact: MessagePack, no compression
let publisher = mqtt_manager::publisher::presets::compact(instance);

// Balanced: JSON + Gzip
let publisher = mqtt_manager::publisher::presets::efficient(instance);

// High-throughput: MessagePack + Zstd
let publisher = mqtt_manager::publisher::presets::high_performance(instance);

// Ultra-compact: CBOR + Zstd
let publisher = mqtt_manager::publisher::presets::ultra_compact(instance);
```

### Pattern 4: Custom Serialization

```rust
use mqtt_manager::config::{SerializationConfig, SerializationFormat, CompressionType};

let config = SerializationConfig {
    format: SerializationFormat::MessagePack,
    compression: CompressionType::Zstd,
    compression_threshold: 512,
};

let publisher = Publisher::with_config(instance, &config);
```

## Architecture

The library follows a layered architecture:

```
Application Code
    ↓
MqttInstance (high-level API)
    ├─ Publisher (serialization, compression, framing)
    ├─ Supervisor (lifecycle management)
    └─ ConnectionKernel (event loop, reconnection)
        ├─ Backoff (exponential backoff)
        └─ rumqttc AsyncClient + EventLoop
            ↓
        Network (TCP/TLS)
```

### Core Components

- **MqttManager** - Entry point, coordinates initialization
- **MqttInstance** - Active connection, provides public API
- **Publisher** - Handles serialization, compression, publishing
- **ConnectionKernel** - Manages event loop, reconnection, state
- **SubscriptionManager** - Tracks and resubscribes to topics
- **Supervisor** - Monitors connection lifecycle, publishes status
- **Backoff** - Exponential backoff algorithm with configurable limits
- **Framer** - Handles the message into frames

## Error Handling

The library uses a unified `TransferError` type that categorizes all failures:

```rust
use mqtt_manager::TransferError;

match publisher.publish(&data, &metadata).await {
    Ok(()) => println!("Success"),
    
    // Configuration errors (fix and restart)
    Err(TransferError::ClientSetup(msg)) => {
        eprintln!("Setup failed: {}", msg);
        std::process::exit(1);
    }
    
    // Data serialization errors (fix data)
    Err(TransferError::Serialization(msg)) => {
        eprintln!("Data too large: {}", msg);
    }
    
    // Network errors (will retry automatically)
    Err(TransferError::ClientConnection(e)) => {
        eprintln!("Network error: {}", e);
    }
    
    // Retry policy exhausted (critical)
    Err(TransferError::RetriesPolicy(e)) => {
        eprintln!("Cannot reconnect: {}", e);
        std::process::exit(1);
    }
    
    Err(e) => eprintln!("Other error: {}", e),
}
```

## QoS Guarantees

This library supports all three MQTT QoS levels:

| QoS | Name | Guarantee | Use Case |
|-----|------|-----------|----------|
| 0 | At Most Once | Fire-and-forget | Sensors with frequent updates |
| 1 | At Least Once | Delivered once or more | Important notifications |
| 2 | Exactly Once | Delivered exactly once | Critical transactions |

**Note**: QoS is per-message, set in `EndpointMetadata`. For critical data, always use QoS 1+.

## Connection Lifecycle

The connection goes through these states:

```
Connecting ──(CONNACK)──> Connected
                            ↓
                     (network error)
                            ↓
                       Disconnected
                            ↓
                      Reconnecting(N)
                            ↓
                        Connecting
```

**Reconnection Strategy**:
- Initial delay: Configurable (default 5s)
- Growth: Exponential with multiplier (default 1x)
- Maximum: 60 seconds (hard cap)
- Attempts: Configurable, 0 = unlimited (recommended)

## TLS/SSL Configuration

### CA-Only Verification (Most Common)

```rust
let tls = TlsConfig::with_ca_only("/etc/mqtt/ca.pem");
```

### Mutual TLS (Client Authentication)

```rust
let tls = TlsConfig::new(
    "/etc/mqtt/ca.pem",
    "/etc/mqtt/client.crt",
    "/etc/mqtt/client.key",
);
```

**Security Best Practices**:
- Keep private keys in secure storage (e.g., HashiCorp Vault)
- Set file permissions: `chmod 600` on key files
- Never commit keys to version control
- Rotate certificates before expiry
- Use strong key sizes (2048+ bits for RSA)

## Serialization Formats

Choose based on your requirements:

| Format | Size | Speed | Human-Readable | Use Case |
|--------|------|-------|---|---|
| JSON | Large (1.0x) | Slow | Yes | Debugging, development |
| MessagePack | Small (0.6x) | Fast | No | Production, bandwidth-limited |
| CBOR | Small (0.6x) | Medium | No | Interoperability |

**Compression** reduces size by 60-80% for text/structured data:
- **Gzip**: Standard, good compatibility
- **Zstd**: Faster, better ratio (recommended)

## Performance

Typical performance on modern hardware (Intel i7, 8GB RAM):

| Metric | Value |
|--------|-------|
| Throughput | 1,000-2,000 msg/sec |
| Latency (network) | 10-100 ms |
| Memory (idle) | 5-10 MB |
| Memory (per subscription) | ~100 bytes |
| CPU (idle) | < 1% |
| CPU (loaded) | < 5% |

**Optimization Tips**:
- Use MessagePack + Zstd for maximum throughput
- Increase `max_inflight` for bursty traffic
- Increase `request_channel_capacity` to buffer more locally
- Use QoS 0 only for high-frequency, low-importance data

## Thread Safety

All public types are safe for concurrent use:

```rust
let instance = Arc::new(instance);
let publisher = Publisher::new(instance.clone());

// Spawn multiple tasks publishing concurrently
for _ in 0..10 {
    let pub = publisher.clone();
    tokio::spawn(async move {
        // Safe to publish from multiple tasks
        pub.publish(&data, &metadata).await?;
    });
}
// or Spawn multiple tasks publishing concurrently with frame stream
for _ in 0..10 {
    let pub = publisher.clone();
    tokio::spawn(async move {
        // Safe to publish from multiple tasks
        pub.publish_framed(&data, &metadata).await?;
    });
}
```

## Examples

The repository includes several complete examples:

- `01_basic_publish_subscribe.rs` - Simple pub/sub with state monitoring
- `02_serialization_formats.rs` - Different serialization options
- `03_basic_publish_framed_msg.rs` - Different serialization options with frame stream

Run examples:

```bash
cargo run --example 01_basic_publish_subscribe
cargo run --example 02_serialization_formats
cargo run --example 03_basic_publish_framed_msg
```

## Testing

Run the test suite:

```bash
# Unit tests
cargo test --lib

# Integration tests (requires running MQTT broker)
cargo test --test integration_tests

# All tests
cargo test --all

# With output
cargo test -- --nocapture
```

## Troubleshooting

### Connection Refused

```
Error: Connection refused (os error 111)
```

**Solution**: Verify broker is running and host/port are correct.

```bash
# Test connectivity
telnet localhost 1883
```

### TLS Certificate Validation Failed

```
Error: TLS handshake failed
```

**Solution**: Verify CA certificate path and format (must be PEM).

```bash
# Inspect certificate
openssl x509 -in ca.pem -text -noout

# Verify format
file ca.pem  # Should be: PEM certificate
```

### Message Too Large

```
Error: Serialization error: message too large
```

**Solution**: Reduce data size or increase `max_packet_size` in config.

```rust
config.max_packet_size = Some(262144);  // 256KB
```

### High CPU Usage

**Solution**: Enable compression, reduce publish rate, or profile with `perf`.

```rust
let config = SerializationConfig {
    format: SerializationFormat::MessagePack,
    compression: CompressionType::Zstd,
    compression_threshold: 512,
};
```

### Memory Growth

**Solution**: Check for message buffering or subscription memory leaks.

```bash
# Monitor memory
watch -n 1 'ps aux | grep mqtt'

# Profile with valgrind
valgrind --leak-check=full ./target/debug/your_app
```


## Acknowledgments

Built on top of the excellent [rumqttc](https://github.com/bytebeamio/rumqttc) library, which provides the underlying MQTT protocol implementation.

## Support

- **Examples**: See `/examples` directory

## Changelog

### v0.1.0 (Initial Release)

- Core MQTT functionality with automatic reconnection
- Multiple serialization formats (JSON, MessagePack, CBOR)
- Compression support (Gzip, Zstd)
- TLS/SSL with mutual authentication
- Comprehensive error handling
- Connection state monitoring
- Configuration management with TOML support

## Roadmap

Planned for future releases:

- Message buffering with persistence
- Metrics collection (Prometheus integration)
- Circuit breaker pattern
- Multiple broker failover
- Advanced diagnostics and debugging tools
- Performance optimizations

---
