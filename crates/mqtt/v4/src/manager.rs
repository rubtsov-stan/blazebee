//! MQTT Manager Module
//! High-level MQTT manager that coordinates all components.
//!
//! The `MqttManager` is the entry point for applications. It:
//! 1. Loads or creates configuration
//! 2. Builds the MQTT client and connection kernel
//! 3. Spawns the supervisor for subscription management
//! 4. Returns an `MqttInstance` for application use
//!
//! # Base Topic Support
//!
//! The manager automatically applies the `base_topic` from configuration to all
//! MQTT operations:
//! - Subscriptions: `topic` → `{base_topic}/topic`
//! - Publications: `topic` → `{base_topic}/topic`
//! - Unsubscriptions: `topic` → `{base_topic}/topic`
//!
//! To disable base_topic, set `base_topic = ""` in configuration.
//!
//! # Typical Usage
//!
//! ```ignore
//! let manager = MqttManager::new("mqtt.example.com", 1883)?;
//! let instance = manager.build().await?;
//!
//! // Now use instance.publish(), instance.subscribe(), etc.
//! instance.subscribe("sensor/+/data").await?; // Subscribes to "myapp/sensor/+/data"
//! let msg = MyMessage { temperature: 23.5 };
//! instance.publish(&msg, &metadata).await?; // Publishes to "myapp/sensor/temperature"
//! ```

use std::{collections::HashSet, sync::Arc};

use rumqttc::{AsyncClient, QoS, SubscribeFilter};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::{
    client::ClientBuilder, config::Config, connection::ConnectionKernel, error::TransferError,
    supervisor::Supervisor,
};

#[derive(Debug)]
pub struct PublishDrain {
    inflight: std::sync::atomic::AtomicUsize,
    notify: tokio::sync::Notify,
}

impl PublishDrain {
    pub fn new() -> Self {
        Self {
            inflight: std::sync::atomic::AtomicUsize::new(0),
            notify: tokio::sync::Notify::new(),
        }
    }

    pub fn enter(self: &std::sync::Arc<Self>) -> PublishGuard {
        self.inflight
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        PublishGuard {
            drain: self.clone(),
        }
    }

    pub async fn wait_idle(&self) {
        while self.inflight.load(std::sync::atomic::Ordering::Acquire) != 0 {
            self.notify.notified().await;
        }
    }
}

pub struct PublishGuard {
    drain: std::sync::Arc<PublishDrain>,
}

impl Drop for PublishGuard {
    fn drop(&mut self) {
        if self
            .drain
            .inflight
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel)
            == 1
        {
            self.drain.notify.notify_waiters();
        }
    }
}

/// Entry point for building an MQTT manager.
///
/// The manager handles initialization of all components:
/// - Client builder
/// - Connection kernel (with reconnect logic)
/// - Supervisor (subscription management)
/// - Subscription manager (with base_topic support)
/// - State management
///
/// Most applications create a manager, call `build()` to get an
/// instance, then use the instance for their MQTT operations.
pub struct MqttManager {
    /// Configuration for the MQTT connection
    config: Config,

    /// The MQTT client (optional, populated during build)
    client: Option<AsyncClient>,

    /// The Subscription manager for topic handling
    subscription_manager: Option<SubscriptionManager>,

    /// The supervisor managing subscriptions and reconnection
    supervisor: Option<Supervisor>,

    /// Cancellation token for coordinating shutdown
    cancel_token: CancellationToken,
}

impl MqttManager {
    /// Creates a manager from an existing configuration struct.
    ///
    /// # Arguments
    /// - `config`: Pre-built or loaded configuration
    ///
    /// # Returns
    /// - `Ok(Self)`: Manager ready to build
    /// - `Err(TransferError)`: Currently never fails, kept for future validation
    ///
    /// # Examples
    /// ```ignore
    /// let config = Config::from_file("mqtt.toml")?;
    /// let manager = MqttManager::from_config(config)?;
    /// ```
    pub fn from_config(config: Config) -> Result<Self, TransferError> {
        Ok(Self {
            config,
            client: None,
            supervisor: None,
            subscription_manager: None,
            cancel_token: CancellationToken::new(),
        })
    }

    /// Creates a manager with minimal configuration.
    ///
    /// Useful for quick setup or testing. All other settings use defaults.
    ///
    /// # Arguments
    /// - `host`: Broker hostname or IP
    /// - `port`: Broker port (usually 1883 or 8883)
    ///
    /// # Returns
    /// - `Ok(Self)`: Manager with default settings
    /// - `Err(TransferError)`: If settings validation fails
    ///
    /// # Examples
    /// ```ignore
    /// let manager = MqttManager::new("localhost", 1883)?;
    /// ```
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, TransferError> {
        let config = Config {
            host: host.into(),
            port,
            ..Default::default()
        };
        Self::from_config(config)
    }

    /// Builds all MQTT infrastructure.
    ///
    /// This is the main initialization method. It:
    /// 1. Creates the MQTT client
    /// 2. Creates the connection kernel
    /// 3. Creates the supervisor
    /// 4. Creates the subscription manager (with base_topic support)
    /// 5. Prepares the instance for use
    ///
    /// Note: The actual connection happens asynchronously after this returns.
    /// Check the state via subscription to know when connected.
    ///
    /// # Returns
    /// - `Ok(MqttInstance)`: Ready to use instance
    /// - `Err(TransferError)`: If any component failed to initialize
    ///
    /// # Examples
    /// ```ignore
    /// let instance = manager.build_and_start().await?;
    /// instance.subscribe("topic").await?; // Uses base_topic
    /// ```
    pub async fn build_and_start(mut self) -> Result<MqttInstance, TransferError> {
        info!(
            "Building MQTT infrastructure with base_topic: '{}'",
            self.config.base_topic
        );

        // Create client and event loop
        let (client, event_loop) = ClientBuilder::from_config(&self.config)?.build()?;

        self.client = Some(client.clone());
        // Token that actually stops ConnectionKernel.
        let connection_cancel = CancellationToken::new();
        // Create connection kernel (manages event loop and reconnection)
        let mut connection_kernel =
            ConnectionKernel::new(client.clone(), event_loop, connection_cancel.clone());
        // Publish drain barrier: connection cancel is delayed until all publishes complete.
        let publish_drain = Arc::new(PublishDrain::new());
        let state_rx = connection_kernel.subscribe_state();

        // Create supervisor (manages subscriptions and reconnection)
        let supervisor = Supervisor::new(
            &self.config.base_topic,
            state_rx,
            client.clone(),
            self.cancel_token.clone(),
        );

        // Create subscription manager with base_topic
        let subscription_manager = SubscriptionManager::new(
            &self.config.base_topic,
            client.clone(),
            self.cancel_token.clone(),
        );

        self.supervisor = Some(supervisor.clone());
        self.subscription_manager = Some(subscription_manager.clone());

        info!("MQTT infrastructure built successfully");

        // Spawn connection kernel task
        tokio::spawn(async move {
            if let Err(e) = connection_kernel.reconnect().await {
                error!("MQTT connection kernel exited with error: {:?}", e);
                panic!("MQTT connection kernel failed to start")
            };
        });

        // Bridge: request shutdown -> wait drain -> stop connection kernel.
        {
            let shutdown_request = self.cancel_token.clone();
            let drain = publish_drain.clone();
            let conn_cancel = connection_cancel.clone();
            tokio::spawn(async move {
                shutdown_request.cancelled().await;
                drain.wait_idle().await;
                conn_cancel.cancel();
            });
        }
        Ok(MqttInstance {
            client,
            supervisor,
            subscription_manager,
            cancel_token: self.cancel_token,
            base_topic: self.config.base_topic.clone(),
            max_packet_size: self.config.max_packet_size.unwrap_or_default() as usize,
            framing_enabled: self.config.framing.enabled,
            publish_drain: publish_drain,
            connection_cancel: connection_cancel,
        })
    }

    /// Creates only the MQTT client without starting the event loop.
    ///
    /// Useful for testing or special scenarios where you want to manage
    /// the event loop manually. Normally, use `build_and_start()`.
    ///
    /// # Returns
    /// - `Ok(AsyncClient)`: The MQTT client, ready to use
    /// - `Err(TransferError)`: If client creation failed
    ///
    /// # Examples
    /// ```ignore
    /// let client = manager.build_client_only().await?;
    /// // Manually spawn event loop on another task
    /// ```
    pub async fn build_client_only(self) -> Result<AsyncClient, TransferError> {
        let (client, _) = ClientBuilder::from_config(&self.config)?.build()?;
        Ok(client)
    }

    /// Gets a reference to the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Gets a clone of the cancellation token.
    ///
    /// Can be used to trigger shutdown from anywhere in the application.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}

/// Manages MQTT topic subscriptions with automatic reconnection and base_topic support.
///
/// The subscription manager:
/// - Keeps track of all subscribed topics (without base_topic prefix)
/// - Automatically applies base_topic when communicating with broker
/// - Handles subscribing and unsubscribing operations
/// - Automatically resubscribes to all topics when reconnected
/// - Provides information about current subscriptions
///
/// Most operations are best-effort, meaning failures are logged but don't
/// break the system, making it resilient during network disruptions.
#[derive(Debug, Clone)]
pub struct SubscriptionManager {
    /// Base topic prefix for all operations
    base_topic: String,

    /// Set of currently subscribed topics (without base_topic prefix)
    topics: Arc<RwLock<HashSet<String>>>,

    /// The MQTT client for sending subscription commands
    client: Arc<AsyncClient>,

    /// Cancellation token for shutdown coordination
    cancel_token: CancellationToken,
}

impl SubscriptionManager {
    /// Creates a new subscription manager with base_topic support.
    ///
    /// # Arguments
    /// - `base_topic`: Base topic prefix for all operations
    /// - `client`: MQTT client for sending subscription commands
    /// - `cancel_token`: Cancellation token for shutdown coordination
    ///
    /// # Examples
    /// ```ignore
    /// let manager = SubscriptionManager::new(
    ///     "myapp",
    ///     client,
    ///     cancel_token,
    /// );
    /// ```
    pub fn new(base_topic: &str, client: AsyncClient, cancel_token: CancellationToken) -> Self {
        Self {
            base_topic: base_topic.to_string(),
            topics: Arc::new(RwLock::new(HashSet::new())),
            client: Arc::new(client),
            cancel_token,
        }
    }

    /// Applies base_topic to a given topic.
    ///
    /// # Arguments
    /// - `topic`: Topic without base_topic prefix
    ///
    /// # Returns
    /// - `String`: Full topic with base_topic prefix
    ///
    /// # Examples
    /// ```ignore
    /// let full = manager.with_base_topic("sensor/temperature");
    /// // Returns "myapp/sensor/temperature"
    /// ```
    pub fn with_base_topic(&self, topic: &str) -> String {
        if self.base_topic.is_empty() {
            topic.to_string()
        } else {
            format!(
                "{}/{}",
                self.base_topic.trim_end_matches('/'),
                topic.trim_start_matches('/')
            )
        }
    }

    /// Removes base_topic from a given full topic.
    ///
    /// # Arguments
    /// - `topic`: Full topic with base_topic prefix
    ///
    /// # Returns
    /// - `String`: Topic without base_topic prefix
    ///
    /// # Examples
    /// ```ignore
    /// let short = manager.strip_base_topic("myapp/sensor/temperature");
    /// // Returns "sensor/temperature"
    /// ```
    pub fn strip_base_topic(&self, topic: &str) -> String {
        if self.base_topic.is_empty() {
            topic.to_string()
        } else {
            let base = format!("{}/", self.base_topic.trim_end_matches('/'));
            topic.strip_prefix(&base).unwrap_or(topic).to_string()
        }
    }

    /// Subscribes to a topic.
    ///
    /// Adds the topic (without base_topic) to the manager's internal list and
    /// subscribes to the full topic (with base_topic) with the broker.
    /// If not connected, the subscription will be automatically retried when
    /// the connection is restored.
    ///
    /// # Arguments
    /// - `topic`: Topic string without base_topic (can include MQTT wildcards)
    ///
    /// # Returns
    /// - `Ok(())`: Topic successfully registered
    /// - `Err(TransferError)`: Only if registration fails (rare)
    ///
    /// # Wildcards
    /// - `+`: Matches a single level (e.g., "home/+/temperature")
    /// - `#`: Matches multiple levels (e.g., "home/#")
    ///
    /// # Examples
    /// ```ignore
    /// // With base_topic = "myapp"
    /// subscription_manager.subscribe("sensor/+/data").await?;
    /// // Actually subscribes to "myapp/sensor/+/data"
    /// ```
    pub async fn subscribe(&self, topic: &str) -> Result<(), TransferError> {
        // Add to our tracked topics (without base_topic)
        {
            let mut topics = self.topics.write().await;
            topics.insert(topic.to_string());
        }

        // Subscribe with base_topic
        let full_topic = self.with_base_topic(topic);
        match self.try_subscribe_now(&full_topic).await {
            Ok(()) => {
                info!("Subscribed to topic: {} (full: {})", topic, full_topic);
                Ok(())
            }
            Err(e) => {
                warn!(
                    "Immediate subscription to '{}' (full: '{}') failed (will retry on reconnect): {:?}",
                    topic, full_topic, e
                );
                // Still return Ok since we've registered it for later
                Ok(())
            }
        }
    }

    /// Unsubscribes from a topic.
    ///
    /// Removes the topic (without base_topic) from the manager's list and sends
    /// an unsubscribe request to the broker for the full topic (with base_topic).
    /// Even if the request fails, the topic is removed from our tracking to avoid
    /// resubscribing later.
    ///
    /// # Arguments
    /// - `topic`: Topic string without base_topic to unsubscribe from
    ///
    /// # Returns
    /// - `Ok(())`: Topic successfully unsubscribed
    /// - `Err(TransferError)`: Only if local removal fails (rare)
    ///
    /// # Examples
    /// ```ignore
    /// // With base_topic = "myapp"
    /// subscription_manager.unsubscribe("old/topic").await?;
    /// // Actually unsubscribes from "myapp/old/topic"
    /// ```
    pub async fn unsubscribe(&self, topic: &str) -> Result<(), TransferError> {
        // Unsubscribe with base_topic
        let full_topic = self.with_base_topic(topic);
        match self.try_unsubscribe_now(&full_topic).await {
            Ok(()) => info!("Unsubscribed from topic: {} (full: {})", topic, full_topic),
            Err(e) => warn!(
                "Failed to unsubscribe from '{}' (full: '{}'): {:?}",
                topic, full_topic, e
            ),
        }

        // Always remove from our tracking
        let mut topics = self.topics.write().await;
        topics.remove(topic);

        Ok(())
    }

    /// Gets a list of all currently tracked topics (without base_topic).
    ///
    /// This returns the topics we're trying to subscribe to, which may
    /// not be actively subscribed if we're currently disconnected.
    ///
    /// # Returns
    /// - `Vec<String>`: List of topic strings without base_topic prefix
    ///
    /// # Examples
    /// ```ignore
    /// let topics = subscription_manager.get_subscribed_topics().await;
    /// println!("Tracking {} topics", topics.len());
    /// ```
    pub async fn get_subscribed_topics(&self) -> Vec<String> {
        let topics = self.topics.read().await;
        topics.iter().cloned().collect()
    }

    /// Gets a list of all full topics (with base_topic).
    ///
    /// Returns the actual topics as they appear on the broker.
    ///
    /// # Returns
    /// - `Vec<String>`: List of full topic strings
    ///
    /// # Examples
    /// ```ignore
    /// let full_topics = subscription_manager.get_full_topics().await;
    /// ```
    pub async fn get_full_topics(&self) -> Vec<String> {
        let topics = self.topics.read().await;
        topics.iter().map(|t| self.with_base_topic(t)).collect()
    }

    /// Checks if we're tracking a specific topic (without base_topic).
    ///
    /// # Arguments
    /// - `topic`: Topic string without base_topic to check
    ///
    /// # Returns
    /// - `true`: Topic is in our tracking list
    /// - `false`: Topic is not being tracked
    ///
    /// # Examples
    /// ```ignore
    /// if subscription_manager.is_subscribed("my/topic").await {
    ///     println!("Already subscribed to my/topic");
    /// }
    /// ```
    pub async fn is_subscribed(&self, topic: &str) -> bool {
        let topics = self.topics.read().await;
        topics.contains(topic)
    }

    /// Resubscribes to all tracked topics.
    ///
    /// Called automatically by the supervisor when connection is restored.
    /// Uses `subscribe_many()` for efficiency when there are many topics.
    /// Automatically applies base_topic to all topics.
    ///
    /// # Returns
    /// - `Ok(())`: All topics resubscribed successfully
    /// - `Err(TransferError)`: If the bulk subscription fails
    ///
    /// # Note
    /// This is typically called automatically; you don't need to call it manually.
    pub async fn resubscribe_all(&self) -> Result<(), TransferError> {
        let topics = self.topics.read().await;

        if topics.is_empty() {
            debug!("No topics to resubscribe to");
            return Ok(());
        }

        // Create filters with base_topic
        let filters: Vec<SubscribeFilter> = topics
            .iter()
            .map(|topic| {
                let full_topic = self.with_base_topic(topic);
                SubscribeFilter::new(full_topic, QoS::AtLeastOnce)
            })
            .collect();

        info!(
            "Resubscribing to {} topics with base_topic '{}'",
            filters.len(),
            self.base_topic
        );

        if let Err(e) = self.client.subscribe_many(filters).await {
            error!("Failed to resubscribe to topics: {:?}", e);
            return Err(e.into());
        }

        info!("Successfully resubscribed to all topics");
        Ok(())
    }

    /// Clears all tracked subscriptions.
    ///
    /// Removes all topics from the tracking list. Does not send unsubscribe
    /// commands to the broker. Useful for cleanup or reset scenarios.
    ///
    /// # Returns
    /// - `Ok(())`: All topics cleared
    pub async fn clear_all(&self) -> Result<(), TransferError> {
        let mut topics = self.topics.write().await;
        topics.clear();
        info!("Cleared all tracked subscriptions");
        Ok(())
    }

    /// Gets the base topic prefix.
    ///
    /// # Returns
    /// - `&str`: The base topic prefix
    pub fn base_topic(&self) -> &str {
        &self.base_topic
    }

    /// Gets the cancellation token.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Internal: Attempts to subscribe to a single topic immediately.
    async fn try_subscribe_now(&self, full_topic: &str) -> Result<(), TransferError> {
        self.client.subscribe(full_topic, QoS::AtLeastOnce).await?;
        Ok(())
    }

    /// Internal: Attempts to unsubscribe from a single topic immediately.
    async fn try_unsubscribe_now(&self, full_topic: &str) -> Result<(), TransferError> {
        self.client.unsubscribe(full_topic).await?;
        Ok(())
    }
}

/// An active MQTT connection instance ready for use.
///
/// Returned by `MqttManager::build_and_start()`. Provides all the interfaces
/// applications need: publishing, subscribing, state monitoring, and shutdown.
/// All topic operations automatically use the configured base_topic.
#[derive(Debug, Clone)]
pub struct MqttInstance {
    /// The MQTT client for sending commands
    client: AsyncClient,

    /// The supervisor managing subscriptions
    supervisor: Supervisor,

    /// The subscription manager for topic handling
    subscription_manager: SubscriptionManager,

    /// Cancellation token for triggering shutdown
    cancel_token: CancellationToken,

    /// Base topic prefix for all operations
    base_topic: String,

    /// Max MQTT packet size (bytes) - used as max frame payload size.
    max_packet_size: usize,

    /// Enables framing behavior in Publisher::publish_framed.
    framing_enabled: bool,

    /// ConnectionKernel cancellation token (fires only after publish drain).
    connection_cancel: CancellationToken,

    /// Barrier that tracks in-flight publishes and delays ConnectionKernel shutdown.
    publish_drain: Arc<PublishDrain>,
}

impl MqttInstance {
    /// Gets a reference to the MQTT client.
    ///
    /// The client is thread-safe and can be cloned for use across tasks.
    pub fn client(&self) -> &AsyncClient {
        &self.client
    }

    /// Gets a reference to the supervisor.
    ///
    /// The supervisor can be cloned for advanced subscription management.
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    /// Gets a reference to the subscription manager.
    ///
    /// The subscription manager can be cloned for managing topic subscriptions.
    pub fn subscription_manager(&self) -> &SubscriptionManager {
        &self.subscription_manager
    }

    /// Gets the base topic prefix.
    ///
    /// # Returns
    /// - `&str`: The base topic prefix
    ///
    /// # Examples
    /// ```ignore
    /// println!("Base topic: {}", instance.base_topic());
    /// ```
    pub fn base_topic(&self) -> &str {
        &self.base_topic
    }

    /// Applies base_topic to a given topic.
    ///
    /// # Arguments
    /// - `topic`: Topic without base_topic prefix
    ///
    /// # Returns
    /// - `String`: Full topic with base_topic prefix
    ///
    /// # Examples
    /// ```ignore
    /// let full = instance.with_base_topic("sensor/temperature");
    /// ```
    pub fn with_base_topic(&self, topic: &str) -> String {
        self.subscription_manager.with_base_topic(topic)
    }

    /// Removes base_topic from a given full topic.
    ///
    /// # Arguments
    /// - `topic`: Full topic with base_topic prefix
    ///
    /// # Returns
    /// - `String`: Topic without base_topic prefix
    ///
    /// # Examples
    /// ```ignore
    /// let short = instance.strip_base_topic("myapp/sensor/temperature");
    /// ```
    pub fn strip_base_topic(&self, topic: &str) -> String {
        self.subscription_manager.strip_base_topic(topic)
    }

    /// Starts monitoring connection state and resubscribing on reconnect.
    ///
    /// This spawns a background task that:
    /// 1. Watches for connection state changes
    /// 2. Resubscribes to all topics when reconnected
    /// 3. Publishes a "online" status message (with base_topic)
    ///
    /// Call this after creating subscriptions. It must be awaited (spawns
    /// a task internally) but returns immediately.
    ///
    /// # Returns
    /// - `Ok(())`: Monitoring task started
    /// - `Err(TransferError)`: If monitoring setup failed
    ///
    /// # Examples
    /// ```ignore
    /// instance.subscribe("device/+/status").await?;
    /// instance.start_monitoring().await?;
    /// ```
    pub async fn start_monitoring(&self) -> Result<(), TransferError> {
        self.supervisor.monitor().await
    }

    /// Subscribes to a topic.
    ///
    /// The subscription is registered with the supervisor, which will
    /// automatically resubscribe if the connection is lost and restored.
    /// The base_topic is automatically applied.
    ///
    /// # Arguments
    /// - `topic`: Topic string without base_topic (may include wildcards)
    ///
    /// # Returns
    /// - `Ok(())`: Subscription registered (may not be active if disconnected)
    /// - `Err(TransferError)`: If subscription failed
    ///
    /// # Wildcards
    /// - `+`: Single level (e.g., "home/+/temperature")
    /// - `#`: Multi-level (e.g., "home/#")
    ///
    /// # Examples
    /// ```ignore
    /// // With base_topic = "myapp"
    /// instance.subscribe("device/123/commands").await?;
    /// // Actually subscribes to "myapp/device/123/commands"
    /// instance.subscribe("sensor/+/data").await?;
    /// // Actually subscribes to "myapp/sensor/+/data"
    /// ```
    pub async fn subscribe(&self, topic: &str) -> Result<(), TransferError> {
        self.subscription_manager.subscribe(topic).await
    }

    /// Unsubscribes from a topic.
    ///
    /// # Arguments
    /// - `topic`: Topic string without base_topic to unsubscribe from
    ///
    /// # Returns
    /// - `Ok(())`: Unsubscription processed
    /// - `Err(TransferError)`: If unsubscription failed
    ///
    /// # Examples
    /// ```ignore
    /// // With base_topic = "myapp"
    /// instance.unsubscribe("old/topic").await?;
    /// // Actually unsubscribes from "myapp/old/topic"
    /// ```
    pub async fn unsubscribe(&self, topic: &str) -> Result<(), TransferError> {
        self.subscription_manager.unsubscribe(topic).await
    }

    /// Gracefully shuts down the MQTT connection.
    ///
    /// Sends a DISCONNECT packet, stops the event loop, and releases resources.
    /// Applications should call this before exiting.
    ///
    /// # Returns
    /// - `Ok(())`: Shutdown completed
    /// - `Err(TransferError)`: If shutdown failed
    ///
    /// # Examples
    /// ```ignore
    /// instance.shutdown().await?;
    /// println!("MQTT disconnected");
    /// ```
    pub async fn shutdown(&self) -> Result<(), TransferError> {
        self.client.disconnect().await?;
        // request shutdown for the rest of the system
        self.cancel_token.cancel();
        // wait for all in-flight publishes (including framed streams)
        self.publish_drain.wait_idle().await;
        // stop connection kernel; it will best-effort DISCONNECT
        self.connection_cancel.cancel();
        Ok(())
    }

    /// Gets the cancellation token.
    ///
    /// Can be cloned and used to trigger shutdown from other parts of the app.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
    /// Gets the max packet size param
    pub fn max_packet_size(&self) -> usize {
        self.max_packet_size
    }

    /// Gets the frame availability status
    pub fn framing_enabled(&self) -> bool {
        self.framing_enabled
    }

    /// Gets the publish drain idle
    pub fn publish_drain(&self) -> Arc<PublishDrain> {
        self.publish_drain.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = MqttManager::new("localhost", 1883).unwrap();
        assert_eq!(manager.config().host, "localhost");
        assert_eq!(manager.config().port, 1883);
        assert_eq!(manager.config().base_topic, "blazebee_mqtt_v4"); // Default base_topic
    }

    #[tokio::test]
    async fn test_manager_from_config() {
        let config = Config {
            base_topic: "myapp".to_string(),
            host: "localhost".to_string(),
            port: 1883,
            ..Default::default()
        };
        let manager = MqttManager::from_config(config).unwrap();
        assert_eq!(manager.config().base_topic, "myapp");
        assert!(!manager.cancel_token().is_cancelled());
    }

    #[tokio::test]
    async fn test_subscription_manager_base_topic() {
        let cancel_token = CancellationToken::new();
        let (client, _) =
            rumqttc::AsyncClient::new(rumqttc::MqttOptions::new("test", "localhost", 1883), 10);

        let manager = SubscriptionManager::new("myapp", client.clone(), cancel_token.clone());

        // Test with_base_topic
        assert_eq!(manager.with_base_topic("sensor/temp"), "myapp/sensor/temp");
        assert_eq!(manager.with_base_topic("/sensor/temp"), "myapp/sensor/temp");

        // Test strip_base_topic
        assert_eq!(manager.strip_base_topic("myapp/sensor/temp"), "sensor/temp");
        assert_eq!(
            manager.strip_base_topic("other/sensor/temp"),
            "other/sensor/temp"
        );

        // Test with empty base_topic
        let manager2 = SubscriptionManager::new("", client.clone(), cancel_token.clone());
        assert_eq!(manager2.with_base_topic("sensor/temp"), "sensor/temp");
        assert_eq!(manager2.strip_base_topic("sensor/temp"), "sensor/temp");
    }
}
