use std::path::PathBuf;

use blazebee_queue::prelude::*;
use bytes::Bytes;
use tokio::time::Instant;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("Blazebee Queue with Data Rotation usage");

    let mut config = QueueConfig::with_retention();
    config.data_dir = PathBuf::from("./queue_rotation_usage_v2");

    println!();

    // Rotation policy
    let retention = RetentionPolicy::default()
        .with_max_size(500 * 1024 * 1024) // 500MB max
        .with_max_messages(50_000) // 50k messages max
        .with_grace_period(5000); // 5 second grace

    println!("Retention Policy:");
    println!("Max size: 500 MB");
    println!("Max messages: 50,000");
    println!("Grace period: 5 seconds\n");

    let queue = Queue::new_with_retention(config, retention).await?;
    println!("Queue initialized with rotation policy\n");

    println!("Usage: Writing 5000 messages (will trigger automatic rotation)...\n");

    let mut offsets = Vec::new();
    let start = Instant::now();

    for batch in 0..50 {
        let mut messages = Vec::new();
        for i in 0..100 {
            let msg_id = batch * 100 + i;
            messages.push(Bytes::from(format!(
                "Message {} [batch {}] with payload to increase message size: \
                 Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
                msg_id, batch
            )));
        }

        match queue.append_batch(messages).await {
            Ok(batch_offsets) => {
                offsets.extend(batch_offsets);
            }
            Err(e) => println!("Error writing batch {}: {}", batch, e),
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let elapsed = start.elapsed();
    println!("Write completed in {:?}", elapsed);
    println!(
        "  Throughput: {:.2} msg/sec\n",
        offsets.len() as f64 / elapsed.as_secs_f64()
    );

    // Test reads after rotation
    println!("Testing reads after rotation:");
    let test_offsets = vec![0, 100, 500, 2000, 4999];
    for offset in test_offsets {
        match queue.read(offset).await {
            Ok(Some(_)) => println!("Offset {} is available", offset),
            Ok(None) => println!("Offset {} was rotated out", offset),
            Err(e) => println!("Offset {} error: {}", offset, e),
        }
    }
    println!();

    println!("Initiating graceful shutdown...");
    queue.shutdown().await?;
    println!("Shutdown complete\n");

    println!("Rotation completed successfully!");
    println!("Data stored in: ./queue_rotation_usage_v2/partition-*/");

    Ok(())
}
