//! Connection state management for MQTT clients.
//!
//! This module provides types to track and communicate the current state of an MQTT connection,
//! including transient states like reconnection attempts. It's designed to be observable by
//! application code through watch channels, allowing for reactive connection monitoring.
//!
//! # Examples
//!
//! ```ignore
//! use mqtt_manager::ConnectionState;
//!
//! let state = ConnectionState::Reconnecting(5.0);
//! println!("Status: {}", state);  // "Reconnecting (in 5 seconds)"
//! println!("Type: {}", state.as_str());  // "Reconnecting"
//!
//! // Monitor for fatal errors
//! if let ConnectionState::FatalExit(reason) = &state {
//!     eprintln!("Connection cannot recover: {}", reason);
//!     // Application should decide whether to restart or exit
//! }
//! ```

use std::fmt;

/// Represents the current state of an MQTT connection.
///
/// The connection lifecycle flows through these states:
/// - `Connecting` -> `Connected` (successful handshake)
/// - `Connected` -> `Disconnected` (broker closed, network error, etc.)
/// - `Disconnected` -> `Reconnecting` -> `Connecting` -> ... (exponential backoff retry loop)
/// - `Disconnected` -> `FatalExit` (non-recoverable error, requires manual intervention)
///
/// State transitions are driven by the connection kernel, which monitors the underlying
/// MQTT event loop. Application code should subscribe to state changes via watch channels
/// to implement adaptive behavior (e.g., buffering publishes during disconnection).
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Actively attempting to establish a connection to the broker.
    ///
    /// This state indicates the client is sending CONNECT packets and waiting for CONNACK.
    /// No subscriptions or publishes will succeed in this state.
    Connecting,

    /// Successfully connected to the broker with active keep-alive.
    ///
    /// In this state, subscriptions are active, publishes are possible, and the client
    /// is responding to broker pings. This is the only state suitable for normal operation.
    Connected,

    /// Connection lost, either due to broker termination or network failure.
    ///
    /// The `String` field contains the reason for disconnection, which may be:
    /// - A broker-initiated disconnect (e.g., "Disconnected by broker")
    /// - A network error (e.g., "Connection refused")
    /// - A timeout or other condition
    ///
    /// The client will automatically attempt to reconnect per the configured backoff policy,
    /// unless the error is classified as fatal (in which case transitions to `FatalExit`).
    Disconnected(String),

    /// Waiting before the next reconnection attempt (exponential backoff).
    ///
    /// The `f64` field represents seconds until reconnection is attempted. This allows
    /// applications to:
    /// - Display progress indicators to users
    /// - Estimate recovery time
    /// - Implement custom retry logic by cancelling and forcing reconnection
    ///
    /// # Note on backoff algorithm
    /// The delay increases exponentially with each attempt up to a configured maximum.
    /// For example: 1s -> 1.1s -> 1.21s -> ... -> 60s (default cap).
    Reconnecting(f64),

    /// A non-recoverable error has occurred; automatic reconnection will not be attempted.
    ///
    /// This state indicates a fundamental issue that cannot be resolved by simply
    /// reestablishing the connection. The `String` field contains detailed diagnostic
    /// information about what went wrong.
    ///
    /// # Causes
    ///
    /// Typical reasons for entering `FatalExit` state include:
    /// - **TLS/SSL errors**: Invalid certificates, expired credentials, or incompatible
    ///   cipher suites
    /// - **Authentication failures**: Invalid username/password that won't succeed on retry
    /// - **Protocol violations**: Broker responded in an unexpected, irrecoverable way
    /// - **Configuration errors**: Client ID conflicts, unsupported protocol versions, etc.
    /// - **System resource exhaustion**: File descriptors, memory, or other OS limits
    ///
    /// # Application Response
    ///
    /// When the connection enters `FatalExit` state, applications should:
    /// 1. **Log the error** with full details for diagnostics
    /// 2. **Stop publishing** new messages (they will fail)
    /// 3. **Decide on recovery strategy**:
    ///    - Restart the entire connection with corrected configuration
    ///    - Notify users/admin of the problem
    ///    - Switch to a backup/fallback system
    ///    - Gracefully shut down the application
    ///
    /// # Example Errors
    ///
    /// ```ignore
    /// ConnectionState::FatalExit("TLS handshake failed: certificate expired".into())
    /// ConnectionState::FatalExit("Authentication rejected: invalid credentials".into())
    /// ConnectionState::FatalExit("Protocol violation: broker sent malformed packet".into())
    /// ```
    ///
    /// # Note
    /// This state is terminal for the current connection instance. To recover,
    /// applications must create a new `MqttManager` or restart the connection
    /// with corrected parameters.
    FatalExit(String),
}

impl ConnectionState {
    /// Returns a short string identifier for the current state.
    ///
    /// This is useful for logging, metrics, and UI display where detailed information
    /// isn't needed. The returned string is always a static lifetime (no allocations).
    ///
    /// # Returns
    /// One of: `"Connecting"`, `"Connected"`, `"Disconnected"`, `"Reconnecting"`, `"FatalExit"`
    ///
    /// # Examples
    /// ```ignore
    /// assert_eq!(ConnectionState::Connected.as_str(), "Connected");
    /// assert_eq!(ConnectionState::Disconnected("error".into()).as_str(), "Disconnected");
    /// assert_eq!(ConnectionState::FatalExit("tls error".into()).as_str(), "FatalExit");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionState::Connecting => "Connecting",
            ConnectionState::Connected => "Connected",
            ConnectionState::Disconnected(_) => "Disconnected",
            ConnectionState::Reconnecting(_) => "Reconnecting",
            ConnectionState::FatalExit(_) => "FatalExit",
        }
    }

    /// Returns contextual details about the current state.
    ///
    /// For `Connecting` and `Connected`, this returns an empty string.
    /// For `Disconnected`, it returns the disconnection reason.
    /// For `Reconnecting`, it returns the delay until next attempt.
    /// For `FatalExit`, it returns the detailed error description.
    ///
    /// # Examples
    /// ```ignore
    /// let state = ConnectionState::Reconnecting(5.5);
    /// assert_eq!(state.details(), "in 5.5 seconds");
    ///
    /// let state = ConnectionState::Disconnected("Connection timeout".into());
    /// assert_eq!(state.details(), "Connection timeout");
    ///
    /// let state = ConnectionState::FatalExit("TLS certificate expired".into());
    /// assert_eq!(state.details(), "TLS certificate expired");
    /// ```
    pub fn details(&self) -> String {
        match self {
            ConnectionState::Connecting => String::new(),
            ConnectionState::Connected => String::new(),
            ConnectionState::Disconnected(reason) => reason.clone(),
            ConnectionState::Reconnecting(seconds) => format!("in {seconds} seconds"),
            ConnectionState::FatalExit(reason) => reason.clone(),
        }
    }

    /// Checks if the connection is currently active.
    ///
    /// Returns true only if in `Connected` state, indicating that publishes
    /// and subscriptions will succeed.
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    /// Checks if the client is actively attempting to connect.
    ///
    /// Returns true for `Connecting`, `Reconnecting`, and initial connection attempts.
    /// Useful for showing "connecting" spinners or progress indicators.
    pub fn is_connecting(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connecting | ConnectionState::Reconnecting(_)
        )
    }

    /// Checks if the connection has encountered a fatal, non-recoverable error.
    ///
    /// Returns true only for `FatalExit` state. This indicates that automatic
    /// reconnection will not be attempted and manual intervention is required.
    ///
    /// # Examples
    /// ```ignore
    /// let state = ConnectionState::FatalExit("Invalid credentials".into());
    /// assert!(state.is_fatal());
    ///
    /// let state = ConnectionState::Disconnected("Network timeout".into());
    /// assert!(!state.is_fatal()); // Will not attempt to reconnect
    /// ```
    pub fn is_fatal(&self) -> bool {
        matches!(self, ConnectionState::FatalExit(_))
    }

    /// Checks if the connection can potentially become active again.
    ///
    /// Returns true for states that can transition to `Connected` through
    /// normal operation or automatic reconnection.
    ///
    /// # Returns
    /// - `true`: `Connecting`, `Reconnecting`, `Disconnected` (non-fatal)
    /// - `false`: `Connected`, `FatalExit`
    ///
    /// # Examples
    /// ```ignore
    /// assert!(ConnectionState::Reconnecting(2.0).is_recoverable());
    /// assert!(ConnectionState::Disconnected("timeout".into()).is_recoverable());
    /// assert!(!ConnectionState::FatalExit("bad cert".into()).is_recoverable());
    /// assert!(!ConnectionState::Connected.is_recoverable()); // Already connected
    /// ```
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connecting
                | ConnectionState::Reconnecting(_)
                | ConnectionState::Disconnected(_)
        )
    }
}

impl fmt::Display for ConnectionState {
    /// Formats the state as a human-readable string with optional details.
    ///
    /// # Examples
    /// ```ignore
    /// println!("{}", ConnectionState::Connected);  // "Connected"
    /// println!("{}", ConnectionState::Reconnecting(2.5));  // "Reconnecting (in 2.5 seconds)"
    /// println!("{}", ConnectionState::Disconnected("timeout".into()));  // "Disconnected (timeout)"
    /// println!("{}", ConnectionState::FatalExit("TLS error".into()));  // "FatalExit (TLS error)"
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())?;
        let details = self.details();
        if !details.is_empty() {
            write!(f, " ({details})")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_as_str() {
        assert_eq!(ConnectionState::Connecting.as_str(), "Connecting");
        assert_eq!(ConnectionState::Connected.as_str(), "Connected");
        assert_eq!(
            ConnectionState::Disconnected("test".into()).as_str(),
            "Disconnected"
        );
        assert_eq!(ConnectionState::Reconnecting(1.0).as_str(), "Reconnecting");
        assert_eq!(
            ConnectionState::FatalExit("error".into()).as_str(),
            "FatalExit"
        );
    }

    #[test]
    fn test_state_details() {
        assert_eq!(ConnectionState::Connecting.details(), "");
        assert_eq!(ConnectionState::Connected.details(), "");
        assert_eq!(
            ConnectionState::Disconnected("network error".into()).details(),
            "network error"
        );
        assert_eq!(
            ConnectionState::Reconnecting(3.5).details(),
            "in 3.5 seconds"
        );
        assert_eq!(
            ConnectionState::FatalExit("TLS handshake failed".into()).details(),
            "TLS handshake failed"
        );
    }

    #[test]
    fn test_state_display() {
        assert_eq!(ConnectionState::Connected.to_string(), "Connected");
        assert_eq!(
            ConnectionState::Reconnecting(2.0).to_string(),
            "Reconnecting (in 2 seconds)"
        );
        assert_eq!(
            ConnectionState::Disconnected("broker closed".into()).to_string(),
            "Disconnected (broker closed)"
        );
        assert_eq!(
            ConnectionState::FatalExit("certificate expired".into()).to_string(),
            "FatalExit (certificate expired)"
        );
    }

    #[test]
    fn test_is_connected() {
        assert!(ConnectionState::Connected.is_connected());
        assert!(!ConnectionState::Connecting.is_connected());
        assert!(!ConnectionState::Disconnected("error".into()).is_connected());
        assert!(!ConnectionState::Reconnecting(1.0).is_connected());
        assert!(!ConnectionState::FatalExit("error".into()).is_connected());
    }

    #[test]
    fn test_is_connecting() {
        assert!(ConnectionState::Connecting.is_connecting());
        assert!(ConnectionState::Reconnecting(1.0).is_connecting());
        assert!(!ConnectionState::Connected.is_connecting());
        assert!(!ConnectionState::Disconnected("error".into()).is_connecting());
        assert!(!ConnectionState::FatalExit("error".into()).is_connecting());
    }

    #[test]
    fn test_is_fatal() {
        assert!(ConnectionState::FatalExit("error".into()).is_fatal());
        assert!(!ConnectionState::Connected.is_fatal());
        assert!(!ConnectionState::Disconnected("error".into()).is_fatal());
        assert!(!ConnectionState::Reconnecting(1.0).is_fatal());
        assert!(!ConnectionState::Connecting.is_fatal());
    }

    #[test]
    fn test_is_recoverable() {
        assert!(ConnectionState::Connecting.is_recoverable());
        assert!(ConnectionState::Reconnecting(1.0).is_recoverable());
        assert!(ConnectionState::Disconnected("error".into()).is_recoverable());
        assert!(!ConnectionState::Connected.is_recoverable());
        assert!(!ConnectionState::FatalExit("error".into()).is_recoverable());
    }
}
