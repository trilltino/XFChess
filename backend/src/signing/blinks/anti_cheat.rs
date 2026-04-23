//! Anti-cheat validation for Solana Blinks tournament registration.
//!
//! This module provides IP-based pattern detection, rate limiting,
//! and other security checks to prevent bot attacks and sybil attacks
//! on tournaments.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use once_cell::sync::OnceCell;
use tokio::sync::RwLock;

/// IP-based pattern detection for anti-cheat.
pub struct IpPatternDetector {
    /// Tracks registration attempts per IP
    registrations_per_ip: Arc<RwLock<HashMap<String, u32>>>,
    /// Tracks rapid registration attempts per IP (rate limiting)
    rate_limit_tracker: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    /// Maximum registrations allowed per IP
    max_registrations_per_ip: u32,
    /// Rate limit window duration
    rate_limit_window: Duration,
    /// Maximum registrations within rate limit window
    max_registrations_per_window: u32,
}

impl IpPatternDetector {
    /// Creates a new IP pattern detector.
    pub fn new() -> Self {
        Self {
            registrations_per_ip: Arc::new(RwLock::new(HashMap::new())),
            rate_limit_tracker: Arc::new(RwLock::new(HashMap::new())),
            max_registrations_per_ip: 3, // Max 3 registrations per IP
            rate_limit_window: Duration::from_secs(300), // 5 minutes
            max_registrations_per_window: 2, // Max 2 registrations per 5 minutes
        }
    }

    /// Checks if an IP address has suspicious registration patterns.
    ///
    /// Returns an error message if patterns are detected, None if clean.
    pub async fn check_ip_patterns(&self, ip_address: &str, _tournament_id: u64) -> Option<String> {
        // Check total registrations per IP
        {
            let registrations = self.registrations_per_ip.read().await;
            let count = registrations.get(ip_address).unwrap_or(&0);
            if *count >= self.max_registrations_per_ip {
                return Some(format!(
                    "Too many registrations from this IP address (max {} per IP)",
                    self.max_registrations_per_ip
                ));
            }
        }

        // Check rate limiting
        {
            let mut tracker = self.rate_limit_tracker.write().await;
            let now = Instant::now();
            let attempts = tracker.entry(ip_address.to_string()).or_insert_with(Vec::new);

            // Remove old attempts outside the window
            attempts.retain(|t| now.duration_since(*t) < self.rate_limit_window);

            // Check if too many recent attempts
            if attempts.len() >= self.max_registrations_per_window as usize {
                return Some(format!(
                    "Too many rapid registration attempts. Please wait {} seconds.",
                    self.rate_limit_window.as_secs()
                ));
            }

            // Record this attempt
            attempts.push(now);
        }

        // Record successful registration
        {
            let mut registrations = self.registrations_per_ip.write().await;
            *registrations.entry(ip_address.to_string()).or_insert(0) += 1;
        }

        None
    }

    /// Clears rate limit tracking for a specific IP (for testing).
    #[cfg(test)]
    pub async fn clear_ip(&self, ip_address: &str) {
        let mut tracker = self.rate_limit_tracker.write().await;
        tracker.remove(ip_address);

        let mut registrations = self.registrations_per_ip.write().await;
        registrations.remove(ip_address);
    }
}

impl Default for IpPatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Global IP pattern detector instance.
static IP_DETECTOR: OnceCell<IpPatternDetector> = OnceCell::new();

/// Gets the global IP pattern detector instance.
pub fn get_ip_detector() -> &'static IpPatternDetector {
    IP_DETECTOR.get_or_init(|| IpPatternDetector::new())
}

/// Checks IP patterns using the global detector.
///
/// This is a convenience function that uses the singleton instance.
pub async fn check_ip_patterns(ip_address: &str, _tournament_id: u64) -> Option<String> {
    get_ip_detector().check_ip_patterns(ip_address, _tournament_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ip_pattern_detection() {
        let detector = IpPatternDetector::new();

        // First registration should pass
        assert!(detector.check_ip_patterns("192.168.1.1", 1).await.is_none());

        // Second registration should pass
        assert!(detector.check_ip_patterns("192.168.1.1", 1).await.is_none());

        // Third registration should fail (rate limit: max 2 per 5 min window)
        assert!(detector.check_ip_patterns("192.168.1.1", 1).await.is_some());
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let detector = IpPatternDetector::new();

        // First registration should pass
        assert!(detector.check_ip_patterns("192.168.1.2", 1).await.is_none());

        // Second registration should pass
        assert!(detector.check_ip_patterns("192.168.1.2", 1).await.is_none());

        // Third registration should fail due to rate limiting
        assert!(detector.check_ip_patterns("192.168.1.2", 1).await.is_some());
    }

    #[tokio::test]
    async fn test_different_ips() {
        let detector = IpPatternDetector::new();

        // Different IPs should not interfere
        assert!(detector.check_ip_patterns("192.168.1.3", 1).await.is_none());
        assert!(detector.check_ip_patterns("192.168.1.4", 1).await.is_none());
        assert!(detector.check_ip_patterns("192.168.1.5", 1).await.is_none());
        assert!(detector.check_ip_patterns("192.168.1.6", 1).await.is_none());
    }
}
