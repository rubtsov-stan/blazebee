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
//! ```

use std::fmt;

/// Represents the current state of an MQTT connection.
///
/// The connection lifecycle flows through these states:
/// - `Connecting` -> `Connected` (successful handshake)
/// - `Connected` -> `Disconnected` (broker closed, network error, etc.)
/// - `Disconnected` -> `Reconnecting` -> `Connecting` -> ... (exponential backoff retry loop)
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
    /// - A timeout or other fatal condition
    ///
    /// The client will automatically attempt to reconnect per the configured backoff policy.
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
}

impl ConnectionState {
    /// Returns a short string identifier for the current state.
    ///
    /// This is useful for logging, metrics, and UI display where detailed information
    /// isn't needed. The returned string is always a static lifetime (no allocations).
    ///
    /// # Returns
    /// One of: `"Connecting"`, `"Connected"`, `"Disconnected"`, `"Reconnecting"`
    ///
    /// # Examples
    /// ```ignore
    /// assert_eq!(ConnectionState::Connected.as_str(), "Connected");
    /// assert_eq!(ConnectionState::Disconnected("error".into()).as_str(), "Disconnected");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionState::Connecting => "Connecting",
            ConnectionState::Connected => "Connected",
            ConnectionState::Disconnected(_) => "Disconnected",
            ConnectionState::Reconnecting(_) => "Reconnecting",
        }
    }

    /// Returns contextual details about the current state.
    ///
    /// For `Connecting` and `Connected`, this returns an empty string.
    /// For `Disconnected`, it returns the disconnection reason.
    /// For `Reconnecting`, it returns the delay until next attempt.
    ///
    /// # Examples
    /// ```ignore
    /// let state = ConnectionState::Reconnecting(5.5);
    /// assert_eq!(state.details(), "in 5.5 seconds");
    ///
    /// let state = ConnectionState::Disconnected("Connection timeout".into());
    /// assert_eq!(state.details(), "Connection timeout");
    /// ```
    pub fn details(&self) -> String {
        match self {
            ConnectionState::Connecting => String::new(),
            ConnectionState::Connected => String::new(),
            ConnectionState::Disconnected(reason) => reason.clone(),
            ConnectionState::Reconnecting(seconds) => format!("in {seconds} seconds"),
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
}

impl fmt::Display for ConnectionState {
    /// Formats the state as a human-readable string with optional details.
    ///
    /// # Examples
    /// ```ignore
    /// println!("{}", ConnectionState::Connected);  // "Connected"
    /// println!("{}", ConnectionState::Reconnecting(2.5));  // "Reconnecting (in 2.5 seconds)"
    /// println!("{}", ConnectionState::Disconnected("timeout".into()));  // "Disconnected (timeout)"
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
    }

    #[test]
    fn test_is_connected() {
        assert!(ConnectionState::Connected.is_connected());
        assert!(!ConnectionState::Connecting.is_connected());
        assert!(!ConnectionState::Disconnected("error".into()).is_connected());
        assert!(!ConnectionState::Reconnecting(1.0).is_connected());
    }

    #[test]
    fn test_is_connecting() {
        assert!(ConnectionState::Connecting.is_connecting());
        assert!(ConnectionState::Reconnecting(1.0).is_connecting());
        assert!(!ConnectionState::Connected.is_connecting());
        assert!(!ConnectionState::Disconnected("error".into()).is_connecting());
    }
}
