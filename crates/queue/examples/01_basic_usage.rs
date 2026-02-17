use std::path::PathBuf;

use blazebee_queue::prelude::*;
use bytes::Bytes;
use tokio::time::Instant;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("Blazebee Queue usage");

    let config = QueueConfig::new(PathBuf::from("./queue_usage"))
        .with_segment_size(10 * 1024 * 1024)
        .with_fsync_policy(FsyncPolicy::Immediate);

    let queue = Queue::new(config).await?;
    println!("Queue initialized");

    println!("Usage 1: Single message append");
    let msg = Bytes::from("Hello, persistent queue!");
    let offset = queue.append(msg).await?;
    println!("Message appended at offset: {}\n", offset);

    println!("Usage 2: Batch append (1000 messages)");
    let mut messages = Vec::new();
    for i in 0..1000 {
        messages.push(Bytes::from(format!("Message {}", i)));
    }

    let start = Instant::now();
    let offsets = queue.append_batch(messages).await?;
    let elapsed = start.elapsed();

    println!(" {} messages appended in {:?}", offsets.len(), elapsed);
    println!(
        "  Throughput: {:.2} msg/sec\n",
        offsets.len() as f64 / elapsed.as_secs_f64()
    );

    println!("Usage 3: Read message");
    if let Some(data) = queue.read(0).await? {
        println!(
            "Read message at offset 0: {}\n",
            String::from_utf8_lossy(&data)
        );
    }

    println!("Initiating graceful shutdown...");
    queue.shutdown().await?;
    println!("Queue shutdown complete\n");

    println!("Completed successfully!");

    Ok(())
}
