//! Connection state monitoring and lifecycle management.
//!
//! The `Supervisor` watches over the MQTT connection and handles important
//! lifecycle events:
//! - Publishing online/offline status messages
//! - Reacting to connection and disconnection events
//!
//! Most applications won't interact directly with the supervisor—it works
//! automatically in the background once started via `MqttInstance`.

use rumqttc::{AsyncClient, QoS};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::{error::TransferError, state::ConnectionState};

/// Monitors MQTT connection state and handles lifecycle events.
///
/// The supervisor:
/// - Publishes online status when connected
/// - Handles graceful disconnection
/// - Reacts to cancellation for clean shutdown
///
/// It runs in the background once started and requires minimal attention.
#[derive(Debug, Clone)]
pub struct Supervisor {
    /// Watch channel for connection state (from connection kernel)
    state_rx: watch::Receiver<ConnectionState>,

    /// Base topic for application-level status messages
    base_topic: String,

    /// The MQTT client for sending commands
    client: AsyncClient,

    /// Cancellation token for shutdown
    cancel_token: CancellationToken,
}

impl Supervisor {
    /// Creates a new supervisor.
    ///
    /// # Arguments
    /// - `base_topic`: Base topic for status messages (e.g., "myapp")
    ///   Status gets published to "{base_topic}/status"
    /// - `state_rx`: Watch channel receiving connection state updates
    /// - `client`: MQTT client for sending commands
    /// - `cancel_token`: Cancellation token for shutdown
    pub fn new(
        base_topic: &str,
        state_rx: watch::Receiver<ConnectionState>,
        client: AsyncClient,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            base_topic: base_topic.to_string(),
            state_rx,
            client,
            cancel_token,
        }
    }

    /// Publishes the "online" status message.
    ///
    /// Called when we first connect to let other clients know we're online.
    async fn publish_online_status(&self) -> Result<(), TransferError> {
        self.client
            .publish(
                format!("{}/status", self.base_topic),
                QoS::ExactlyOnce,
                true,
                "online".as_bytes().to_vec(),
            )
            .await?;
        info!("Published online status to {}/status", self.base_topic);
        Ok(())
    }

    /// Handles a successful connection to the broker.
    ///
    /// Publishes our online status
    async fn on_connect(&self) -> Result<(), TransferError> {
        // Publish that we're online (best effort)
        if let Err(e) = self.publish_online_status().await {
            warn!("Failed to publish online status: {}", e);
        }

        Ok(())
    }

    /// Handles disconnection from the broker.
    ///
    /// Tries to disconnect cleanly, mostly for logging purposes.
    async fn on_disconnect(&self) -> Result<(), TransferError> {
        info!("Disconnected from MQTT broker");

        // Try to disconnect cleanly
        if let Err(e) = self.client.disconnect().await {
            warn!("Clean disconnect failed: {}", e);
        }

        Ok(())
    }

    /// Starts monitoring connection state and handling events.
    ///
    /// Launches a background task that:
    /// 1. Watches for connection state changes
    /// 2. Calls appropriate handlers for connect/disconnect events
    /// 3. Responds to cancellation requests
    ///
    /// You should call this early in your application, ideally right after
    /// creating your MqttInstance. It returns immediately after starting
    /// the background task.
    ///
    /// # Returns
    /// - `Ok(())`: Monitoring task started
    /// - `Err(TransferError)`: Should not fail in normal circumstances
    ///
    /// # Examples
    /// ```ignore
    /// supervisor.monitor().await?;
    /// // Now the supervisor is running in the background
    /// ```
    pub async fn monitor(&self) -> Result<(), TransferError> {
        let state_rx = self.state_rx.clone();
        let cancel = self.cancel_token.clone();

        // Handle initial state if we're already connected
        let current_state = state_rx.borrow().clone();
        if let ConnectionState::Connected = current_state {
            info!("Already connected when supervisor started");
            if let Err(e) = self.on_connect().await {
                warn!("Initial connection handling failed: {:?}", e);
            }
        }

        // Spawn the monitoring task
        tokio::spawn({
            let supervisor = self.clone();
            async move {
                supervisor.run_monitor_loop(state_rx, cancel).await;
            }
        });

        Ok(())
    }

    /// Internal: Runs the main monitoring loop.
    async fn run_monitor_loop(
        &self,
        mut state_rx: watch::Receiver<ConnectionState>,
        cancel: CancellationToken,
    ) {
        info!("Supervisor monitoring started");

        loop {
            tokio::select! {
                // Check for shutdown request
                _ = cancel.cancelled() => {
                    info!("Supervisor shutting down due to cancellation");
                    if let Err(e) = self.on_disconnect().await {
                        warn!("Error during shutdown disconnect: {:?}", e);
                    }
                    break;
                }

                // Watch for connection state changes
                changed = state_rx.changed() => {
                    if changed.is_err() {
                        warn!("State channel closed, supervisor exiting");
                        break;
                    }

                    let state = state_rx.borrow().clone();
                    match state {
                        ConnectionState::Connected => {
                            if let Err(e) = self.on_connect().await {
                                warn!("Error handling connection: {:?}", e);
                            }
                        }
                        ConnectionState::Disconnected(reason) => {
                            warn!("Disconnected: {}", reason);
                            if let Err(e) = self.on_disconnect().await {
                                warn!("Error handling disconnection: {:?}", e);
                            }
                        }
                        _ => {
                            // Other states (connecting, etc.) are ignored
                        }
                    }
                }
            }
        }

        info!("Supervisor monitoring stopped");
    }

    /// Gets a clone of the state receiver.
    ///
    /// Useful for advanced scenarios where you want to watch connection
    /// state directly.
    pub fn state_receiver(&self) -> watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }

    /// Gets the cancellation token.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}
