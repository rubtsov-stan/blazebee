---
title: Linux Install
description: Run BlazeBee as a systemd service from the Debian package or a release binary.
---

## Debian package

Packages are published per release for amd64 and arm64 in three variants
(`minimal`, `standard`, `large`; see [Building](../development/build) for
the collector sets).

```bash
sudo apt install ./blazebee_<version>_<arch>.deb
```

The package installs:

| Path                                   | Purpose |
|----------------------------------------|---------|
| `/usr/bin/blazebee`                    | binary |
| `/etc/blazebee/config.toml`            | configuration (conffile, preserved on upgrade) |
| `/lib/systemd/system/blazebee.service` | unit, enabled and started on install |

Edit the configuration, at least `transport.host`, then restart:

```bash
sudo editor /etc/blazebee/config.toml
sudo systemctl restart blazebee
journalctl -u blazebee -f
```

Until a broker is reachable the service logs reconnection attempts and
takes no samples; it never exits because of an unreachable broker.

## Release binary

```bash
sudo install -m 0755 blazebee /usr/bin/blazebee
sudo install -d /etc/blazebee
sudo install -m 0644 config.example.toml /etc/blazebee/config.toml
sudo install -m 0644 contrib/systemd/blazebee.service /lib/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now blazebee
```

## The unit

`contrib/systemd/blazebee.service` runs the agent with `DynamicUser=yes`:
systemd allocates an unprivileged user at start, so no account needs to be
created. The sandbox grants read access to `/proc` and `/sys`, network
sockets, `AF_UNIX` (journald, D-Bus for the `systemd` collector) and the
`adjtimex` syscall for the `ntp` collector; everything else is denied.

`BLAZEBEE_CONFIG=/etc/blazebee/config.toml` is set in the unit. To use
another file, override it:

```bash
sudo systemctl edit blazebee
# [Service]
# Environment=BLAZEBEE_CONFIG=/srv/blazebee/config.toml
```

Credentials belong in the environment rather than the file:

```ini
[Service]
EnvironmentFile=/etc/blazebee/env
```

with `MQTT_PASSWORD=...` in `/etc/blazebee/env` (mode 0600) and
`password = "${MQTT_PASSWORD}"` in the configuration.

## Signals and exit codes

| Event                               | Behaviour |
|-------------------------------------|-----------|
| `SIGTERM`, `SIGINT`                 | publish `offline`, DISCONNECT, exit 0 |
| configuration or logging error      | message on stderr, exit 2 |
| unrecoverable broker error          | exit 1; `Restart=on-failure` restarts after 5 s |

## Logs

Logs go to the journal through stdout. Levels and per-module filters can
be raised without editing the file:

```bash
sudo systemctl edit blazebee
# [Service]
# Environment=RUST_LOG=debug,rumqttc=info
```

Each sampling cycle is logged at `debug` with the number of published and
failed collectors and the cycle duration.
