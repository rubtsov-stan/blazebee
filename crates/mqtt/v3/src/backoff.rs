//! Capped exponential backoff with jitter for reconnection attempts.
//!
//! The delay sequence is `initial, initial*factor, ...` capped at `max`, each
//! value scaled by a random factor in `1 ± jitter`. There is no attempt
//! limit: a long-running agent must keep trying until the broker returns or
//! the process is stopped. Jitter spreads reconnects of many agents after a
//! broker restart.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
    factor: f64,
    jitter: f64,
    next: Duration,
    rng: u64,
}

impl Backoff {
    /// `factor` below 1.0 is treated as 1.0; `max` below `initial` is raised to `initial`.
    pub fn new(initial: Duration, max: Duration, factor: f64) -> Self {
        let max = max.max(initial);
        Self {
            initial,
            max,
            factor: if factor.is_finite() && factor >= 1.0 {
                factor
            } else {
                1.0
            },
            jitter: 0.0,
            next: initial,
            rng: seed(),
        }
    }

    /// Scales every delay by a uniform random factor in `1 ± fraction`.
    /// `fraction` is clamped to `0.0..=0.5`.
    pub fn with_jitter(mut self, fraction: f64) -> Self {
        self.jitter = if fraction.is_finite() {
            fraction.clamp(0.0, 0.5)
        } else {
            0.0
        };
        self
    }

    /// Restarts the sequence from the initial delay. Call after a successful connection.
    pub fn reset(&mut self) {
        self.next = self.initial;
    }

    /// Returns the delay to wait before the next attempt and advances the sequence.
    pub fn next_delay(&mut self) -> Duration {
        let base = self.next;
        self.next =
            Duration::from_secs_f64((base.as_secs_f64() * self.factor).min(self.max.as_secs_f64()));
        if self.jitter == 0.0 {
            return base;
        }
        let unit = self.random_unit();
        let scale = 1.0 + self.jitter * (2.0 * unit - 1.0);
        Duration::from_secs_f64(base.as_secs_f64() * scale)
    }

    pub fn max_delay(&self) -> Duration {
        self.max
    }

    /// xorshift64*: cheap, no dependency, more than good enough for jitter.
    fn random_unit(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64
    }
}

impl Default for Backoff {
    /// 1 s doubling to a 60 s cap with 20 % jitter: six attempts to reach the cap.
    fn default() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(60), 2.0).with_jitter(0.2)
    }
}

/// Non-zero seed from the standard library's randomly keyed hasher.
fn seed() -> u64 {
    let s = RandomState::new().build_hasher().finish();
    if s == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_until_cap_without_jitter() {
        let mut b = Backoff::new(Duration::from_secs(1), Duration::from_secs(10), 2.0);
        let delays: Vec<u64> = (0..6).map(|_| b.next_delay().as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 10, 10]);
    }

    #[test]
    fn reset_restarts_sequence() {
        let mut b = Backoff::new(Duration::from_secs(1), Duration::from_secs(10), 2.0);
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn jitter_stays_within_bounds_and_never_exhausts() {
        let mut b =
            Backoff::new(Duration::from_secs(1), Duration::from_secs(60), 2.0).with_jitter(0.2);
        for _ in 0..10_000 {
            let d = b.next_delay().as_secs_f64();
            assert!((0.8..=72.0).contains(&d), "delay {d} out of bounds");
        }
        let d = b.next_delay().as_secs_f64();
        assert!(
            (48.0..=72.0).contains(&d),
            "capped delay {d} should be 60 s ± 20 %"
        );
    }

    #[test]
    fn degenerate_parameters_are_clamped() {
        let mut b = Backoff::new(Duration::from_secs(5), Duration::from_secs(1), 0.5);
        assert_eq!(b.max_delay(), Duration::from_secs(5));
        assert_eq!(b.next_delay(), Duration::from_secs(5));
        assert_eq!(b.next_delay(), Duration::from_secs(5));
    }
}
