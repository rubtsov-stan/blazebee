---
title: Linux Install
description: Install and run BlazeBee natively on Linux systems for direct host integration.
---


BlazeBee can be deployed directly on Linux hosts without containers, providing maximal visibility into the operating system with minimal overhead. Native deployment is recommended for bare-metal servers, virtual machines, and environments where direct access to kernel and hardware interfaces is required.

This page documents installation methods, filesystem layout, and systemd-based service management.

## Purpose

This guide explains:
- How to install BlazeBee on Linux systems
- How to run it as a managed systemd service
- How configuration and permissions are handled
- How to operate BlazeBee in long-running production environments

## Prerequisites

### Required

- Linux distribution (Debian / Ubuntu recommended)
- Systemd
- MQTT broker reachable from the host
- Root or sudo access for installation

### Optional

- Dedicated system user for BlazeBee
- Journald for centralized logging

## Installation Methods

### Debian Package Installation

BlazeBee provides prebuilt `.deb` packages for supported releases.

```bash
wget https://github.com/rubtsov-stan/blazebee/releases/download/vX.Y.Z/blazebee.deb
sudo dpkg -i blazebee.deb
````

This installs:

* Binary: `/usr/local/bin/blazebee`
* Default directories:

  * `/etc/blazebee/`
  * `/var/lib/blazebee/`

Any missing dependencies must be resolved manually if the host system is minimal.

## Filesystem Layout

Recommended directory structure:

```text
/etc/blazebee/
  └── config.toml

/var/lib/blazebee/
  └── runtime data (if required)

/usr/local/bin/
  └── blazebee
```

The configuration file location is explicitly defined via environment variables.

## Systemd Integration

BlazeBee is designed to run as a long-lived system service under systemd.

### Installing the Unit File

A reference unit file is provided in the repository:

```bash
sudo cp contrib/systemd/blazebee.service /etc/systemd/system/
```

### Reload and Enable

```bash
sudo systemctl daemon-reload
sudo systemctl start blazebee
sudo systemctl enable blazebee
```

## Systemd Service Configuration

### Example Unit File

```ini
[Unit]
Description=BlazeBee - Lightweight System Metrics Collector and MQTT Publisher
Documentation=https://rubtsov-stan.github.io/blazebee/
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/blazebee
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"
Environment="BLAZEBEE_CONFIG=/etc/blazebee/config.toml"

KillMode=mixed
KillSignal=SIGTERM

User=blazebee
Group=blazebee

WorkingDirectory=/var/lib/blazebee
ReadWritePaths=/var/lib/blazebee
ReadOnlyPaths=/etc/blazebee/config.toml

StandardOutput=journal
StandardError=journal
SyslogIdentifier=blazebee

Restart=on-failure
RestartSec=5s
TimeoutStartSec=30
TimeoutStopSec=10

[Install]
WantedBy=multi-user.target
```

### Key Parameters Explained

* `BLAZEBEE_CONFIG`
  Explicit path to the configuration file.

* `KillSignal=SIGTERM`
  Enables graceful shutdown of collectors and MQTT connections.

* `Restart=on-failure`
  Ensures automatic recovery on crashes.

* `StandardOutput=journal`
  Routes logs into journald.

## Security Hardening

BlazeBee supports strict systemd sandboxing.

Common hardening options:

```ini
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
```

These options:

* Limit filesystem access
* Prevent privilege escalation
* Restrict kernel interaction

Some advanced collectors may require loosening these restrictions.

## User and Permissions

Recommended approach:

```bash
sudo useradd --system --home /var/lib/blazebee --shell /usr/sbin/nologin blazebee
sudo chown -R blazebee:blazebee /var/lib/blazebee
```

Collectors reading from `/proc` and `/sys` usually do not require root privileges, but hardware or power metrics may.

## Configuration Management

* Configuration file is read once at startup
* Changes require service restart:

```bash
sudo systemctl restart blazebee
```

* Configuration path is defined exclusively via `BLAZEBEE_CONFIG`

## Logs and Monitoring

### Viewing Logs

```bash
journalctl -u blazebee -f
```

Logs include:

* Startup validation
* Collector lifecycle
* MQTT connection state
* Runtime errors

## Operational Considerations

* Native deployment offers maximum metric fidelity
* Suitable for long-running production systems
* Ideal for environments requiring:

  * Hardware metrics
  * Kernel-level statistics
  * Minimal latency

## Summary

Native Linux deployment provides the highest level of observability and control over BlazeBee. With systemd integration, hardened execution, and explicit configuration management, BlazeBee can run reliably as a core monitoring component on Linux hosts in production environments.
