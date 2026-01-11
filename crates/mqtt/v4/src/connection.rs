//! MQTT connection management with automatic reconnection and state tracking.
//!
//! This module provides two key types:
//! - `ConnectionBuilder`: Creates client and event loop from configuration
//! - `ConnectionKernel`: Manages the event loop, handles errors, implements backoff
//!
//! The connection kernel is the "heart" of the MQTT manager. It runs the event loop,
//! detects failures, applies exponential backoff, and notifies subscribers of state changes.
//!
//! # Architecture
//!
//! ```text
//! Application
//!     ↓
//! Publisher/Subscriber (use client)
//!     ↓
//! ConnectionKernel (runs event loop, manages reconnection)
//!     ↓
//! rumqttc AsyncClient & EventLoop (MQTT protocol)
//!     ↓
//! Network (TCP/TLS)
//! ```
//!
//! # Usage
//!
//! ```ignore
//! // Build client and event loop
//! let (client, event_loop) = ClientBuilder::from_config(&config)?.build()?;
//!
//! // Create kernel to manage connection
//! let mut kernel = ConnectionKernel::new(client, event_loop, cancel_token);
//!
//! // Subscribe to state changes
//! let mut state_rx = kernel.subscribe_state();
//!
//! // Run the kernel (blocking until shutdown)
//! kernel.reconnect().await?;
//! ```

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use rumqttc::{AsyncClient, ConnectReturnCode, ConnectionError, Event, EventLoop, Packet};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};

use super::{
    backoff::Backoff, client::ClientBuilder, config::Config, error::TransferError,
    state::ConnectionState,
};

/// Builder for creating an MQTT client from configuration.
///
/// This is a lightweight wrapper around `ClientBuilder` that encapsulates
/// the client and event loop. Callers extract these using `take_*` methods.
pub struct ConnectionBuilder {
    /// The MQTT client for sending publishes/subscribes
    client: Option<AsyncClient>,

    /// The event loop for receiving MQTT packets and events
    event_loop: Option<EventLoop>,
}

impl ConnectionBuilder {
    /// Creates a new connection builder from MQTT configuration.
    ///
    /// # Arguments
    /// - `config`: MQTT settings (host, port, TLS, etc.)
    ///
    /// # Returns
    /// - `Ok(Self)`: Builder ready to provide client and event loop
    /// - `Err(TransferError)`: Configuration invalid or client creation failed
    ///
    /// # Examples
    /// ```ignore
    /// let config = Config::load_from_file("mqtt.toml")?;
    /// let builder = ConnectionBuilder::new(&config)?;
    /// ```
    pub fn new(config: &Config) -> Result<Self, TransferError> {
        let (client, event_loop) = ClientBuilder::from_config(&config)?.build()?;

        Ok(Self {
            client: Some(client),
            event_loop: Some(event_loop),
        })
    }

    /// Gets a reference to the MQTT client without consuming it.
    ///
    /// Useful for inspecting client before taking ownership.
    pub fn client(&self) -> Option<&AsyncClient> {
        self.client.as_ref()
    }

    /// Takes ownership of the MQTT client, leaving None in its place.
    ///
    /// Can only be called once per builder. Subsequent calls return None.
    pub async fn take_client(&mut self) -> Option<AsyncClient> {
        self.client.take()
    }

    /// Takes ownership of the event loop, leaving None in its place.
    ///
    /// Can only be called once per builder. Subsequent calls return None.
    pub async fn take_event_loop(&mut self) -> Option<EventLoop> {
        self.event_loop.take()
    }
}

/// Core connection management logic.
///
/// The kernel runs the MQTT event loop, detects connection failures,
/// applies exponential backoff, and notifies subscribers of state changes.
///
/// # Responsibilities
///
/// 1. **Event Loop Management**: Drives rumqttc's event loop to pump MQTT packets
/// 2. **Error Handling**: Detects failures (network, protocol, fatal errors)
/// 3. **Retry Logic**: Applies exponential backoff between reconnection attempts
/// 4. **State Tracking**: Maintains and broadcasts connection state
/// 5. **Graceful Shutdown**: Responds to cancellation token and closes cleanly
///
/// # Concurrency
///
/// The kernel is designed to run on a single tokio task. The client can be
/// cloned and used from other tasks (AsyncClient is thread-safe).
pub struct ConnectionKernel {
    /// The MQTT client for sending commands
    client: AsyncClient,

    /// The event loop that receives MQTT events
    event_loop: EventLoop,

    /// Atomic boolean indicating if we're currently connected
    is_connected: Arc<AtomicBool>,

    /// Exponential backoff for reconnection delays
    backoff: Mutex<Backoff>,

    /// Cancellation token for triggering shutdown
    cancel: CancellationToken,

    /// Broadcast channel for connection state updates
    state_tx: tokio::sync::watch::Sender<ConnectionState>,

    /// Receive side of state broadcast channel
    state_rx: tokio::sync::watch::Receiver<ConnectionState>,
}

impl ConnectionKernel {
    /// Creates a new connection kernel.
    ///
    /// # Arguments
    /// - `client`: MQTT client for sending commands
    /// - `event_loop`: MQTT event loop for receiving packets
    /// - `cancel`: Token to signal shutdown
    ///
    /// # State
    /// - Connected flag: false (not connected yet)
    /// - Backoff: default (1s initial, 1.1x growth, 60s max)
    /// - State: Connecting (initial)
    ///
    /// # Examples
    /// ```ignore
    /// let kernel = ConnectionKernel::new(client, event_loop, cancel_token);
    /// ```
    pub fn new(client: AsyncClient, event_loop: EventLoop, cancel: CancellationToken) -> Self {
        let (state_tx, state_rx) = tokio::sync::watch::channel(ConnectionState::Connecting);
        Self {
            client,
            event_loop,
            is_connected: Arc::new(AtomicBool::new(false)),
            backoff: Mutex::new(Backoff::default()),
            cancel,
            state_tx,
            state_rx,
        }
    }

    /// Subscribes to connection state changes.
    ///
    /// Returns a watch channel receiver. Applications can use this to:
    /// - Update UI in real-time
    /// - Pause/resume publishing based on connection state
    /// - Log or alert on state transitions
    ///
    /// # Returns
    /// A watch receiver that will be notified whenever connection state changes.
    /// The receiver will see the current state immediately upon subscription.
    ///
    /// # Examples
    /// ```ignore
    /// let mut state_rx = kernel.subscribe_state();
    /// loop {
    ///     state_rx.changed().await?;
    ///     match state_rx.borrow().clone() {
    ///         ConnectionState::Connected => println!("Online"),
    ///         ConnectionState::Disconnected(reason) => println!("Error: {}", reason),
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub fn subscribe_state(&self) -> tokio::sync::watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }

    /// Updates the connection state and notifies subscribers.
    ///
    /// Only broadcasts if the state has actually changed (optimization
    /// to avoid unnecessary notifications).
    ///
    /// Logs the state change at INFO level.
    async fn update_state(&mut self, state: ConnectionState) {
        let need_update = {
            let current = self.state_tx.borrow();
            *current != state
        };

        if need_update {
            if let Err(_) = self.state_tx.send(state.clone()) {
                warn!("No subscribers for state updates");
            } else {
                info!("Connection state changed to: {:?}", state);
            }
        }
    }

    /// Main reconnection loop.
    ///
    /// This is the core event loop that:
    /// 1. Runs the MQTT event loop to process packets
    /// 2. Detects errors and failures
    /// 3. Applies exponential backoff on failures
    /// 4. Restarts the connection
    /// 5. Responds to cancellation
    ///
    /// This method runs indefinitely until either:
    /// - A fatal error occurs (connection rejected, wrong creds)
    /// - Max reconnection attempts exceeded
    /// - Cancellation token is triggered
    ///
    /// # Returns
    /// - `Ok(())`: Graceful shutdown or max retries exceeded
    /// - `Err(TransferError)`: Fatal error or unrecoverable state
    ///
    /// # Behavior
    ///
    /// **On connection success** (CONNACK received):
    /// - Set is_connected = true
    /// - Reset backoff timer
    /// - Broadcast Connected state
    ///
    /// **On transient error** (network timeout, connection reset):
    /// - Apply backoff delay
    /// - Retry connection
    /// - Broadcast Reconnecting state with delay
    ///
    /// **On fatal error** (bad credentials, cert validation failed):
    /// - Log as FATAL
    /// - Give up immediately
    /// - Return error to caller
    ///
    /// **On cancellation** (shutdown requested):
    /// - Send DISCONNECT packet
    /// - Release all resources
    /// - Return Ok(())
    pub async fn reconnect(&mut self) -> Result<(), TransferError> {
        self.update_state(ConnectionState::Connecting).await;
        self.backoff.lock().await.reset();

        if self.is_connected.load(Ordering::Acquire) {
            warn!("Connection Kernel already running");
            return Ok(());
        }

        info!("Starting connection event loop...");
        loop {
            tokio::select! {
                // Shutdown requested
                _ = self.cancel.cancelled() => {
                    info!("Shutdown signal received, initiating graceful shutdown...");
                    self.is_connected.store(false, Ordering::Release);

                    self.disconnect().await?;

                    info!("Connection kernel shutdown completed");
                    return Ok(());
                }

                // MQTT event loop tick
                event_result = self.event_loop.poll() => {
                    match event_result {
                        Ok(event) => {
                            // Successfully received MQTT event
                            self.handle_event(event).await?
                        }
                        Err(e) => {
                            // Event loop returned error
                            // Check if this is a fatal error
                            if is_fatal_error(&e) {
                                error!("Fatal error encountered, shutting down driver");
                                debug!("Fatal error encountered, shutting down driver, error: {e}");
                                self.update_state(ConnectionState::Disconnected(e.to_string())).await;
                                return Err(TransferError::from(e));
                            }

                            // Transient error—apply backoff and retry
                            match self.next_retry_delay().await {
                                Ok(sleep_duration) => {
                                    error!(
                                        "Reconnecting in {:.2} seconds due to error: {:?}",
                                        sleep_duration.as_secs_f64(),
                                        get_error_message(&e)
                                    );
                                    self.update_state(ConnectionState::Reconnecting(sleep_duration.as_secs_f64())).await;
                                    tokio::time::sleep(sleep_duration).await;
                                }
                                Err(backoff_err) => {
                                    // Max retries exceeded
                                    error!("Maximum retry attempts exceeded: {:?}", backoff_err);
                                    self.update_state(ConnectionState::Disconnected(backoff_err.to_string())).await;
                                    return Err(backoff_err);
                                }
                            }
                            }
                        }
                    }
            }
        }
    }

    /// Processes a single MQTT event from the broker.
    ///
    /// Handles different packet types:
    /// - ConnAck: Connection established, mark connected
    /// - PingResp/PingReq: Ignore (handled by rumqttc)
    /// - Publish: Trace log (subscription handling in supervisor)
    /// - Disconnect: Mark disconnected
    /// - Others: Ignore
    ///
    /// This is mostly a state machine that updates internal flags
    /// based on protocol events. Actual message handling happens elsewhere.
    async fn handle_event(&mut self, event: Event) -> Result<(), TransferError> {
        match event {
            Event::Incoming(packet) => match packet {
                Packet::ConnAck(conn_ack) => {
                    // Connection handshake successful
                    if conn_ack.code == rumqttc::ConnectReturnCode::Success {
                        info!("Connection established successfully.");
                        self.is_connected.store(true, Ordering::Release);
                        self.update_state(ConnectionState::Connected).await;
                        self.backoff.lock().await.reset();
                    }
                }
                Packet::PingResp | Packet::PingReq => {
                    // Keep-alive ping (handled transparently by rumqttc)
                }
                Packet::Publish(publish) => {
                    // Received published message (from subscription)
                    trace!("Received publish on topic {}", publish.topic);
                }
                Packet::Disconnect => {
                    // Broker closed connection
                    warn!("Disconnected by broker");
                    self.is_connected.store(false, Ordering::Release);
                    self.update_state(ConnectionState::Disconnected(
                        "Disconnected by broker".into(),
                    ))
                    .await;
                }
                _ => {
                    // Other packets (SubAck, PubAck, etc.)—ignore
                }
            },
            Event::Outgoing(outgoing) => {
                // Outgoing packet (we sent this)
                trace!("Outgoing packet: {:?}", outgoing);
            }
        }
        Ok(())
    }

    /// Sends DISCONNECT packet and closes the connection.
    ///
    /// This is a best-effort operation. Even if it fails, we consider
    /// the shutdown successful (the connection will close anyway).
    async fn disconnect(&mut self) -> Result<(), TransferError> {
        if let Err(e) = self.client.disconnect().await {
            warn!("Error sending disconnect packet: {:?}", e);
        }
        Ok(())
    }

    /// Gets the atomic connected flag.
    ///
    /// Can be cloned and used by other tasks to check connection status
    /// without waiting for state channel notification.
    pub fn is_connected(&self) -> Arc<AtomicBool> {
        self.is_connected.to_owned()
    }

    /// Gets the next retry delay and advances the backoff timer.
    ///
    /// Called after each failed reconnection attempt to determine
    /// how long to wait before trying again.
    ///
    /// # Returns
    /// - `Ok(Duration)`: Wait this long before retrying
    /// - `Err(TransferError)`: Max attempts exceeded, give up
    async fn next_retry_delay(&mut self) -> Result<Duration, TransferError> {
        match self.backoff.lock().await.next_sleep() {
            Ok(duration) => Ok(duration),
            Err(e) => Err(TransferError::RetriesPolicy(e)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Disposition {
    // Error is unrecoverable, reconnecting makes no sense
    Fatal,
    // Error is temporary, reconnect attempt is reasonable
    Reconnect,
}

fn classify_connection_error(err: &ConnectionError) -> Disposition {
    use Disposition::*;

    match err {
        // =================================================
        // Fatal errors (no point in retrying)
        // =================================================

        // TLS errors usually mean invalid certificates,
        // broken configuration, or incompatible crypto setup
        ConnectionError::Tls(_) => Fatal,

        // Internal MQTT state corruption or protocol violation
        ConnectionError::MqttState(_) => Fatal,

        // Broker responded with something other than CONNACK
        // This indicates a protocol-level failure
        ConnectionError::NotConnAck(_) => Fatal,

        // All pending requests are completed and connection
        // cannot be reused
        ConnectionError::RequestsDone => Fatal,

        // =================================================
        // Potentially recoverable errors (retryable)
        // =================================================

        // I/O errors need additional classification
        ConnectionError::Io(e) => match e.kind() {
            // These usually indicate local misconfiguration
            // or programming errors, not transient conditions
            std::io::ErrorKind::AddrInUse
            | std::io::ErrorKind::PermissionDenied
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::InvalidData => Fatal,

            // Everything else is assumed to be temporary
            _ => Reconnect,
        },

        // Network stalled or broker did not respond in time
        ConnectionError::NetworkTimeout | ConnectionError::FlushTimeout => Reconnect,

        // Broker explicitly refused the connection
        ConnectionError::ConnectionRefused(code) => match code {
            // These indicate permanent incompatibility or
            // invalid credentials/configuration
            ConnectReturnCode::RefusedProtocolVersion
            | ConnectReturnCode::BadClientId
            | ConnectReturnCode::BadUserNamePassword
            | ConnectReturnCode::NotAuthorized => Fatal,

            // Broker is up but currently overloaded or unavailable
            ConnectReturnCode::ServiceUnavailable => Reconnect,

            // Default to retry for unknown or future codes
            _ => Reconnect,
        },

        // Catch-all for new or unexpected error variants:
        // prefer reconnect to avoid hard failures
        #[allow(unreachable_patterns)]
        _ => Reconnect,
    }
}

fn is_fatal_error(err: &ConnectionError) -> bool {
    // Convenience helper for callers that only care
    // about fatal vs non-fatal classification
    matches!(classify_connection_error(err), Disposition::Fatal)
}

/// Extracts the innermost error message from an error chain.
///
/// Walks down the error source chain to find the root cause message.
/// Removes quotes if present.
fn get_error_message(e: &dyn std::error::Error) -> String {
    let mut current = e;
    while let Some(source) = current.source() {
        current = source;
    }
    let msg = current.to_string();
    msg.trim_matches('"').to_string()
}

#[cfg(test)]
mod connection_manager_tests {
    use super::*;
    use crate::config::FramingConfig;

    /// Helper to create test configuration
    fn create_test_broker_config() -> Config {
        Config {
            base_topic: "test-blazebee_mqtt_v4".to_string(),
            host: "localhost".to_string(),
            port: 1883,
            connection_timeout: 30,
            clean_session: true,
            max_inflight: 20,
            keep_alive: 60,
            client_id: "test_client".to_string(),
            max_packet_size: Some(2048),
            request_channel_capacity: Some(100),
            reconnect_delay: 5,
            max_reconnect_attempts: 5,
            reconnect_backoff_delimiter: 2,
            tls: None,
            serialization: None,
            framing: FramingConfig::default(),
        }
    }

    #[test]
    fn test_connection_builder_new() {
        let config = create_test_broker_config();
        let builder = ConnectionBuilder::new(&config);
        assert!(builder.is_ok());
    }

    #[tokio::test]
    async fn test_connection_builder_client() {
        let config = create_test_broker_config();
        let builder = ConnectionBuilder::new(&config).unwrap();

        assert!(builder.client().is_some());
    }

    #[tokio::test]
    async fn test_connection_builder_take_client() {
        let config = create_test_broker_config();
        let mut builder = ConnectionBuilder::new(&config).unwrap();

        let taken_client = builder.take_client().await;
        assert!(taken_client.is_some());

        // Second call returns None
        assert!(builder.client().is_none());
    }

    #[tokio::test]
    async fn test_connection_builder_take_event_loop() {
        let config = create_test_broker_config();
        let mut builder = ConnectionBuilder::new(&config).unwrap();

        let taken_event_loop = builder.take_event_loop().await;
        assert!(taken_event_loop.is_some());
    }
}

#[cfg(test)]
mod connection_kernel_tests {
    use super::*;
    use crate::config::FramingConfig;

    fn create_test_broker_config() -> Config {
        Config {
            base_topic: "test-blazebee_mqtt_v4".to_string(),
            host: "localhost".to_string(),
            port: 1883,
            connection_timeout: 5,
            clean_session: true,
            max_inflight: 20,
            keep_alive: 30,
            client_id: "test_client_kernel".to_string(),
            max_packet_size: Some(2048),
            request_channel_capacity: Some(100),
            reconnect_delay: 1,
            max_reconnect_attempts: 3,
            reconnect_backoff_delimiter: 2,
            tls: None,
            serialization: None,
            framing: FramingConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_is_connected_accessor() {
        let config = create_test_broker_config();
        let mut builder = ConnectionBuilder::new(&config).unwrap();
        let client = builder.take_client().await.unwrap();
        let event_loop = builder.take_event_loop().await.unwrap();
        let kernel = ConnectionKernel::new(client, event_loop, CancellationToken::new());

        let is_connected = kernel.is_connected();
        assert!(!is_connected.load(Ordering::Acquire));

        is_connected.store(true, Ordering::Release);
        assert!(is_connected.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn test_backoff_functionality() {
        let config = create_test_broker_config();
        let mut builder = ConnectionBuilder::new(&config).unwrap();
        let client = builder.take_client().await.unwrap();
        let event_loop = builder.take_event_loop().await.unwrap();
        let mut kernel = ConnectionKernel::new(client, event_loop, CancellationToken::new());

        let initial_delay = kernel.next_retry_delay().await.unwrap();
        assert!(initial_delay >= Duration::from_secs(1));

        let next_delay = kernel.next_retry_delay().await.unwrap();
        assert!(next_delay >= initial_delay);
    }

    #[tokio::test]
    async fn test_is_fatal_error_detection() {
        use std::io;

        let non_fatal_error = rumqttc::ConnectionError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "connection refused",
        ));
        assert!(!is_fatal_error(&non_fatal_error));

        let fatal_error1 = rumqttc::ConnectionError::Io(io::Error::new(
            io::ErrorKind::AddrInUse,
            "address in use",
        ));
        assert!(is_fatal_error(&fatal_error1));

        let fatal_error2 = rumqttc::ConnectionError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "permission denied",
        ));
        assert!(is_fatal_error(&fatal_error2));
    }

    #[tokio::test]
    async fn test_connection_kernel_state_subscription() {
        let config = create_test_broker_config();
        let mut builder = ConnectionBuilder::new(&config).unwrap();
        let client = builder.take_client().await.unwrap();
        let event_loop = builder.take_event_loop().await.unwrap();
        let mut kernel = ConnectionKernel::new(client, event_loop, CancellationToken::new());

        let mut state_rx = kernel.subscribe_state();
        let initial_state = state_rx.borrow().clone();
        assert_eq!(initial_state, ConnectionState::Connecting);

        kernel.update_state(ConnectionState::Connected).await;

        state_rx.changed().await.unwrap();
        let new_state = state_rx.borrow().clone();
        assert_eq!(new_state, ConnectionState::Connected);
    }
}
