---
title: System Metrics
description: Core system metrics collected by BlazeBee.
---

System metrics form the default and recommended monitoring baseline in BlazeBee. These collectors provide essential visibility into system health, performance, and resource utilization with minimal overhead.

They are suitable for continuous operation in production environments.

## Purpose

This page documents the standard collectors that:

* Are enabled in typical deployments
* Have low performance impact
* Cover the most common operational needs

## Core System Collectors

### Uptime

**Collector name:** `uptime`

* Reports system uptime in seconds
* Derived from kernel uptime sources

**Use cases:**

* Detecting reboots
* Correlating restarts with incidents

---

### Load Average

**Collector name:** `load_average`

* Reports 1, 5, and 15 minute load averages
* Kernel-derived metrics

**Use cases:**

* Capacity planning
* Detecting sustained overload

---

### CPU

**Collector name:** `cpu`

* Per-core and aggregated CPU usage
* User, system, idle, iowait breakdown

**Use cases:**

* Performance analysis
* Detecting CPU saturation

---

### Memory (RAM)

**Collector name:** `ram`

* Total, used, free, cached memory
* Swap usage (if available)

**Use cases:**

* Memory leak detection
* Capacity monitoring

---

### Disk

**Collector name:** `disk`

* Disk usage per mount point
* Read/write statistics

**Use cases:**

* Storage capacity tracking
* IO bottleneck detection

---

### Network

**Collector name:** `network`

* Per-interface RX/TX counters
* Packet and error statistics

**Use cases:**

* Traffic analysis
* Detecting packet loss

---

### Processes

**Collector name:** `processes`

* Total running processes
* Blocked and zombie counts

**Use cases:**

* Detecting process leaks
* OS-level health checks

---

### Context Switches

**Collector name:** `vmstat`

* Voluntary and involuntary context switches
* Kernel scheduler activity

**Use cases:**

* Diagnosing contention
* Performance tuning

## Enabling System Metrics

System collectors are enabled using the same configuration mechanism:

```toml
[[metrics.collectors.enabled]]
name = "cpu"
[metrics.collectors.enabled.metadata]
topic  = "metrics/cpu"
qos    = 0
retain = true
```

Most system metrics are included in the `standard` feature set.

## Usage Scenarios

* Baseline infrastructure monitoring
* Alerting on thresholds (CPU, RAM, disk)
* Feeding dashboards and time-series databases
* Supporting SRE incident response

## Summary

System metrics provide reliable, low-cost observability into operating system behavior. They are the foundation of BlazeBee deployments and should be enabled in nearly all environments.

