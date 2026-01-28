---
title: Kubernetes Deployment
description: Deploy BlazeBee on Kubernetes for scalable, managed orchestration in cloud-native environments.
---

BlazeBee is designed to run reliably inside Kubernetes clusters with minimal operational overhead. Its stateless runtime model, externalized configuration, and MQTT-based transport make it suitable for both centralized and node-level metric collection in cloud-native environments.

This page describes recommended Kubernetes deployment patterns, configuration strategies, and operational considerations.

## Purpose

This guide explains:
- How to deploy BlazeBee in Kubernetes
- How configuration is managed using ConfigMaps and Secrets
- When to use Deployment vs DaemonSet
- How BlazeBee integrates with MQTT services inside a cluster

## Prerequisites

### Required

- A running Kubernetes cluster  
  - Local: Minikube, Kind  
  - Managed: EKS, GKE, AKS, etc.
- `kubectl` configured for the target cluster
- BlazeBee container image available in a registry accessible by the cluster

### Optional

- MQTT broker deployed inside the cluster
- Persistent storage for the broker
- Kubernetes Secrets for credentials

## Image Selection

Choose an image variant appropriate for your use case:

- `rubtsovs/blazebee:standard-amd64` — recommended default
- `rubtsovs/blazebee:large-amd64` — advanced collectors
- Architecture-specific tags for non-amd64 nodes

The image must include all collectors required by the configuration.

## Basic Deployment Model

The simplest deployment uses a single replica BlazeBee instance publishing cluster-level or host-level metrics.

### Deployment Manifest

Create `blazebee-deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: blazebee
  labels:
    app: blazebee
spec:
  replicas: 1
  selector:
    matchLabels:
      app: blazebee
  template:
    metadata:
      labels:
        app: blazebee
    spec:
      containers:
        - name: blazebee
          image: rubtsovs/blazebee:latest-amd64
          imagePullPolicy: IfNotPresent
          volumeMounts:
            - name: config
              mountPath: /etc/blazebee/config.toml
              subPath: config.toml
      volumes:
        - name: config
          configMap:
            name: blazebee-config
````

This deployment:

* Runs a single BlazeBee instance
* Loads configuration from a ConfigMap
* Does not require persistent storage

## Configuration via ConfigMap

### ConfigMap Definition

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: blazebee-config
data:
  config.toml: |
    [transport]
    host = "mqtt-service"
    port = 1883

    [[metrics.collectors.enabled]]
    name = "cpu"
    [metrics.collectors.enabled.metadata]
    topic = "metrics/cpu"
    qos = 0
    retain = true

    [[metrics.collectors.enabled]]
    name = "ram"
    [metrics.collectors.enabled.metadata]
    topic = "metrics/ram"
    qos = 0
    retain = true

    [[metrics.collectors.enabled]]
    name = "disk"
    [metrics.collectors.enabled.metadata]
    topic = "metrics/disk"
    qos = 0
    retain = true

    [[metrics.collectors.enabled]]
    name = "network"
    [metrics.collectors.enabled.metadata]
    topic = "metrics/network"
    qos = 0
    retain = true
```

### Configuration Notes

* Configuration is immutable at runtime
* Changes require pod restart
* Topic naming should reflect cluster and node identity if multiple instances are used

## Applying the Deployment

```bash
kubectl apply -f blazebee-deployment.yaml
```

Verify:

```bash
kubectl get pods
kubectl logs deployment/blazebee
```

## DaemonSet Deployment (Node-Level Metrics)

For per-node system metrics, BlazeBee should be deployed as a DaemonSet.

### When to Use DaemonSet

* Collect metrics from every node
* Monitor hardware, kernel, or filesystem state
* Avoid metric aggregation gaps during node scaling

### DaemonSet Example

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: blazebee
spec:
  selector:
    matchLabels:
      app: blazebee
  template:
    metadata:
      labels:
        app: blazebee
    spec:
      containers:
        - name: blazebee
          image: rubtsovs/blazebee:standard-amd64
          volumeMounts:
            - name: config
              mountPath: /etc/blazebee/config.toml
              subPath: config.toml
      volumes:
        - name: config
          configMap:
            name: blazebee-config
```

Each node runs exactly one BlazeBee instance.

## Secrets Management

MQTT credentials should not be stored in ConfigMaps.

### Secret Definition

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: blazebee-secrets
type: Opaque
data:
  mqtt-username: <base64>
  mqtt-password: <base64>
```

### Injecting Secrets

```yaml
env:
  - name: MQTT_USERNAME
    valueFrom:
      secretKeyRef:
        name: blazebee-secrets
        key: mqtt-username
  - name: MQTT_PASSWORD
    valueFrom:
      secretKeyRef:
        name: blazebee-secrets
        key: mqtt-password
```

These variables can be referenced inside `config.toml`.

## Resource Management

BlazeBee has a low runtime footprint.

Recommended defaults:

```yaml
resources:
  requests:
    cpu: "50m"
    memory: "64Mi"
  limits:
    cpu: "200m"
    memory: "256Mi"
```

Adjust based on:

* Number of enabled collectors
* Collection frequency
* Cluster size

## MQTT Integration

### Broker Deployment

MQTT broker should be deployed as:

* Kubernetes Service
* StatefulSet or Deployment

Example service hostname:

```toml
[transport]
host = "mqtt-service"
port = 1883
```

### Networking

* BlazeBee communicates with the broker over the cluster network
* No ports need to be exposed externally
* TLS is recommended for multi-tenant clusters

## Logging and Observability

* Logs are written to STDOUT
* Accessible via `kubectl logs`
* Designed for integration with cluster-wide log aggregation

Example:

```bash
kubectl logs -l app=blazebee
```

## Rolling Updates

* Configuration changes require pod restart
* Deployment updates trigger rolling replacement
* DaemonSet updates roll node by node

## Operational Considerations

* Advanced collectors may require:

  * Host access (`/proc`, `/sys`)
  * Privileged pods
* Linux-only support
* Clock synchronization recommended for accurate timestamps

## Summary

BlazeBee integrates cleanly into Kubernetes using standard primitives. Deployment and DaemonSet modes cover both centralized and node-level monitoring scenarios. Externalized configuration, MQTT-based transport, and low resource usage make BlazeBee suitable for production-grade cloud-native environments.
