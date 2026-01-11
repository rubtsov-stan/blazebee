//! Readiness state management for the application.
//!
//! This module provides a mechanism to track and propagate the operational
//! readiness of the system. Readiness is typically tied to external dependencies
//! such as MQTT broker connectivity. Components can subscribe to readiness
//! changes and delay their operation until the system reports `Ready`.

use std::fmt;

use tokio::sync::watch;
use tracing::{debug, warn};

/// Represents the current readiness state of the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadinessState {
    /// The system is fully operational and ready to perform its primary tasks.
    Ready,
    /// The system is not ready, with an optional reason describing the cause.
    NotReadyYet(String),
    /// The readiness state has not yet been determined.
    Unknown,
}

impl ReadinessState {
    /// Returns true if the system is ready.
    pub fn is_ready(&self) -> bool {
        matches!(self, ReadinessState::Ready)
    }

    /// Returns a short string representation of the state.
    pub fn as_str(&self) -> &str {
        match self {
            ReadinessState::Ready => "Ready",
            ReadinessState::NotReadyYet(_) => "NotReadyYet",
            ReadinessState::Unknown => "Unknown",
        }
    }

    /// Returns the reason string if the state is `NotReadyYet`, otherwise an empty string.
    pub fn reason(&self) -> &str {
        match self {
            ReadinessState::NotReadyYet(reason) => reason,
            _ => "",
        }
    }
}

impl fmt::Display for ReadinessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadinessState::Ready => write!(f, "Ready"),
            ReadinessState::NotReadyYet(reason) => write!(f, "NotReadyYet: {}", reason),
            ReadinessState::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Listener implementation that translates MQTT connection state into readiness state.
#[cfg(feature = "blazebee-mqtt-v4")]
pub mod listener {
    use blazebee_mqtt_v4::state::ConnectionState;

    use super::*;

    /// Spawns a task that listens to MQTT connection state changes and updates
    /// the shared readiness state accordingly.
    pub async fn listen(
        mut connection_state_rx: watch::Receiver<ConnectionState>,
        state_tx: watch::Sender<ReadinessState>,
    ) {
        debug!("Launching ConnectionState listening (blazebee-mqtt-v4)");

        // Send initial state
        {
            let conn_state = connection_state_rx.borrow().clone();
            let readiness_state = adapt_connection_state(&conn_state);
            debug!("Initial state of MQTT: {:?}", conn_state);
            let _ = state_tx.send(readiness_state.clone());
            debug!("Initial readiness status: {}", readiness_state);
        }

        // React to subsequent changes
        while connection_state_rx.changed().await.is_ok() {
            let conn_state = connection_state_rx.borrow().clone();
            let readiness_state = adapt_connection_state(&conn_state);
            debug!(
                "Transition: MQTT {:?} -> readiness {}",
                conn_state, readiness_state
            );

            if state_tx.send(readiness_state).is_err() {
                warn!("No subscribers to readiness status");
                break;
            }
        }

        debug!("ConnectionState channel closed, listening stopped");
    }

    /// Maps an MQTT connection state to the corresponding readiness state.
    fn adapt_connection_state(state: &ConnectionState) -> ReadinessState {
        match state {
            ConnectionState::Connected => ReadinessState::Ready,
            ConnectionState::Disconnected(reason) => {
                ReadinessState::NotReadyYet(format!("Disconnected: {}", reason))
            }
            ConnectionState::Reconnecting(secs) => {
                ReadinessState::NotReadyYet(format!("Reconnecting in {:.1} sec", secs))
            }
            ConnectionState::Connecting => ReadinessState::NotReadyYet("Connecting...".to_string()),
        }
    }
}

/// Shared readiness tracker that allows multiple components to observe state changes.
#[derive(Debug, Clone)]
pub struct Readiness {
    state_tx: watch::Sender<ReadinessState>,
    state_rx: watch::Receiver<ReadinessState>,
}

impl Readiness {
    /// Creates a new readiness tracker with an initial `Unknown` state.
    pub fn new() -> Self {
        let (state_tx, state_rx) = watch::channel(ReadinessState::Unknown);
        Self { state_tx, state_rx }
    }

    /// Returns a receiver that can be used to subscribe to readiness changes.
    pub fn subscribe(&self) -> watch::Receiver<ReadinessState> {
        self.state_rx.clone()
    }

    /// Returns the current readiness state without subscribing.
    pub fn current_state(&self) -> ReadinessState {
        self.state_rx.borrow().clone()
    }

    /// Starts a background task that translates MQTT connection state into readiness state.
    ///
    /// This method is only available when the `blazebee-mqtt-v4` feature is enabled.
    #[cfg(feature = "blazebee-mqtt-v4")]
    pub async fn start_listening(
        &self,
        connection_state_rx: watch::Receiver<blazebee_mqtt_v4::state::ConnectionState>,
    ) {
        let state_tx = self.state_tx.clone();
        tokio::spawn(async move {
            listener::listen(connection_state_rx, state_tx).await;
        });
    }

    /// Manually updates the readiness state.
    ///
    /// Logs the transition at debug level.
    pub fn set_state(&self, state: ReadinessState) {
        let old_state = self.state_rx.borrow().clone();
        let _ = self.state_tx.send(state.clone());
        debug!(
            "The readiness status has changed: {} -> {}",
            old_state, state
        );
    }
}

impl Default for Readiness {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readiness_state_is_ready() {
        assert!(ReadinessState::Ready.is_ready());
        assert!(!ReadinessState::Unknown.is_ready());
        assert!(!ReadinessState::NotReadyYet("error".into()).is_ready());
    }

    #[test]
    fn test_readiness_state_as_str() {
        assert_eq!(ReadinessState::Ready.as_str(), "Ready");
        assert_eq!(ReadinessState::Unknown.as_str(), "Unknown");
        assert_eq!(
            ReadinessState::NotReadyYet("reason".into()).as_str(),
            "NotReadyYet"
        );
    }

    #[test]
    fn test_readiness_state_reason() {
        assert_eq!(ReadinessState::Ready.reason(), "");
        assert_eq!(
            ReadinessState::NotReadyYet("test reason".into()).reason(),
            "test reason"
        );
    }

    #[test]
    fn test_readiness_state_display() {
        assert_eq!(ReadinessState::Ready.to_string(), "Ready");
        assert_eq!(ReadinessState::Unknown.to_string(), "Unknown");
        assert!(ReadinessState::NotReadyYet("error".into())
            .to_string()
            .contains("error"));
    }

    #[tokio::test]
    async fn test_readiness_creation() {
        let readiness = Readiness::new();
        assert_eq!(readiness.current_state(), ReadinessState::Unknown);
    }

    #[tokio::test]
    async fn test_readiness_subscribe() {
        let readiness = Readiness::new();
        let state_rx = readiness.subscribe();
        assert_eq!(*state_rx.borrow(), ReadinessState::Unknown);
    }

    #[tokio::test]
    async fn test_readiness_set_state() {
        let readiness = Readiness::new();
        let mut state_rx = readiness.subscribe();
        readiness.set_state(ReadinessState::Ready);
        state_rx.changed().await.unwrap();
        assert_eq!(*state_rx.borrow(), ReadinessState::Ready);
    }

    #[tokio::test]
    async fn test_readiness_multiple_subscribers() {
        let readiness = Readiness::new();
        let mut rx1 = readiness.subscribe();
        let mut rx2 = readiness.subscribe();
        readiness.set_state(ReadinessState::Ready);
        rx1.changed().await.unwrap();
        rx2.changed().await.unwrap();
        assert_eq!(*rx1.borrow(), ReadinessState::Ready);
        assert_eq!(*rx2.borrow(), ReadinessState::Ready);
    }

    #[test]
    fn test_readiness_state_equality() {
        assert_eq!(ReadinessState::Ready, ReadinessState::Ready);
        assert_ne!(ReadinessState::Ready, ReadinessState::Unknown);
        let reason1 = ReadinessState::NotReadyYet("error".into());
        let reason2 = ReadinessState::NotReadyYet("error".into());
        assert_eq!(reason1, reason2);
    }
}
