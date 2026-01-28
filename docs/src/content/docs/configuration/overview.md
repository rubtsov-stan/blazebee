---
title: Configuration Guide
description: Overview of BlazeBee configuration
---
### Purpose of This Page
Details BlazeBee settings.

BlazeBee configuration is performed via a TOML file (`config.toml`). Use the provided `config.example.toml` as the reference. By default BlazeBee loads configuration from `/etc/blazebee/config.toml` or from a path set in the environment variable `BLAZEBEE_CONFIG`. 

### 1. Logging Configuration

```toml
[logger]
level            = "trace"                # Global log level: trace, debug, info, warn, error
timestamp_format = "Unix"                 # Format of timestamps in logs

[logger.console]
enabled         = true                   # Enable logging to console
format          = "compact"              # Format: compact, pretty, json
show_target     = false                  # Include module path
show_thread_ids = false
show_spans      = false
ansi_colors     = true                   # Enable ANSI colors

[logger.journald]
enabled    = false                      # Enable systemd journal logging
identifier = "blazebee"                 # Journal identifier
````

* `logger.level`: Controls verbosity. Use `info` for standard operation, `debug` for detailed output, `trace` for full tracing.
* `logger.console.*`: Options for console output formatting.
* `logger.journald`: Systemd journal integration; enable only on systems with systemd.([GitHub][1])

### 2. Transport (MQTT) Configuration

```toml
[transport]
host          = "localhost"             # MQTT broker address
port          = 1883                    # MQTT broker port
client_id     = "blazebee-${HOSTNAME}"   # Client ID; can use environment variables
keep_alive    = 60                      # Keep-alive interval in seconds
clean_session = false                   # MQTT clean session flag

[transport.serialization]
format = "json"                         # Message serialization format

[transport.framing]
enabled = true                          # Enable message framing
```

* `transport.host` / `transport.port`: Address and port of the MQTT broker.
* `transport.client_id`: Unique identifier for the MQTT client; typically includes the host name.
* `keep_alive`: Interval at which the MQTT keep-alive ping is sent.
* `clean_session`: Set to `false` to retain subscriptions across restarts.
* TLS and authentication can be configured but are commented out by default. Uncomment and set `tls`, `username`, and `password` fields for secured connections.([GitHub][1])

### 3. Metrics Collection and Publishing

```toml
[metrics]
collection_interval = 5                 # Frequency (seconds) to collect metrics
```

* `collection_interval`: Interval to trigger metric collection.

### 4. Enabled Collectors

Metrics collectors are specified under `[[metrics.collectors.enabled]]`. Each collector entry includes:

* `name`: Identifier of the metric collector.
* `metadata.topic`: MQTT topic where metrics are published.
* `metadata.qos`: MQTT Quality of Service (0, 1, 2).
* `metadata.retain`: MQTT retain flag for published messages.

Example:

```toml
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic  = "metrics/cpu"
qos    = 0
retain = true

[[metrics.collectors.enabled]]
name = "ram"
[metrics.collectors.enabled.metadata]
topic  = "metrics/ram"
qos    = 0
retain = true
```

* Define one `[[metrics.collectors.enabled]]` block per metric.
* Typical core collectors: `cpu`, `ram`, `disk`, `network`, `processes`, `uptime`, `load_average`, `vmstat`.
* Advanced collectors (often in “large” build): `pressure`, `thermal`, `hwmon`, `entropy`, `filefd`, `schedstat`, `filesystem`, `systemd`, `power`, `edac`, `mdraid`.
* To disable a collector, remove its block from the configuration.([GitHub][1])

### 5. Example Collector Block

```toml
[[metrics.collectors.enabled]]
name = "network"
[metrics.collectors.enabled.metadata]
topic  = "metrics/network"
qos    = 0
retain = true
```

* This block enables network metrics, publishing to the `metrics/network` topic.
* Set `retain = true` if latest metric must persist on broker for clients that connect later.([GitHub][1])

### 6. Environment Variables

* BlazeBee reads configuration from the file indicated by the `BLAZEBEE_CONFIG` environment variable.
* MQTT credentials and other sensitive values can be injected using environment variables in the config (for example `${HOSTNAME}`, `${MQTT_PASSWORD}`) to avoid plaintext secrets in TOML.([GitHub][1])

### 7. Deployment Notes

* Place the final configuration at `/etc/blazebee/config.toml` for packaged or container-based deployments.
* For Docker, mount the file into the container and set `BLAZEBEE_CONFIG` accordingly.
* Ensure logging settings match the intended runtime environment (console or journald).
* Adjust collection and refresh intervals based on infrastructure scale and metric granularity requirements.([GitHub][1])

This configuration guide reflects the structure and defaults of the `config.example.toml` file in the `rubtsov-stan/blazebee` repository.([GitHub][1])

[1]: https://raw.githubusercontent.com/rubtsov-stan/blazebee/main/config.example.toml "raw.githubusercontent.com"

