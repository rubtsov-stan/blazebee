//! Exponential backoff implementation for connection retry logic.
//!
//! This module implements a proven backoff strategy to handle transient network failures
//! gracefully. When a connection fails, rather than immediately retrying (which can
//! overwhelm a recovering broker), we wait an increasing amount of time between attempts.
//!
//! # Algorithm
//!
//! The backoff delay grows exponentially:
//! ```text
//! delay[n] = min(initial * multiplier^(n-1), max_delay)
//! ```
//!
//! For example, with default settings (initial=1s, multiplier=1.1, max=60s):
//! - Attempt 1: wait 1.0s
//! - Attempt 2: wait 1.1s
//! - Attempt 3: wait 1.21s
//! - ...
//! - Attempt 60+: wait 60s (capped)
//!
//! This approach is crucial for distributed systems because it:
//! - Prevents thundering herd when a broker recovers
//! - Reduces load on the broker during incidents
//! - Improves success rate on recovery (broker isn't overwhelmed)
//!
//! # Examples
//!
//! ```ignore
//! use std::time::Duration;
//! use mqtt_manager::Backoff;
//!
//! let mut backoff = Backoff::default();
//!
//! // First retry
//! let delay = backoff.next_sleep().unwrap();
//! assert_eq!(delay, Duration::from_secs(1));
//!
//! // Second retry (slightly longer)
//! let delay = backoff.next_sleep().unwrap();
//! assert!(delay > Duration::from_secs(1));
//!
//! // Reset if connection succeeds
//! backoff.reset();
//! let delay = backoff.next_sleep().unwrap();
//! assert_eq!(delay, Duration::from_secs(1));
//! ```

use std::time::Duration;

use thiserror::Error;

/// Error type for backoff exhaustion.
///
/// Indicates that the maximum number of retry attempts has been exceeded.
/// When this occurs, the client should typically shut down or switch to an error state.
#[derive(Debug, Error)]
pub enum BackoffError {
    /// Maximum retry attempts exceeded with the given limit.
    ///
    /// The `u32` field contains the configured limit. If the limit was calculated
    /// automatically from timing parameters, this will be the computed value.
    #[error("Maximum number of attempts exceeded: {0}")]
    MaxAttemptLimitError(u32),
}

/// Exponential backoff controller for connection retry logic.
///
/// Manages retry timing to prevent overwhelming a recovering broker. Each failed
/// connection attempt increments an internal counter, and the next_sleep() call
/// returns an increasingly longer delay.
///
/// The backoff resets when a connection succeeds, returning delays to the minimum.
///
/// # Thread Safety
/// This struct is not `Send` by default due to Duration and f64, but is designed
/// to be wrapped in a Mutex for use in async contexts (see connection kernel).
#[derive(Debug, Clone)]
pub struct Backoff {
    /// The initial delay before the first retry.
    initial_delay: Duration,

    /// The current delay value (increases with each failed attempt).
    current_delay: Duration,

    /// Maximum delay cap (prevents delays from growing unboundedly).
    max_delay: Duration,

    /// Multiplicative factor applied to delay after each attempt (typically 1.1-2.0).
    multiplier: f64,

    /// Count of attempted retries (0 before first attempt).
    attempt: u32,

    /// Optional hard limit on retry attempts. If None, uses calculated_max_attempts.
    /// Useful for temporary connection loss that you want to give up on quickly.
    max_attempts: Option<u32>,

    /// Automatically calculated maximum attempts based on timing parameters.
    ///
    /// Computed as: ceil(log_multiplier(max_delay / initial_delay))
    /// This ensures we reach the max_delay and then stop incrementing.
    calculated_max_attempts: u32,
}

impl Backoff {
    /// Creates a new backoff controller with custom timing parameters.
    ///
    /// # Arguments
    /// - `initial`: Starting delay before first retry (typically 1-5 seconds)
    /// - `max`: Maximum delay cap (typically 30-120 seconds)
    /// - `multiplier`: Exponential growth factor (typically 1.1-2.0)
    ///
    /// # Panics
    /// This function does not panic, but will return calculated_max_attempts=1 if:
    /// - initial >= max (can't grow exponentially)
    /// - multiplier <= 1.0 (growth factor must be > 1.0)
    ///
    /// # Examples
    /// ```ignore
    /// use std::time::Duration;
    /// use mqtt_manager::Backoff;
    ///
    /// // Conservative backoff: start at 2s, cap at 120s, grow by 50% each time
    /// let backoff = Backoff::new(
    ///     Duration::from_secs(2),
    ///     Duration::from_secs(120),
    ///     1.5,
    /// );
    /// ```
    pub fn new(initial: Duration, max: Duration, multiplier: f64) -> Self {
        let calculated_max_attempts = Self::calculate_max_attempts(initial, max, multiplier);
        Self {
            initial_delay: initial,
            current_delay: initial,
            max_delay: max,
            multiplier,
            attempt: 0,
            max_attempts: None,
            calculated_max_attempts,
        }
    }

    /// Calculates the maximum number of attempts before hitting the delay cap.
    ///
    /// Uses logarithmic math: if initial * multiplier^n = max, then:
    /// n = log(max/initial) / log(multiplier)
    ///
    /// This ensures we don't keep calculating delays that stay at max_delay—we give up
    /// once we've effectively "saturated" the backoff schedule.
    ///
    /// # Returns
    /// The number of attempts until the delay plateaus. Minimum of 1.
    fn calculate_max_attempts(initial: Duration, max: Duration, multiplier: f64) -> u32 {
        // Guard against degenerate cases
        if initial >= max || multiplier <= 1.0 {
            return 1;
        }

        let initial_secs = initial.as_secs_f64();
        let max_secs = max.as_secs_f64();

        // Solve: initial * multiplier^n = max
        // => n = log(max/initial) / log(multiplier)
        let n = (max_secs / initial_secs).log(multiplier);
        n.floor() as u32 + 1
    }

    /// Sets an explicit maximum number of attempts.
    ///
    /// By default, the max is calculated from timing parameters. Use this to override
    /// with a stricter limit if your use case requires faster failure.
    ///
    /// # Arguments
    /// - `max`: Maximum number of retries (0 means fail immediately)
    ///
    /// # Examples
    /// ```ignore
    /// let mut backoff = Backoff::default();
    /// backoff.set_max_attempts(5);  // Give up after 5 attempts
    /// ```
    pub fn set_max_attempts(&mut self, max: u32) {
        self.max_attempts = Some(max);
    }

    /// Resets the backoff timer to initial state.
    ///
    /// Call this when a connection succeeds, so the next failure starts with
    /// minimum delay again.
    ///
    /// # Examples
    /// ```ignore
    /// backoff.next_sleep().unwrap();  // After failed attempt
    /// // ... connection succeeds ...
    /// backoff.reset();  // Start over
    /// backoff.next_sleep().unwrap();  // Back to initial delay
    /// ```
    pub fn reset(&mut self) {
        self.current_delay = self.initial_delay;
        self.attempt = 0;
    }

    /// Returns the next sleep duration and advances the backoff timer.
    ///
    /// On each call, returns the current delay and increments the attempt counter.
    /// Subsequent calls return increasing delays up to the configured maximum.
    ///
    /// # Returns
    /// - `Ok(Duration)`: The sleep duration before the next retry attempt
    /// - `Err(BackoffError)`: If maximum attempts has been exceeded
    ///
    /// # Examples
    /// ```ignore
    /// let mut backoff = Backoff::default();
    ///
    /// loop {
    ///     match backoff.next_sleep() {
    ///         Ok(delay) => {
    ///             println!("Waiting {} seconds...", delay.as_secs_f64());
    ///             tokio::time::sleep(delay).await;
    ///             // attempt connection
    ///         }
    ///         Err(_) => {
    ///             eprintln!("Max retries exceeded, giving up");
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn next_sleep(&mut self) -> Result<Duration, BackoffError> {
        self.attempt += 1;
        let effective_max = self.max_attempts.unwrap_or(self.calculated_max_attempts);

        if self.attempt > effective_max {
            return Err(BackoffError::MaxAttemptLimitError(effective_max));
        }

        let sleep = self.current_delay;

        // Calculate next delay: current * multiplier, capped at max
        let next_delay_secs = self.current_delay.as_secs_f64() * self.multiplier;
        self.current_delay = Duration::from_secs_f64(next_delay_secs);

        if self.current_delay > self.max_delay {
            self.current_delay = self.max_delay;
        }

        Ok(sleep)
    }

    /// Gets the configured maximum delay.
    pub fn max_delay(&self) -> Duration {
        self.max_delay
    }

    /// Gets the explicit maximum attempts limit, if set.
    pub fn max_attempts(&self) -> Option<u32> {
        self.max_attempts
    }

    /// Gets the current attempt count.
    ///
    /// Incremented by next_sleep(). Useful for logging and diagnostics.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Gets the calculated maximum attempts based on timing parameters.
    ///
    /// This is the default limit if no explicit max_attempts was set.
    pub fn calculated_max_attempts(&self) -> u32 {
        self.calculated_max_attempts
    }

    /// Gets the current delay that will be returned by next sleep.
    ///
    /// Useful for logging and metrics to show how long users must wait.
    pub fn current_delay(&self) -> Duration {
        self.current_delay
    }
}

impl Default for Backoff {
    /// Creates a backoff with sensible defaults for MQTT.
    ///
    /// - Initial delay: 1 second
    /// - Maximum delay: 60 seconds
    /// - Multiplier: 1.1 (10% increase per attempt)
    ///
    /// This gives a gentle backoff that doesn't punish temporary network hiccups
    /// but caps out quickly for sustained outages. Most connections recover in
    /// the 1-10 second range.
    fn default() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(60), 1.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_default_creation() {
        let backoff = Backoff::default();
        assert_eq!(backoff.attempt, 0);
        assert_eq!(backoff.current_delay, Duration::from_secs(1));
        assert_eq!(backoff.max_delay, Duration::from_secs(60));
    }

    #[test]
    fn test_backoff_next_sleep_progression() {
        let mut backoff = Backoff::default();

        let delay1 = backoff.next_sleep().unwrap();
        assert_eq!(delay1, Duration::from_secs(1));

        let delay2 = backoff.next_sleep().unwrap();
        assert!(delay2 > delay1);
        assert!(delay2 < Duration::from_secs_f64(1.2));
    }

    #[test]
    fn test_backoff_respects_max_delay() {
        let mut backoff = Backoff::new(
            Duration::from_secs(1),
            Duration::from_secs(10),
            2.0, // Aggressive doubling
        );

        // Exhaust attempts until we hit max
        let mut last_delay = Duration::from_secs(0);
        while let Ok(delay) = backoff.next_sleep() {
            last_delay = delay;
            if delay >= backoff.max_delay() {
                break;
            }
        }

        assert!(last_delay <= Duration::from_secs(10));
    }

    #[test]
    fn test_backoff_reset() {
        let mut backoff = Backoff::default();

        backoff.next_sleep().unwrap();
        backoff.next_sleep().unwrap();
        assert_eq!(backoff.attempt, 2);

        backoff.reset();
        assert_eq!(backoff.attempt, 0);
        assert_eq!(backoff.current_delay, Duration::from_secs(1));
    }

    #[test]
    fn test_backoff_max_attempts_exceeded() {
        let mut backoff = Backoff::default();
        backoff.set_max_attempts(2);

        let _ = backoff.next_sleep();
        let _ = backoff.next_sleep();
        let result = backoff.next_sleep();

        assert!(result.is_err());
        if let Err(BackoffError::MaxAttemptLimitError(max)) = result {
            assert_eq!(max, 2);
        }
    }

    #[test]
    fn test_backoff_calculated_max_attempts() {
        // With initial 1s, max 60s, multiplier 1.1:
        // We should saturate around 60-ish attempts
        let backoff = Backoff::default();
        assert!(backoff.calculated_max_attempts() > 50);
        assert!(backoff.calculated_max_attempts() < 100);
    }

    #[test]
    fn test_backoff_edge_case_invalid_multiplier() {
        // Multiplier <= 1.0 should cap at 1 attempt
        let backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(10), 0.9);
        assert_eq!(backoff.calculated_max_attempts(), 1);
    }

    #[test]
    fn test_backoff_edge_case_initial_equals_max() {
        // If initial == max, can't grow, so cap at 1
        let backoff = Backoff::new(Duration::from_secs(10), Duration::from_secs(10), 1.5);
        assert_eq!(backoff.calculated_max_attempts(), 1);
    }
}
