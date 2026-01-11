use serde::{Deserialize, Serialize};

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};
use crate::register_collector;

/// System entropy pool statistics.
/// Entropy is random data used by the kernel for cryptographic operations,
/// random number generation, and other security-critical functions.
/// The available entropy is measured in bits.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyStats {
    /// Number of bits of entropy currently available in the kernel's entropy pool.
    /// Higher values indicate more random data is available for cryptographic operations.
    /// The pool is refilled by kernel events (disk I/O, network events, interrupts, etc.)
    pub available_entropy: u32,
}

/// Collector for system entropy statistics from /proc/sys/kernel/random/entropy_avail.
/// This collector monitors the available entropy pool size, which is important for
/// systems that need high-quality random numbers (cryptography, key generation, etc.)
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntropyCollector;

/// Constructor methods for EntropyCollector
impl EntropyCollector {
    /// Creates a new EntropyCollector instance
    pub fn new() -> Self {
        EntropyCollector
    }
}

/// Default trait implementation for EntropyCollector
impl Default for EntropyCollector {
    fn default() -> Self {
        EntropyCollector::new()
    }
}

/// Data producer implementation for Entropy collector.
/// Reads from /proc/sys/kernel/random/entropy_avail to get the current entropy pool size.
#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
))]
#[async_trait::async_trait]
impl DataProducer for EntropyCollector {
    type Output = EntropyStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        // Read the entropy_avail file which contains the number of bits available in the entropy pool
        let content = tokio::fs::read_to_string("/proc/sys/kernel/random/entropy_avail")
            .await
            .map_err(|source| CollectorError::FileRead {
                path: "/proc/sys/kernel/random/entropy_avail".to_string(),
                source,
            })?;

        // Parse the entropy value (single integer value)
        let available_entropy =
            content
                .trim()
                .parse::<u32>()
                .map_err(|_| CollectorError::ParseError {
                    metric: "entropy_avail".to_string(),
                    location: "/proc/sys/kernel/random/entropy_avail".to_string(),
                    reason: "invalid integer format".to_string(),
                })?;

        Ok(EntropyStats { available_entropy })
    }
}

#[cfg(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
))]
register_collector!(EntropyCollector, "entropy");

/// Fallback implementation for unsupported platforms or disabled features
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
)))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyStats {
    pub available_entropy: u32,
}

/// Fallback collector for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
)))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntropyCollector;

/// Fallback data producer that returns an error for unsupported platforms
#[cfg(not(any(
    all(feature = "large", target_os = "linux"),
    all(feature = "collector-entropy", target_os = "linux")
)))]
#[async_trait::async_trait]
impl DataProducer for EntropyCollector {
    type Output = EntropyStats;

    async fn produce(&self) -> CollectorResult<Self::Output> {
        Err(
            crate::core::collectors::error::CollectorError::UnsupportedCollector(
                "Entropy collector not enabled or not supported on this platform".to_string(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_stats_creation() {
        let stats = EntropyStats {
            available_entropy: 3500,
        };

        assert_eq!(stats.available_entropy, 3500);
    }

    #[test]
    fn test_entropy_stats_zero_entropy() {
        // Test with zero entropy (critical situation - no randomness available)
        let stats = EntropyStats {
            available_entropy: 0,
        };

        assert_eq!(stats.available_entropy, 0);
    }

    #[test]
    fn test_entropy_stats_max_pool() {
        // Typical maximum entropy pool size on modern Linux systems
        let stats = EntropyStats {
            available_entropy: 4096,
        };

        assert_eq!(stats.available_entropy, 4096);
    }

    #[test]
    fn test_entropy_stats_typical_value() {
        // Typical value during normal system operation
        let stats = EntropyStats {
            available_entropy: 2048,
        };

        assert!(stats.available_entropy > 1000);
        assert!(stats.available_entropy < 4096);
    }

    #[test]
    fn test_entropy_stats_low_entropy() {
        // Low entropy situation - not critical but suboptimal for crypto operations
        let stats = EntropyStats {
            available_entropy: 256,
        };

        assert!(stats.available_entropy < 1024);
    }

    #[test]
    fn test_entropy_collector_creation() {
        let _ = EntropyCollector::new();
        let _ = EntropyCollector::default();
        // Both instances should be created successfully
    }

    #[test]
    fn test_entropy_stats_serialize() {
        let stats = EntropyStats {
            available_entropy: 3000,
        };

        let json = serde_json::to_string(&stats).expect("serialization failed");
        assert!(json.contains("3000"));
        assert!(json.contains("available_entropy"));
    }

    #[test]
    fn test_entropy_stats_deserialize() {
        let json = r#"{
            "available_entropy": 2500
        }"#;

        let stats: EntropyStats = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(stats.available_entropy, 2500);
    }

    #[test]
    fn test_entropy_stats_clone() {
        let stats1 = EntropyStats {
            available_entropy: 3500,
        };

        let stats2 = stats1.clone();
        assert_eq!(stats1.available_entropy, stats2.available_entropy);
    }

    #[test]
    fn test_entropy_stats_debug_format() {
        let stats = EntropyStats {
            available_entropy: 3500,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("EntropyStats"));
        assert!(debug_str.contains("3500"));
    }

    #[test]
    fn test_entropy_stats_critical_threshold() {
        // Test entropy level that triggers warnings (typically < 256)
        let low_entropy = EntropyStats {
            available_entropy: 100,
        };

        let normal_entropy = EntropyStats {
            available_entropy: 2048,
        };

        assert!(low_entropy.available_entropy < 256);
        assert!(normal_entropy.available_entropy > 256);
    }

    #[test]
    fn test_entropy_stats_healthy_pool() {
        // Healthy entropy pool is typically > 1024 bits
        let healthy = EntropyStats {
            available_entropy: 3500,
        };

        assert!(healthy.available_entropy > 1024);
    }

    #[test]
    fn test_entropy_stats_exhaustion_scenario() {
        // Scenario when entropy pool is almost exhausted
        let exhausted = EntropyStats {
            available_entropy: 50,
        };

        assert!(exhausted.available_entropy < 256);
        // This would indicate the system is under stress or doing heavy crypto
    }

    #[test]
    fn test_entropy_stats_maximum_u32() {
        // Test with a very large entropy value (though unlikely in practice)
        let stats = EntropyStats {
            available_entropy: u32::MAX,
        };

        assert_eq!(stats.available_entropy, u32::MAX);
    }

    #[test]
    fn test_entropy_collector_default_impl() {
        let collector1 = EntropyCollector::new();
        let collector2 = EntropyCollector::default();

        // Both should behave identically
        let json1 = serde_json::to_string(&collector1).expect("serialization failed");
        let json2 = serde_json::to_string(&collector2).expect("serialization failed");
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_entropy_stats_u32_bounds() {
        // Verify that u32 type properly bounds entropy values
        let min_entropy = EntropyStats {
            available_entropy: u32::MIN,
        };
        let _ = EntropyStats {
            available_entropy: u32::MAX,
        };

        assert_eq!(min_entropy.available_entropy, 0);
        // max_entropy can hold very large values (though unlikely)
    }

    #[test]
    fn test_entropy_stats_serialization_roundtrip() {
        let original = EntropyStats {
            available_entropy: 2748,
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: EntropyStats =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(original.available_entropy, deserialized.available_entropy);
    }

    #[test]
    fn test_entropy_stats_comparison() {
        let low = EntropyStats {
            available_entropy: 512,
        };
        let high = EntropyStats {
            available_entropy: 3000,
        };

        assert!(low.available_entropy < high.available_entropy);
    }

    #[test]
    fn test_entropy_pool_refill_monitoring() {
        // Test helper to understand entropy pool state changes
        let depleted = EntropyStats {
            available_entropy: 256,
        };
        let refilled = EntropyStats {
            available_entropy: 3500,
        };

        // The increase from depleted to refilled state
        let increase = refilled.available_entropy - depleted.available_entropy;
        assert_eq!(increase, 3244);
    }

    #[test]
    fn test_entropy_crypto_operational_threshold() {
        // Many crypto operations require at least 256-512 bits of entropy
        let minimum_for_crypto = EntropyStats {
            available_entropy: 256,
        };
        let adequate_for_crypto = EntropyStats {
            available_entropy: 512,
        };
        let optimal_for_crypto = EntropyStats {
            available_entropy: 2048,
        };

        assert!(minimum_for_crypto.available_entropy >= 256);
        assert!(adequate_for_crypto.available_entropy >= 256);
        assert!(optimal_for_crypto.available_entropy >= 256);
    }

    #[test]
    fn test_entropy_stats_clone_independence() {
        let mut stats1 = EntropyStats {
            available_entropy: 3000,
        };
        let stats2 = stats1.clone();

        // Modify original, clone should not be affected
        stats1.available_entropy = 2000;

        assert_eq!(stats1.available_entropy, 2000);
        assert_eq!(stats2.available_entropy, 3000);
    }
}
