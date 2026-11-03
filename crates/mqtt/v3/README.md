# blazebee-mqtt-v3

MQTT 3.1.1 transport used by [BlazeBee](../../../README.md). It wraps
[`rumqttc`](https://github.com/bytebeamio/rumqtt) with:

- a connection driver that reconnects forever with capped, jittered
  exponential backoff and stops only on unrecoverable errors (rejected
  credentials, TLS failures);
- a retained `{base_topic}/status` topic (`online` / `offline`) backed by a
  last-will message;
- a non-blocking publisher with JSON, MessagePack and CBOR encodings and
  optional gzip or zstd compression;
- optional framing for payloads larger than the configured packet size.

The crate is internal to the workspace and is not published. See
`examples/publish.rs` for the API in use:

```bash
cargo run -p blazebee-mqtt-v3 --example publish -- localhost 1883
```
