//! Fixed-interval sampling loop.
//!
//! Delivery policy: metrics are *latest-value* samples. While the broker is
//! unreachable no sample is taken or queued; when the connection returns the
//! next cycle publishes fresh values. Samples are never buffered beyond the
//! transport's bounded queue, so memory stays flat during an outage. Skipped
//! cycles are counted and logged when collection resumes.
//!
//! Collectors run one after another on the runtime thread. Each call is
//! bounded by [`COLLECT_TIMEOUT`]; a slow collector delays the rest of the
//! cycle but can never stall the process. If a cycle overruns the interval,
//! the missed ticks are skipped rather than replayed in a burst.

use std::time::{Duration, Instant};

use blazebee_mqtt_v3::{ConnectionState, EndpointMetadata, TransferError};
use tokio::{sync::watch, time::MissedTickBehavior};
use tracing::{debug, info, trace, warn};

use super::collectors::{Collector, CollectorError};

/// Upper bound for a single collector call.
pub const COLLECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Every Nth consecutive failure of one collector is logged at `warn`;
/// the ones in between are logged at `debug` to avoid flooding the journal.
const FAILURE_LOG_EVERY: u32 = 120;

/// Destination of encoded samples. Implemented by the MQTT publisher and by
/// test doubles.
pub trait Publish {
    fn publish(
        &self,
        value: &dyn erased_serde::Serialize,
        meta: &EndpointMetadata,
    ) -> Result<usize, TransferError>;
}

impl Publish for blazebee_mqtt_v3::Publisher {
    fn publish(
        &self,
        value: &dyn erased_serde::Serialize,
        meta: &EndpointMetadata,
    ) -> Result<usize, TransferError> {
        blazebee_mqtt_v3::Publisher::publish(self, value, meta)
    }
}

struct Entry {
    name: &'static str,
    collector: Box<dyn Collector>,
    meta: EndpointMetadata,
    /// Consecutive failures (collection or publish) since the last success.
    failures: u32,
}

pub struct Scheduler<P> {
    interval: Duration,
    entries: Vec<Entry>,
    publisher: P,
    link: watch::Receiver<ConnectionState>,
}

impl<P: Publish> Scheduler<P> {
    pub fn new(
        interval: Duration,
        entries: Vec<(&'static str, Box<dyn Collector>, EndpointMetadata)>,
        publisher: P,
        link: watch::Receiver<ConnectionState>,
    ) -> Self {
        Self {
            interval,
            entries: entries
                .into_iter()
                .map(|(name, collector, meta)| Entry {
                    name,
                    collector,
                    meta,
                    failures: 0,
                })
                .collect(),
            publisher,
            link,
        }
    }

    /// Runs until the link reports a fatal state. Dropping the future stops
    /// the loop at the next await point; no state needs cleanup.
    pub async fn run(mut self) {
        let mut ticker = tokio::time::interval(self.interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut paused = false;
        let mut ever_connected = false;
        let mut skipped: u64 = 0;

        loop {
            if paused {
                // Wake early when the connection comes back so the first sample
                // is taken immediately; the cadence restarts from that point.
                tokio::select! {
                    _ = ticker.tick() => {}
                    changed = self.link.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        ticker.reset();
                    }
                }
            } else {
                ticker.tick().await;
            }
            // borrow_and_update marks the value as seen so that the early-wake
            // branch above only fires on a genuinely new state.
            let state = self.link.borrow_and_update().clone();
            if state.is_fatal() {
                return;
            }
            if !state.is_connected() {
                if !paused {
                    if ever_connected {
                        warn!(%state, "broker connection lost; collection paused");
                    } else {
                        info!(%state, "waiting for broker connection");
                    }
                    paused = true;
                }
                skipped += 1;
                continue;
            }
            if paused {
                if ever_connected {
                    info!(
                        skipped_cycles = skipped,
                        "broker connection restored; collection resumed"
                    );
                } else {
                    info!("broker connected; collection started");
                }
                paused = false;
                skipped = 0;
            }
            ever_connected = true;

            let started = Instant::now();
            let mut published = 0usize;
            let mut bytes = 0usize;
            for entry in &mut self.entries {
                if let Some(n) = Self::run_entry(&self.publisher, entry).await {
                    published += 1;
                    bytes += n;
                }
            }
            let elapsed = started.elapsed();
            debug!(
                published,
                failed = self.entries.len() - published,
                bytes,
                elapsed_ms = elapsed.as_millis() as u64,
                "cycle complete"
            );
            if elapsed > self.interval {
                warn!(
                    elapsed_ms = elapsed.as_millis() as u64,
                    interval_ms = self.interval.as_millis() as u64,
                    "cycle took longer than the collection interval; ticks will be skipped"
                );
            }
        }
    }

    /// Collects and publishes one entry. Returns the payload size on success.
    async fn run_entry(publisher: &P, entry: &mut Entry) -> Option<usize> {
        let outcome = match tokio::time::timeout(COLLECT_TIMEOUT, entry.collector.collect()).await {
            Err(_) => Err(Failure::Collect(CollectorError::Timeout(COLLECT_TIMEOUT))),
            Ok(Err(err)) => Err(Failure::Collect(err)),
            Ok(Ok(sample)) => publisher
                .publish(&*sample, &entry.meta)
                .map_err(Failure::Publish),
        };
        match outcome {
            Ok(bytes) => {
                if entry.failures > 0 {
                    info!(
                        collector = entry.name,
                        after_failures = entry.failures,
                        "collector recovered"
                    );
                    entry.failures = 0;
                }
                trace!(collector = entry.name, bytes, "published");
                Some(bytes)
            }
            Err(failure) => {
                entry.failures = entry.failures.saturating_add(1);
                let (kind, err): (&str, &dyn std::fmt::Display) = match &failure {
                    Failure::Collect(e) => ("collection failed", e),
                    Failure::Publish(e) => ("publish failed", e),
                };
                if entry.failures == 1 || entry.failures % FAILURE_LOG_EVERY == 0 {
                    warn!(collector = entry.name, consecutive = entry.failures, error = %err, "{kind}");
                } else {
                    debug!(collector = entry.name, consecutive = entry.failures, error = %err, "{kind}");
                }
                None
            }
        }
    }
}

enum Failure {
    Collect(CollectorError),
    Publish(TransferError),
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        rc::Rc,
        sync::atomic::{AtomicU32, Ordering},
    };

    use async_trait::async_trait;
    use serde::Serialize;

    use super::*;
    use crate::core::collectors::{CollectorResult, Sample};

    #[derive(Default)]
    struct Sink {
        published: RefCell<Vec<(String, String)>>,
        fail_with: RefCell<Option<fn() -> TransferError>>,
    }

    impl Publish for Rc<Sink> {
        fn publish(
            &self,
            value: &dyn erased_serde::Serialize,
            meta: &EndpointMetadata,
        ) -> Result<usize, TransferError> {
            if let Some(f) = *self.fail_with.borrow() {
                return Err(f());
            }
            let json = serde_json::to_string(value).unwrap();
            let len = json.len();
            self.published.borrow_mut().push((meta.topic.clone(), json));
            Ok(len)
        }
    }

    #[derive(Serialize)]
    struct Value {
        n: u32,
    }

    struct Counter(AtomicU32);

    #[async_trait]
    impl Collector for Counter {
        async fn collect(&mut self) -> CollectorResult<Sample> {
            let n = self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Box::new(Value { n }))
        }
    }

    struct Failing;

    #[async_trait]
    impl Collector for Failing {
        async fn collect(&mut self) -> CollectorResult<Sample> {
            Err(CollectorError::Unsupported("always"))
        }
    }

    struct Slow;

    #[async_trait]
    impl Collector for Slow {
        async fn collect(&mut self) -> CollectorResult<Sample> {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Ok(Box::new(Value { n: 0 }))
        }
    }

    fn meta(topic: &str) -> EndpointMetadata {
        EndpointMetadata {
            topic: topic.into(),
            qos: 0,
            retain: false,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn publishes_each_interval_while_connected() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let sink = Rc::new(Sink::default());
                let (tx, rx) = watch::channel(ConnectionState::Connected);
                let scheduler = Scheduler::new(
                    Duration::from_secs(5),
                    vec![
                        (
                            "counter",
                            Box::new(Counter(AtomicU32::new(0))) as Box<dyn Collector>,
                            meta("a"),
                        ),
                        ("failing", Box::new(Failing), meta("b")),
                    ],
                    sink.clone(),
                    rx,
                );
                let task = tokio::task::spawn_local(scheduler.run());
                tokio::time::sleep(Duration::from_secs(11)).await;
                assert_eq!(
                    sink.published.borrow().len(),
                    3,
                    "first tick is immediate, then at 5 s and 10 s"
                );
                assert_eq!(
                    sink.published.borrow()[2],
                    ("a".to_string(), r#"{"n":2}"#.to_string())
                );
                tx.send(ConnectionState::FatalExit("x".into())).unwrap();
                tokio::time::sleep(Duration::from_secs(5)).await;
                assert!(task.is_finished(), "run() returns on fatal state");
            })
            .await;
    }

    #[tokio::test(start_paused = true)]
    async fn pauses_while_disconnected_and_resumes() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let sink = Rc::new(Sink::default());
                let (tx, rx) = watch::channel(ConnectionState::Connecting);
                let scheduler = Scheduler::new(
                    Duration::from_secs(1),
                    vec![(
                        "counter",
                        Box::new(Counter(AtomicU32::new(0))) as Box<dyn Collector>,
                        meta("a"),
                    )],
                    sink.clone(),
                    rx,
                );
                let _task = tokio::task::spawn_local(scheduler.run());
                tokio::time::sleep(Duration::from_millis(3500)).await;
                assert!(
                    sink.published.borrow().is_empty(),
                    "nothing while connecting"
                );
                tx.send(ConnectionState::Connected).unwrap();
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert_eq!(
                    sink.published.borrow().len(),
                    1,
                    "sampled right after connecting"
                );
                tokio::time::sleep(Duration::from_millis(2000)).await;
                assert_eq!(sink.published.borrow().len(), 3, "then once per interval");
                tx.send(ConnectionState::Reconnecting(1.0)).unwrap();
                tokio::time::sleep(Duration::from_millis(3000)).await;
                assert_eq!(sink.published.borrow().len(), 3, "paused again");
            })
            .await;
    }

    #[tokio::test(start_paused = true)]
    async fn slow_collector_is_bounded_by_timeout() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let sink = Rc::new(Sink::default());
                let (_tx, rx) = watch::channel(ConnectionState::Connected);
                let scheduler = Scheduler::new(
                    Duration::from_secs(60),
                    vec![
                        ("slow", Box::new(Slow) as Box<dyn Collector>, meta("slow")),
                        ("counter", Box::new(Counter(AtomicU32::new(0))), meta("a")),
                    ],
                    sink.clone(),
                    rx,
                );
                let _task = tokio::task::spawn_local(scheduler.run());
                tokio::time::sleep(COLLECT_TIMEOUT + Duration::from_millis(100)).await;
                let published = sink.published.borrow();
                assert_eq!(published.len(), 1);
                assert_eq!(published[0].0, "a");
            })
            .await;
    }

    #[tokio::test(start_paused = true)]
    async fn publish_errors_do_not_stop_the_loop() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let sink = Rc::new(Sink::default());
                *sink.fail_with.borrow_mut() = Some(|| TransferError::QueueFull);
                let (_tx, rx) = watch::channel(ConnectionState::Connected);
                let scheduler = Scheduler::new(
                    Duration::from_secs(1),
                    vec![(
                        "counter",
                        Box::new(Counter(AtomicU32::new(0))) as Box<dyn Collector>,
                        meta("a"),
                    )],
                    sink.clone(),
                    rx,
                );
                let _task = tokio::task::spawn_local(scheduler.run());
                tokio::time::sleep(Duration::from_millis(2500)).await;
                assert!(sink.published.borrow().is_empty());
                *sink.fail_with.borrow_mut() = None;
                tokio::time::sleep(Duration::from_millis(1000)).await;
                assert_eq!(sink.published.borrow().len(), 1);
            })
            .await;
    }
}
