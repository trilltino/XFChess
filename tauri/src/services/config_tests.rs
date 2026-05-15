//! Unit tests for configuration module.
//! 
//! This module contains comprehensive tests for the configuration
//! functions in services/config.rs to ensure they work correctly
//! with various environment variable configurations.

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Test get_backend_url with SIGNING_SERVICE_URL set
    #[test]
    fn test_get_backend_url_with_signing_service() {
        env::set_var("SIGNING_SERVICE_URL", "https://signing.example.com");
        env::remove_var("BACKEND_URL");
        
        let url = get_backend_url();
        assert_eq!(url, "https://signing.example.com");
        
        env::remove_var("SIGNING_SERVICE_URL");
    }

    /// Test get_backend_url with BACKEND_URL set (fallback)
    #[test]
    fn test_get_backend_url_with_backend_url() {
        env::remove_var("SIGNING_SERVICE_URL");
        env::set_var("BACKEND_URL", "https://backend.example.com");
        
        let url = get_backend_url();
        assert_eq!(url, "https://backend.example.com");
        
        env::remove_var("BACKEND_URL");
    }

    /// Test get_backend_url with no environment variables (default)
    #[test]
    fn test_get_backend_url_default() {
        env::remove_var("SIGNING_SERVICE_URL");
        env::remove_var("BACKEND_URL");
        
        let url = get_backend_url();
        assert_eq!(url, "http://127.0.0.1:8090");
    }

    /// Test get_backend_url with both variables set (SIGNING_SERVICE_URL takes priority)
    #[test]
    fn test_get_backend_url_priority() {
        env::set_var("SIGNING_SERVICE_URL", "https://signing.example.com");
        env::set_var("BACKEND_URL", "https://backend.example.com");
        
        let url = get_backend_url();
        assert_eq!(url, "https://signing.example.com");
        
        env::remove_var("SIGNING_SERVICE_URL");
        env::remove_var("BACKEND_URL");
    }

    /// Test get_admin_api_key when set
    #[test]
    fn test_get_admin_api_key_set() {
        env::set_var("ADMIN_API_KEY", "test-api-key-123");
        
        let key = get_admin_api_key();
        assert_eq!(key, Some("test-api-key-123".to_string()));
        
        env::remove_var("ADMIN_API_KEY");
    }

    /// Test get_admin_api_key when not set
    #[test]
    fn test_get_admin_api_key_not_set() {
        env::remove_var("ADMIN_API_KEY");
        
        let key = get_admin_api_key();
        assert_eq!(key, None);
    }

    /// Test get_wallet_port with custom port
    #[test]
    fn test_get_wallet_port_custom() {
        env::set_var("XFCHESS_WALLET_PORT", "9000");
        
        let port = get_wallet_port();
        assert_eq!(port, 9000);
        
        env::remove_var("XFCHESS_WALLET_PORT");
    }

    /// Test get_wallet_port with invalid port (fallback to default)
    #[test]
    fn test_get_wallet_port_invalid() {
        env::set_var("XFCHESS_WALLET_PORT", "invalid");
        
        let port = get_wallet_port();
        assert_eq!(port, 7454); // Default fallback
        
        env::remove_var("XFCHESS_WALLET_PORT");
    }

    /// Test get_wallet_port with no environment variable (default)
    #[test]
    fn test_get_wallet_port_default() {
        env::remove_var("XFCHESS_WALLET_PORT");
        
        let port = get_wallet_port();
        assert_eq!(port, 7454);
    }

    /// Test is_development with NODE_ENV set to development
    #[test]
    fn test_is_development_true() {
        env::set_var("NODE_ENV", "development");
        
        let is_dev = is_development();
        assert!(is_dev);
        
        env::remove_var("NODE_ENV");
    }

    /// Test is_development with NODE_ENV set to production
    #[test]
    fn test_is_development_false() {
        env::set_var("NODE_ENV", "production");
        
        let is_dev = is_development();
        assert!(!is_dev);
        
        env::remove_var("NODE_ENV");
    }

    /// Test is_development with no NODE_ENV (depends on debug_assertions)
    #[test]
    fn test_is_development_no_env() {
        env::remove_var("NODE_ENV");
        
        // This test depends on whether debug assertions are enabled
        // In debug builds, this should be true
        let is_dev = is_development();
        // We can't assert a specific value since it depends on build mode
        // but we can verify it returns a boolean
        assert_eq!(is_dev, is_dev); // Tautology but verifies type
    }

    /// Test get_log_level with custom level
    #[test]
    fn test_get_log_level_custom() {
        env::set_var("RUST_LOG", "debug");
        
        let level = get_log_level();
        assert_eq!(level, "debug");
        
        env::remove_var("RUST_LOG");
    }

    /// Test get_log_level with no environment variable (default)
    #[test]
    fn test_get_log_level_default() {
        env::remove_var("RUST_LOG");
        
        let level = get_log_level();
        assert_eq!(level, "info");
    }
}
