---
title: Architecture
description: Detailed breakdown of BlazeBee’s internal structure, components, and data flow.
---

### Overview

BlazeBee is implemented in Rust with a focus on performance, modularity, and safe concurrency. It is structured into clearly separated layers: configuration, metric collectors, transport, and execution. This organization supports high throughput metric collection and reliable publishing to MQTT brokers, with extensible components that can be enabled or disabled at build time and run time. The architectural description below is based on the repository layout, README contents, and configuration artifacts in the project.

### Layered Structure

### Configuration Layer

**Role:** Centralizes configuration for the entire system.

- Parses a TOML file (`config.example.toml`) to build an in-memory settings object that drives the behavior of other layers.  
- Validates required sections such as logging, transport, and enabled collectors before runtime begins.  
- Exposes structured config objects to the collection and transport subsystems.  
- Supports environment variable substitution for sensitive values (e.g., MQTT credentials).

**Primary responsibilities:**

- Logging settings (level, output format, console vs journal).  
- MQTT connection parameters (host, port, client ID, keep-alive, TLS settings).  
- Collector enablement and per-collector metadata (topic, QoS, retain flags).  
- Metric collection intervals.

### Collection Layer

**Role:** Encapsulates metric-specific gathering logic into modular units.

- Each collector implements a defined interface for retrieving system metrics (CPU, memory, disk, network, processes, uptime, advanced stats).  
- The registry of active collectors is derived from configuration; only enabled collectors are instantiated at startup.  
- Metric functions read from system data sources (e.g., `/proc`, sysfs) to populate domain-specific structures.  
- Collectors run independently under the async runtime to maximize concurrency and minimize interference.

**Characteristics:**

- Lightweight Rust implementations aiming for minimal CPU and memory overhead.  
- Metric categories separated into core and advanced sets; advanced collectors may require elevated permissions.  
- Collector metadata (MQTT topic, QoS, retain) configured per collector instance.

### Transport Layer

**Role:** Handles network I/O and message delivery to the MQTT broker.

- Based on the `rumqttc` Rust crate for MQTT client functionality.
- Establishes and maintains TCP/TLS connections to the MQTT broker according to configured parameters.  
- Serializes metric payloads to chosen formats (JSON by default; could be extended to others).  
- Performs publish operations with appropriate MQTT settings: topic, Quality of Service (QoS), retain flags.

**Connections and failure management:**

- Maintains reconnect logic and keep-alive heartbeats to preserve broker session.  
- Errors during publishing are logged and isolated; transport failures do not crash the entire system.

### Execution Layer

**Role:** Drives concurrent operation of collectors and transport.

- Uses Rust’s `tokio` asynchronous runtime for scheduling and execution without blocking.  
- Timer-driven loops invoke collectors at configured intervals.  
- Async tasks manage MQTT communication in parallel with metric collection.  
- Graceful shutdown signals are propagated to stop tasks cleanly and close connections.

### Data Flow

1. **Initialization**

   - Read and validate the configuration file.  
   - Construct collector and transport subsystems based on config.  
   - Initialize the async runtime and logging according to settings. 

2. **Collector Scheduling**

   - For each enabled collector, schedule periodic execution using async timers.  
   - Collectors fetch raw system data at configured intervals.  
   - Data is packaged into metric structures.

3. **Serialization & Publishing**

   - Serialize metric data to the configured format.  
   - Hand off serialized payloads to the transport layer.  
   - Transport layer publishes to designated MQTT topics with configured QoS and retain settings.

4. **Error Isolation**

   - Individual collector errors are contained and logged; they do not stop others from running.  
   - MQTT errors are logged, with reconnection attempts; transport layer resilience prevents systemic failure.

### Component Interactions

- **Configuration → Collectors:** config determines which collectors are active and supplies metadata for publishing.
- **Collectors → Transport:** metric data flows from collector tasks into the MQTT client for delivery based on configured topics and flags.   
- **Runtime → All Layers:** `tokio` runtime schedules tasks across collectors and transport concurrently.

### Extensibility

- New collectors can be added by implementing the collector trait and registering them in the build system.
- Transport layer abstraction allows future support for alternative protocols beyond MQTT.

### Goals of Architecture

- Minimize resource usage while maintaining responsiveness.
- Support flexible configuration for diverse deployment models (edge, cloud, IoT).
- Ensure safety through Rust’s type and memory guarantees.

### Summary

BlazeBee’s architecture separates key concerns—configuration, data collection, transport, and execution—into distinct layers that interact through well-defined interfaces. The design supports secure, concurrent metric gathering and efficient delivery over MQTT, with resilience against individual component failures and flexible extensibility for custom use cases.

