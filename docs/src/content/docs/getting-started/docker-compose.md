---
title: Docker Compose Deployment
description: Deploy BlazeBee using Docker Compose in coordinated container environments.
---


Docker Compose deployment is the recommended approach for running BlazeBee in containerized infrastructures where multiple services must be orchestrated together. This includes local testing, edge gateways, and production environments where BlazeBee publishes metrics to an MQTT broker running either locally or externally.

This page describes a complete, reproducible deployment model using Docker Compose.

## Purpose

This guide explains:
- How to run BlazeBee as a container
- How to connect it to an MQTT broker
- How configuration and persistence are handled
- How to operate and observe the running service

## Prerequisites

### Required

- Docker (Engine)
- Docker Compose (v2 or compatible)
- BlazeBee configuration file (`config.toml`)

### Optional

- MQTT broker container (e.g. Eclipse Mosquitto)
- Persistent volumes for broker data and logs

## Image Selection

BlazeBee images are published per architecture and feature set.

Typical image variants:

- `rubtsovs/blazebee:latest-amd64`
- `rubtsovs/blazebee:standard-amd64`
- `rubtsovs/blazebee:large-amd64`

Select the image that matches:
- Target CPU architecture
- Required collector set
- Expected resource footprint

## Basic Docker Compose Setup

Create a `docker-compose.yml` file:

```yaml
version: "3.8"

services:
  blazebee:
    image: rubtsovs/blazebee:latest-amd64
    container_name: blazebee
    volumes:
      - ./config.toml:/etc/blazebee/config.toml:ro
    restart: unless-stopped
````

### Configuration Mount

* The configuration file **must** be mounted at `/etc/blazebee/config.toml`
* The file is read-only inside the container
* All runtime behavior is controlled through this file

### Startup

```bash
docker-compose up -d
```

After startup, BlazeBee immediately:

* Parses configuration
* Initializes collectors
* Connects to the configured MQTT broker
* Starts publishing metrics

## Deployment with Embedded MQTT Broker

If no external broker is available, an MQTT broker can be deployed alongside BlazeBee.

```yaml
version: "3.8"

services:
  mqtt-broker:
    image: eclipse-mosquitto
    container_name: mqtt-broker
    ports:
      - "1883:1883"
    volumes:
      - ./mosquitto.conf:/mosquitto/config/mosquitto.conf:ro
      - mosquitto-data:/mosquitto/data
      - mosquitto-logs:/mosquitto/log
    restart: unless-stopped

  blazebee:
    image: rubtsovs/blazebee:standard-amd64
    container_name: blazebee
    depends_on:
      - mqtt-broker
    volumes:
      - ./config.toml:/etc/blazebee/config.toml:ro
    restart: unless-stopped

volumes:
  mosquitto-data:
  mosquitto-logs:
```

### Broker Configuration Notes

* Expose port `1883` only if external access is required
* Use volumes to persist broker state
* Authentication and TLS should be configured for production use

## Environment Variables

Environment variables can be used for:

* Secrets injection
* Host-specific values
* Overriding static identifiers

Example:

```yaml
environment:
  - HOSTNAME=${HOSTNAME}
  - MQTT_PASSWORD=${MQTT_PASSWORD}
```

These values can be referenced inside `config.toml` using variable substitution.

## Networking

* Containers share a default Docker network
* BlazeBee connects to the broker using the service name as hostname
* Example broker host in `config.toml`:

```toml
[transport]
host = "mqtt-broker"
port = 1883
```

No additional network configuration is required for internal communication.

## Logs and Monitoring

### Viewing Logs

```bash
docker-compose logs blazebee
```

Logs include:

* Startup and configuration validation
* Collector execution
* Transport and publishing status
* Errors and warnings

### Log Destination

* Logs are written to STDOUT
* Intended to be collected by Docker logging drivers or external aggregators
* Journald logging is not used inside containers

## Updates and Restarts

* Configuration changes require container restart
* Image updates require pulling a new image and restarting
* Restart policy ensures automatic recovery on failure

```bash
docker-compose pull
docker-compose up -d
```

## Operational Considerations

* Ensure the container has access to required system interfaces if advanced collectors are enabled
* Some collectors may require:

  * Privileged mode
  * Additional mounts (`/proc`, `/sys`)
* Resource limits can be applied using `deploy.resources`

## Benefits of Docker Compose Deployment

* Deterministic and reproducible setup
* Clear separation of concerns
* Easy integration with MQTT brokers
* Suitable for development, staging, and production

## Summary

Docker Compose provides a clean and maintainable way to deploy BlazeBee in containerized environments. By combining explicit configuration, predictable networking, and restart policies, this approach ensures reliable metric collection and delivery across a wide range of infrastructures.


