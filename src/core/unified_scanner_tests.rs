//! Tests for the unified scanner architecture

use crate::core::unified_scanner::*;
use crate::core::scanner_builder::*;
use crate::core::error_recovery::*;
use crate::core::config_management::*;
use crate::utils::cache::{MemoryCache, SimpleMetrics};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    // Mock connection pool for testing
    struct MockConnectionPool;
    
    impl crate::rpc::ConnectionPoolTrait for MockConnectionPool {
        async fn get_client(&self) -> crate::core::Result<Arc<crate::rpc::RpcClientWrapper>> {
            Err(crate::core::SolanaRecoverError::InternalError("Mock connection pool".to_string()))
        }
    }

    #[test]
    fn test_performance_mode_default() {
        let config = UnifiedScannerConfig::default();
        assert!(matches!(config.performance_mode, PerformanceMode::Balanced));
    }

    #[test]
    fn test_performance_mode_serialization() {
        let mode = PerformanceMode::UltraFast;
        let serialized = serde_json::to_string(&mode).unwrap();
        let deserialized: PerformanceMode = serde_json::from_str(&serialized).unwrap();
        assert!(matches!(deserialized, PerformanceMode::UltraFast));
    }

    #[test]
    fn test_unified_scanner_config_validation() {
        let mut config = UnifiedScannerConfig::default();
        
        // Valid config should pass
        assert!(config.validate().is_ok());
        
        // Invalid config should fail
        config.max_concurrent_scans = 0;
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_unified_scanner_creation() {
        let connection_pool = Arc::new(MockConnectionPool);
        let config = UnifiedScannerConfig::default();
        
        let scanner = UnifiedWalletScanner::new(connection_pool, config);
        
        // Should have default strategies
        assert_eq!(scanner.available_strategies().len(), 3);
        assert!(scanner.available_strategies().contains(&"UltraFast"));
        assert!(scanner.available_strategies().contains(&"Balanced"));
        assert!(scanner.available_strategies().contains(&"ResourceEfficient"));
        
        // Should have active strategy
        assert!(scanner.active_strategy_name().is_some());
    }

    #[tokio::test]
    async fn test_performance_mode_switching() {
        let connection_pool = Arc::new(MockConnectionPool);
        let mut scanner = UnifiedWalletScanner::new(connection_pool, UnifiedScannerConfig::default());
        
        // Default should be Balanced
        assert_eq!(scanner.active_strategy_name(), Some("Balanced"));
        
        // Switch to UltraFast
        assert!(scanner.set_performance_mode(PerformanceMode::UltraFast).is_ok());
        assert_eq!(scanner.active_strategy_name(), Some("UltraFast"));
        
        // Switch to ResourceEfficient
        assert!(scanner.set_performance_mode(PerformanceMode::ResourceEfficient).is_ok());
        assert_eq!(scanner.active_strategy_name(), Some("ResourceEfficient"));
    }

    #[test]
    fn test_strategy_priorities() {
        let ultra_fast = UltraFastStrategy::new();
        let balanced = BalancedStrategy;
        let resource_efficient = ResourceEfficientStrategy;
        
        assert!(ultra_fast.priority() > balanced.priority());
        assert!(balanced.priority() > resource_efficient.priority());
    }

    #[test]
    fn test_strategy_mode_support() {
        let ultra_fast = UltraFastStrategy::new();
        let balanced = BalancedStrategy;
        let resource_efficient = ResourceEfficientStrategy;
        
        assert!(ultra_fast.supports_mode(&PerformanceMode::UltraFast));
        assert!(ultra_fast.supports_mode(&PerformanceMode::Throughput));
        assert!(!ultra_fast.supports_mode(&PerformanceMode::Balanced));
        
        assert!(balanced.supports_mode(&PerformanceMode::Balanced));
        assert!(!balanced.supports_mode(&PerformanceMode::UltraFast));
        
        assert!(resource_efficient.supports_mode(&PerformanceMode::ResourceEfficient));
        assert!(!resource_efficient.supports_mode(&PerformanceMode::Balanced));
    }

    #[test]
    fn test_scanner_builder() {
        let connection_pool = Arc::new(MockConnectionPool);
        
        let result = ScannerBuilder::new()
            .with_connection_pool(connection_pool)
            .with_performance_mode(PerformanceMode::UltraFast)
            .build();
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_scanner_builder_missing_connection_pool() {
        let result = ScannerBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_scanner_factory() {
        let connection_pool = Arc::new(MockConnectionPool);
        
        let ultra_fast_result = ScannerFactory::create_ultra_fast(connection_pool.clone());
        assert!(ultra_fast_result.is_ok());
        
        let balanced_result = ScannerFactory::create_balanced(connection_pool.clone());
        assert!(balanced_result.is_ok());
        
        let resource_efficient_result = ScannerFactory::create_resource_efficient(connection_pool);
        assert!(resource_efficient_result.is_ok());
    }

    #[test]
    fn test_scanner_container() {
        let connection_pool = Arc::new(MockConnectionPool);
        let cache = Arc::new(MemoryCache::new());
        let metrics = Arc::new(SimpleMetrics::new());
        
        let container = ScannerContainer::new(connection_pool)
            .with_cache(cache)
            .with_metrics(metrics);
        
        let builder = container.builder();
        let result = builder.build();
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_config() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, std::time::Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_circuit_breaker_state_transitions() {
        let circuit_breaker = CircuitBreaker::new();
        
        // Initial state should be closed
        assert_eq!(circuit_breaker.get_state().await, CircuitState::Closed);
        
        // Force open
        circuit_breaker.force_open().await;
        assert_eq!(circuit_breaker.get_state().await, CircuitState::Open);
        
        // Force close
        circuit_breaker.force_close().await;
        assert_eq!(circuit_breaker.get_state().await, CircuitState::Closed);
    }

    #[test]
    fn test_retry_policy_delay_calculation() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
            retryable_errors: vec!["timeout".to_string()],
        };
        
        let delay1 = policy.calculate_delay(1);
        let delay2 = policy.calculate_delay(2);
        let delay3 = policy.calculate_delay(3);
        
        assert_eq!(delay1, std::time::Duration::from_millis(100));
        assert_eq!(delay2, std::time::Duration::from_millis(200));
        assert_eq!(delay3, std::time::Duration::from_millis(400));
    }

    #[test]
    fn test_retry_policy_retryable_errors() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
            retryable_errors: vec!["timeout".to_string(), "network".to_string()],
        };
        
        let timeout_error = crate::core::SolanaRecoverError::TimeoutError("Request timed out".to_string());
        let network_error = crate::core::SolanaRecoverError::NetworkError("Network failed".to_string());
        let internal_error = crate::core::SolanaRecoverError::InternalError("Internal error".to_string());
        
        assert!(policy.is_retryable(&timeout_error));
        assert!(policy.is_retryable(&network_error));
        assert!(!policy.is_retryable(&internal_error));
    }

    #[tokio::test]
    async fn test_retry_mechanism_success() {
        let retry = RetryMechanism::with_default_policy();
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        let result = retry.execute(|| {
            let count = call_count.clone();
            async move {
                let current = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if current < 1 {
                    Err(crate::core::SolanaRecoverError::TimeoutError("temporary".to_string()))
                } else {
                    Ok("success")
                }
            }
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_mechanism_exhausted() {
        let retry = RetryMechanism::with_default_policy();
        let mut call_count = 0;
        
        let result = retry.execute(|| async {
            call_count += 1;
            Err(crate::core::SolanaRecoverError::TimeoutError("always fails".to_string()))
        }).await;
        
        assert!(result.is_err());
        assert_eq!(call_count, 3); // max_attempts
    }

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.environment, Environment::Development);
        assert!(!config.rpc.endpoints.is_empty());
        assert_eq!(config.scanner.performance_mode, "balanced");
    }

    #[test]
    fn test_app_config_validation() {
        let mut config = AppConfig::default();
        
        // Valid config should pass
        assert!(config.validate().is_ok());
        
        // Invalid: no RPC endpoints
        config.rpc.endpoints.clear();
        assert!(config.validate().is_err());
        
        // Valid: add endpoint back
        config.rpc.endpoints.push(RpcEndpointConfig {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 100,
            timeout_ms: 30000,
            enabled: true,
        });
        assert!(config.validate().is_ok());
        
        // Invalid: invalid performance mode
        config.scanner.performance_mode = "invalid_mode".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_environment_configs() {
        let dev_config = AppConfig::for_environment(Environment::Development);
        let prod_config = AppConfig::for_environment(Environment::Production);
        
        assert_eq!(dev_config.environment, Environment::Development);
        assert_eq!(prod_config.environment, Environment::Production);
        
        // Production should have higher limits
        assert!(prod_config.rpc.rate_limit_rps > dev_config.rpc.rate_limit_rps);
        assert!(prod_config.scanner.max_concurrent_scans > dev_config.scanner.max_concurrent_scans);
    }

    #[test]
    fn test_app_config_serialization() {
        let config = AppConfig::default();
        
        let serialized = toml::to_string_pretty(&config).unwrap();
        assert!(!serialized.is_empty());
        
        let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(config.environment, deserialized.environment);
        assert_eq!(config.app_name, deserialized.app_name);
    }

    #[test]
    fn test_config_manager_creation() {
        let config = AppConfig::default();
        let manager = ConfigManager::new(config);
        
        // Should be able to get config
        tokio::spawn(async move {
            let retrieved_config = manager.get_config().await;
            assert_eq!(retrieved_config.environment, Environment::Development);
        });
    }

    #[test]
    fn test_batch_scan_request_creation() {
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses: vec![
                "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
                "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            ],
            user_id: Some("user123".to_string()),
            fee_percentage: Some(0.15),
            created_at: chrono::Utc::now(),
        };
        
        assert_eq!(request.wallet_addresses.len(), 2);
        assert_eq!(request.user_id, Some("user123".to_string()));
        assert_eq!(request.fee_percentage, Some(0.15));
    }

    #[test]
    fn test_batch_scan_result_creation() {
        let result = BatchScanResult {
            request_id: Uuid::new_v4(),
            total_wallets: 2,
            successful_scans: 2,
            failed_scans: 0,
            total_recoverable_sol: 0.5,
            estimated_fee_sol: 0.1,
            results: vec![],
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            duration_ms: Some(5000),
            scan_time_ms: 5000,
            ..Default::default()
        };
        
        assert_eq!(result.total_wallets, 2);
        assert_eq!(result.successful_scans, 2);
        assert_eq!(result.failed_scans, 0);
        assert_eq!(result.total_recoverable_sol, 0.5);
    }

    #[test]
    fn test_scan_result_creation() {
        let result = ScanResult {
            id: Uuid::new_v4(),
            wallet_address: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            status: ScanStatus::Completed,
            result: None,
            empty_accounts_found: 5,
            recoverable_sol: 0.1,
            scan_time_ms: 1000,
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            error_message: None,
        };
        
        assert_eq!(result.wallet_address, "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");
        assert_eq!(result.status, ScanStatus::Completed);
        assert_eq!(result.empty_accounts_found, 5);
        assert_eq!(result.recoverable_sol, 0.1);
        assert_eq!(result.scan_time_ms, 1000);
    }

    #[test]
    fn test_rpc_endpoint_creation() {
        let endpoint = RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 100,
            timeout_ms: 30000,
            healthy: true,
        };
        
        assert_eq!(endpoint.url, "https://api.mainnet-beta.solana.com");
        assert_eq!(endpoint.priority, 1);
        assert_eq!(endpoint.rate_limit_rps, 100);
        assert_eq!(endpoint.timeout_ms, 30000);
        assert!(endpoint.healthy);
    }

    // Integration test for the complete architecture
    #[tokio::test]
    async fn test_complete_architecture_integration() {
        // Create components
        let connection_pool = Arc::new(MockConnectionPool);
        let cache = Arc::new(MemoryCache::new());
        let metrics = Arc::new(SimpleMetrics::new());
        
        // Create scanner using builder
        let scanner = ScannerBuilder::new()
            .with_connection_pool(connection_pool)
            .with_cache(cache)
            .with_metrics(metrics)
            .with_performance_mode(PerformanceMode::Balanced)
            .build()
            .unwrap();
        
        // Verify scanner properties
        assert!(scanner.active_strategy_name().is_some());
        assert_eq!(scanner.available_strategies().len(), 3);
        
        // Test performance mode switching
        let mut scanner_mut = scanner;
        assert!(scanner_mut.set_performance_mode(PerformanceMode::UltraFast).is_ok());
        assert_eq!(scanner_mut.active_strategy_name(), Some("UltraFast"));
    }

    // Performance test for strategy selection
    #[test]
    fn test_strategy_selection_performance() {
        let connection_pool = Arc::new(MockConnectionPool);
        let config = UnifiedScannerConfig::default();
        
        let start = std::time::Instant::now();
        
        // Create scanner multiple times to test strategy selection performance
        for _ in 0..100 {
            let _scanner = UnifiedWalletScanner::new(connection_pool.clone(), config.clone());
        }
        
        let duration = start.elapsed();
        assert!(duration.as_millis() < 100, "Strategy selection should be fast");
    }

    // Test error handling in configuration
    #[test]
    fn test_config_error_handling() {
        // Test invalid performance mode
        let mut config = UnifiedScannerConfig::default();
        config.performance_mode = PerformanceMode::UltraFast;
        config.max_concurrent_scans = 0; // Invalid
        
        let connection_pool = Arc::new(MockConnectionPool);
        let scanner = UnifiedWalletScanner::new(connection_pool, config);
        
        // Scanner should still be created but validation would fail
        assert!(scanner.available_strategies().len() > 0);
    }
}
