//! Micro-benchmarks for the hot path of a collection cycle: parsing the
//! largest `/proc` files and encoding a sample for the wire.
//!
//! ```bash
//! cargo bench --features large
//! ```

use blazebee::core::collectors::{cpu, disk, netstat, network, ram, vmstat};
use blazebee_mqtt_v3::{
    serializer::Serializer, CompressionType, SerializationConfig, SerializationFormat,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

const STAT: &str = include_str!("../tests/fixtures/proc/stat");
const MEMINFO: &str = include_str!("../tests/fixtures/proc/meminfo");
const DISKSTATS: &str = include_str!("../tests/fixtures/proc/diskstats");
const NET_DEV: &str = include_str!("../tests/fixtures/proc/net/dev");
const VMSTAT: &str = include_str!("../tests/fixtures/proc/vmstat");
const NETSTAT: &str = include_str!("../tests/fixtures/proc/net/netstat");

fn parsers(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");
    for (name, content) in [
        ("proc_stat", STAT),
        ("meminfo", MEMINFO),
        ("diskstats", DISKSTATS),
        ("net_dev", NET_DEV),
        ("vmstat", VMSTAT),
        ("netstat", NETSTAT),
    ] {
        group.throughput(Throughput::Bytes(content.len() as u64));
        group.bench_function(name, |b| {
            b.iter(|| match name {
                "proc_stat" => drop(cpu::parse(black_box(content)).unwrap()),
                "meminfo" => drop(ram::parse(black_box(content)).unwrap()),
                "diskstats" => drop(disk::parse(black_box(content)).unwrap()),
                "net_dev" => drop(network::parse(black_box(content)).unwrap()),
                "vmstat" => drop(vmstat::parse(black_box(content))),
                _ => drop(netstat::parse(black_box(content))),
            })
        });
    }
    group.finish();
}

fn encoding(c: &mut Criterion) {
    let sample = vmstat::parse(VMSTAT);
    let mut group = c.benchmark_group("encode_vmstat");
    for (name, format, compression) in [
        ("json", SerializationFormat::Json, CompressionType::None),
        (
            "json_gzip",
            SerializationFormat::Json,
            CompressionType::Gzip,
        ),
        (
            "json_zstd",
            SerializationFormat::Json,
            CompressionType::Zstd,
        ),
        (
            "msgpack",
            SerializationFormat::MessagePack,
            CompressionType::None,
        ),
        ("cbor", SerializationFormat::Cbor, CompressionType::None),
    ] {
        let serializer = Serializer::new(&SerializationConfig {
            format,
            compression,
            compression_threshold: 0,
        });
        let size = serializer.encode(&sample, 0).unwrap().len();
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(name, |b| {
            b.iter(|| serializer.encode(black_box(&sample), 4096).unwrap())
        });
    }
    group.finish();
}

criterion_group!(benches, parsers, encoding);
criterion_main!(benches);
