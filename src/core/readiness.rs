//! Readiness state management for the application.
//!
//! This module provides a mechanism to track and propagate the operational
//! readiness of the system. Readiness is typically tied to external dependencies
//! such as MQTT broker connectivity. Components can subscribe to readiness
//! changes and delay their operation until the system reports `Ready`.
//!
//! # State Semantics
//!
//! ```text
//! ┌─────────┐    ┌──────────┐    ┌─────────┐
//! │ Unknown │───▶│ NotReady │───▶│  Ready  │
//! └─────────┘    └──────────┘    └─────────┘
//!      │              │                │
//!      │              │                │
//!      ▼              ▼                ▼
//! ┌─────────────────────────────────────────┐
//! │             FatalExit                   │
//! │      (Terminal, Non-Recoverable)        │
//! └─────────────────────────────────────────┘
//! ```
//!
//! # Typical Usage
//!
//! Applications subscribe to readiness changes and:
//! - Wait for `Ready` before starting normal operations
//! - Handle `NotReadyYet` by retrying or showing user feedback
//! - Treat `FatalExit` as a signal to shut down gracefully

use std::fmt;

use tokio::sync::watch;
use tracing::{debug, warn};

/// Represents the current readiness state of the system.
///
/// Readiness is a system-wide property that indicates whether the application
/// can perform its primary functions. It's typically driven by critical
/// dependencies like MQTT connectivity, database access, or external services.
///
/// Applications should monitor readiness state and:
/// - Delay startup until `Ready`
/// - Pause operations during `NotReadyYet`
/// - Initiate graceful shutdown on `FatalExit`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadinessState {
    /// The system is fully operational and ready to perform its primary tasks.
    ///
    /// All critical dependencies are available and functioning normally.
    /// Application components can proceed with their normal operation.
    Ready,

    /// The system is not ready, with an optional reason describing the cause.
    ///
    /// This is a transient state that typically indicates temporary issues
    /// such as network connectivity problems, service initialization, or
    /// dependency unavailability.
    ///
    /// Applications should:
    /// - Log the reason for diagnostic purposes
    /// - Display user-appropriate feedback if applicable
    /// - Wait for transition to `Ready` before resuming normal operations
    /// - Consider implementing retry logic with exponential backoff
    NotReadyYet(String),

    /// A non-recoverable error has occurred; the system cannot become ready.
    ///
    /// This state indicates a fundamental failure that cannot be resolved
    /// through automatic retries or normal recovery procedures. The system
    /// has encountered a condition that requires manual intervention or
    /// application restart.
    ///
    /// # Causes
    ///
    /// Typical scenarios that lead to `FatalExit` include:
    /// - **Authentication failures**: Invalid credentials, expired tokens,
    ///   or permission denials that won't succeed on retry
    /// - **Configuration errors**: Invalid settings, missing resources, or
    ///   incompatible dependencies
    /// - **Protocol violations**: Unrecoverable communication errors with
    ///   external services
    /// - **Resource exhaustion**: System limits (file descriptors, memory)
    ///   that prevent normal operation
    /// - **Dependency failures**: Critical external services that are
    ///   permanently unavailable or misconfigured
    ///
    /// # Application Response
    ///
    /// When the system enters `FatalExit` state, applications should:
    ///
    /// 1. **Cease normal operations**: Stop processing new requests, messages,
    ///    or user interactions
    /// 2. **Log diagnostic information**: Capture the error reason with
    ///    full context for troubleshooting
    /// 3. **Initiate graceful shutdown**:
    ///    - Complete or abort in-progress operations
    ///    - Release acquired resources (connections, file handles, locks)
    ///    - Persist any necessary state for recovery
    ///    - Notify users or administrators of the failure
    /// 4. **Determine recovery strategy**:
    ///    - Restart the application with corrected configuration
    ///    - Switch to fallback/backup systems if available
    ///    - Escalate to human intervention if automatic recovery is impossible
    ///
    /// # Example
    ///
    /// ```ignore
    /// match readiness.current_state() {
    ///     ReadinessState::Ready => {
    ///         // Proceed with normal operation
    ///     }
    ///     ReadinessState::FatalExit(reason) => {
    ///         error!("System cannot recover: {}", reason);
    ///         // Initiate controlled shutdown
    ///         shutdown_gracefully().await;
    ///         // Optionally restart or exit
    ///         std::process::exit(1);
    ///     }
    ///     _ => {
    ///         // Wait for ready state
    ///     }
    /// }
    /// ```
    ///
    /// # Note on Recovery
    ///
    /// Unlike `NotReadyYet`, which implies eventual recovery through retry,
    /// `FatalExit` indicates that the current application instance cannot
    /// recover. A new instance may be started with corrected parameters,
    /// but the existing instance should terminate gracefully.
    FatalExit(String),

    /// The readiness state has not yet been determined.
    ///
    /// This is the initial state before any dependency checks or connectivity
    /// tests have been performed. Applications should treat this as equivalent
    /// to `NotReadyYet` but may want to distinguish it for logging or UI purposes.
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
            ReadinessState::FatalExit(_) => "FatalExit",
        }
    }

    /// Returns true if the state is `FatalExit`.
    ///
    /// This is a convenience method for quickly checking if the system
    /// has encountered an unrecoverable error.
    pub fn is_fatal(&self) -> bool {
        matches!(self, ReadinessState::FatalExit(_))
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
            ReadinessState::FatalExit(reason) => write!(f, "FatalExit: {}", reason),
        }
    }
}

/// Listener implementation that translates MQTT connection state into readiness state.
#[cfg(feature = "blazebee-mqtt-v3")]
pub mod listener {
    use blazebee_mqtt_v3::state::ConnectionState;

    use super::*;

    /// Spawns a task that listens to MQTT connection state changes and updates
    /// the shared readiness state accordingly.
    ///
    /// This function establishes the relationship between MQTT connectivity
    /// and system readiness. When MQTT connection fails fatally, the system
    /// readiness transitions to `FatalExit`.
    pub async fn listen(
        mut connection_state_rx: watch::Receiver<ConnectionState>,
        state_tx: watch::Sender<ReadinessState>,
    ) {
        debug!("Launching ConnectionState listening (blazebee-mqtt-v3)");

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
    ///
    /// This mapping determines how MQTT connectivity issues affect overall
    /// system readiness. Fatal MQTT errors propagate as `FatalExit` readiness.
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
            ConnectionState::FatalExit(reason) => {
                ReadinessState::FatalExit(format!("Fatal error: {}", reason))
            }
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
    /// This method is only available when the `blazebee-mqtt-v3` feature is enabled.
    /// When MQTT connection enters `FatalExit`, system readiness will also become `FatalExit`.
    #[cfg(feature = "blazebee-mqtt-v3")]
    pub async fn start_listening(
        &self,
        connection_state_rx: watch::Receiver<blazebee_mqtt_v3::state::ConnectionState>,
    ) {
        let state_tx = self.state_tx.clone();
        tokio::spawn(async move {
            listener::listen(connection_state_rx, state_tx).await;
        });
    }

    /// Manually updates the readiness state.
    ///
    /// Logs the transition at debug level.
    ///
    /// # Usage
    ///
    /// This method can be used to manually set readiness state when:
    /// - Application components detect their own fatal errors
    /// - System-wide conditions warrant readiness changes
    /// - Testing or simulation requires specific readiness states
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Manual fatal error reporting
    /// readiness.set_state(ReadinessState::FatalExit(
    ///     "Something went wrong".into()
    /// ));
    /// ```
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
        assert!(!ReadinessState::FatalExit("fatal".into()).is_ready());
    }

    #[test]
    fn test_readiness_state_is_fatal() {
        assert!(ReadinessState::FatalExit("error".into()).is_fatal());
        assert!(!ReadinessState::Ready.is_fatal());
        assert!(!ReadinessState::Unknown.is_fatal());
        assert!(!ReadinessState::NotReadyYet("error".into()).is_fatal());
    }

    #[test]
    fn test_readiness_state_as_str() {
        assert_eq!(ReadinessState::Ready.as_str(), "Ready");
        assert_eq!(ReadinessState::Unknown.as_str(), "Unknown");
        assert_eq!(
            ReadinessState::NotReadyYet("reason".into()).as_str(),
            "NotReadyYet"
        );
        assert_eq!(
            ReadinessState::FatalExit("fatal".into()).as_str(),
            "FatalExit"
        );
    }

    #[test]
    fn test_readiness_state_reason() {
        assert_eq!(ReadinessState::Ready.reason(), "");
        assert_eq!(
            ReadinessState::NotReadyYet("test reason".into()).reason(),
            "test reason"
        );
        assert_eq!(ReadinessState::FatalExit("fatal".into()).reason(), "");
    }

    #[test]
    fn test_readiness_state_display() {
        assert_eq!(ReadinessState::Ready.to_string(), "Ready");
        assert_eq!(ReadinessState::Unknown.to_string(), "Unknown");
        assert!(ReadinessState::NotReadyYet("error".into())
            .to_string()
            .contains("error"));
        assert!(ReadinessState::FatalExit("fatal error".into())
            .to_string()
            .contains("fatal error"));
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
    async fn test_readiness_set_fatal_state() {
        let readiness = Readiness::new();
        let mut state_rx = readiness.subscribe();
        readiness.set_state(ReadinessState::FatalExit("test fatal".into()));
        state_rx.changed().await.unwrap();
        let state = state_rx.borrow().clone();
        match state {
            ReadinessState::FatalExit(reason) => assert_eq!(reason, "test fatal"),
            _ => panic!("Expected FatalExit"),
        }
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
        let fatal1 = ReadinessState::FatalExit("fatal".into());
        let fatal2 = ReadinessState::FatalExit("fatal".into());
        assert_eq!(fatal1, fatal2);
    }
}
