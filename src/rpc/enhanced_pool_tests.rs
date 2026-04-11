#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::RpcEndpoint;
    use std::time::Duration;
    use tokio_test;

    fn create_test_endpoints() -> Vec<RpcEndpoint> {
        vec![
            RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 1,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            },
            RpcEndpoint {
                url: "https://solana-api.projectserum.com".to_string(),
                priority: 2,
                rate_limit_rps: 200,
                timeout_ms: 3000,
                healthy: true,
            },
            RpcEndpoint {
                url: "https://rpc.ankr.com/solana".to_string(),
                priority: 3,
                rate_limit_rps: 150,
                timeout_ms: 4000,
                healthy: true,
            },
        ]
    }

    fn create_test_config() -> PoolConfig {
        PoolConfig {
            max_connections_per_endpoint: 10,
            health_check_interval: Duration::from_secs(1),
            circuit_breaker_threshold: 3,
            circuit_breaker_timeout: Duration::from_secs(5),
            load_balance_strategy: LoadBalanceStrategy::WeightedRoundRobin,
            enable_connection_multiplexing: true,
            enable_compression: true,
        }
    }

    #[tokio::test]
    async fn test_enhanced_pool_creation() {
        let endpoints = create_test_endpoints();
        let config = create_test_config();
        
        let pool = EnhancedConnectionPool::new(endpoints, config);
        
        // Verify endpoints are properly initialized
        let endpoints_guard = pool.endpoints.read().await;
        assert_eq!(endpoints_guard.len(), 3);
        
        // Verify connection pools are created
        assert_eq!(pool.connection_pools.len(), 3);
        
        // Verify circuit breakers are created
        assert_eq!(pool.circuit_breakers.len(), 3);
    }

    #[tokio::test]
    async fn test_endpoint_selection_round_robin() {
        let endpoints = create_test_endpoints();
        let mut config = create_test_config();
        config.load_balance_strategy = LoadBalanceStrategy::RoundRobin;
        
        let pool = EnhancedConnectionPool::new(endpoints, config);
        
        // Test multiple selections to ensure round-robin behavior
        let mut selected_urls = std::collections::HashSet::new();
        
        for _ in 0..10 {
            let url = pool.select_endpoint().await.unwrap();
            selected_urls.insert(url);
        }
        
        // Should have selected from multiple endpoints
        assert!(selected_urls.len() > 1);
    }

    #[tokio::test]
    async fn test_endpoint_selection_weighted() {
        let endpoints = create_test_endpoints();
        let config = create_test_config();
        
        let pool = EnhancedConnectionPool::new(endpoints, config);
        
        // Test weighted selection
        let mut url_counts = std::collections::HashMap::new();
        
        for _ in 0..100 {
            let url = pool.select_endpoint().await.unwrap();
            *url_counts.entry(url).or_insert(0) += 1;
        }
        
        // All endpoints should be selected (weighted distribution)
        assert_eq!(url_counts.len(), 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker_functionality() {
        let circuit_breaker = CircuitBreaker::new(
            "test_endpoint".to_string(),
            2,
            Duration::from_secs(1),
        );
        
        // Initially closed, should allow requests
        assert!(circuit_breaker.allow_request());
        
        // Record failures to trigger circuit breaker
        circuit_breaker.record_failure();
        circuit_breaker.record_failure();
        
        // Should now be open and block requests
        assert!(!circuit_breaker.allow_request());
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Should be half-open and allow requests
        assert!(circuit_breaker.allow_request());
        
        // Record success to close circuit breaker
        circuit_breaker.record_success();
        
        // Should be closed again
        assert!(circuit_breaker.allow_request());
    }

    #[tokio::test]
    async fn test_health_checker_initialization() {
        let health_checker = HealthChecker::new(Duration::from_secs(1));
        
        // Health checker should be created without panicking
        assert_eq!(health_checker.check_interval, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_load_balancer_strategies() {
        let endpoints = create_test_endpoints();
        let weighted_endpoints: Vec<WeightedEndpoint> = endpoints
            .into_iter()
            .enumerate()
            .map(|(i, endpoint)| WeightedEndpoint {
                endpoint: endpoint.clone(),
                weight: 1.0 / (i as f64 + 1.0),
                priority: endpoint.priority,
                region: EnhancedConnectionPool::extract_region(&endpoint.url),
                response_time_ms: 100.0,
                success_rate: 1.0,
                last_health_check_ms: None,
                consecutive_failures: 0,
            })
            .collect();

        // Test RoundRobin strategy
        let lb_rr = LoadBalancer::new(LoadBalanceStrategy::RoundRobin);
        let url_rr = lb_rr.select_endpoint(&weighted_endpoints).await.unwrap();
        assert!(!url_rr.is_empty());

        // Test WeightedRoundRobin strategy
        let lb_wr = LoadBalancer::new(LoadBalanceStrategy::WeightedRoundRobin);
        let url_wr = lb_wr.select_endpoint(&weighted_endpoints).await.unwrap();
        assert!(!url_wr.is_empty());

        // Test LeastConnections strategy
        let lb_lc = LoadBalancer::new(LoadBalanceStrategy::LeastConnections);
        let url_lc = lb_lc.select_endpoint(&weighted_endpoints).await.unwrap();
        assert!(!url_lc.is_empty());

        // Test ResponseTime strategy
        let lb_rt = LoadBalancer::new(LoadBalanceStrategy::ResponseTime);
        let url_rt = lb_rt.select_endpoint(&weighted_endpoints).await.unwrap();
        assert!(!url_rt.is_empty());
    }

    #[tokio::test]
    async fn test_endpoint_metrics_update() {
        let endpoints = create_test_endpoints();
        let config = create_test_config();
        let pool = EnhancedConnectionPool::new(endpoints, config);
        
        let endpoint_url = "https://api.mainnet-beta.solana.com";
        
        // Update metrics with success
        pool.update_endpoint_metrics(endpoint_url, true, 150.0).await;
        
        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.successful_requests, 1);
        assert_eq!(metrics.failed_requests, 0);
        
        // Update metrics with failure
        pool.update_endpoint_metrics(endpoint_url, false, 500.0).await;
        
        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.successful_requests, 1);
        assert_eq!(metrics.failed_requests, 1);
    }

    #[tokio::test]
    async fn test_no_healthy_endpoints_error() {
        let mut endpoints = create_test_endpoints();
        // Mark all endpoints as unhealthy
        for endpoint in &mut endpoints {
            endpoint.healthy = false;
        }
        
        let config = create_test_config();
        let pool = EnhancedConnectionPool::new(endpoints, config);
        
        // Should return error when no healthy endpoints
        let result = pool.select_endpoint().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SolanaRecoverError::ConfigError(_)));
    }

    #[tokio::test]
    async fn test_region_extraction() {
        assert_eq!(
            EnhancedConnectionPool::extract_region("https://us-east.api.example.com"),
            "us-east"
        );
        assert_eq!(
            EnhancedConnectionPool::extract_region("https://us-west.api.example.com"),
            "us-west"
        );
        assert_eq!(
            EnhancedConnectionPool::extract_region("https://eu.api.example.com"),
            "eu-west"
        );
        assert_eq!(
            EnhancedConnectionPool::extract_region("https://global.api.example.com"),
            "global"
        );
    }

    #[tokio::test]
    async fn test_pool_metrics_collection() {
        let endpoints = create_test_endpoints();
        let config = create_test_config();
        let pool = EnhancedConnectionPool::new(endpoints, config);
        
        // Simulate some activity
        for i in 0..10 {
            let success = i % 3 != 0; // 2/3 success rate
            let response_time = 100.0 + (i as f64 * 10.0);
            pool.update_endpoint_metrics("https://api.mainnet-beta.solana.com", success, response_time).await;
        }
        
        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.total_requests, 10);
        assert_eq!(metrics.successful_requests, 7);
        assert_eq!(metrics.failed_requests, 3);
        assert!(metrics.avg_response_time_ms > 0.0);
    }

    #[tokio::test]
    async fn test_connection_pool_basic_functionality() {
        let endpoint = RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 100,
            timeout_ms: 5000,
            healthy: true,
        };
        
        let pool = ConnectionPool::new(endpoint, 5);
        
        // Note: This test may fail if the endpoint is not accessible
        // In a real test environment, you'd mock the RpcClient
        let client_result = pool.get_client().await;
        
        // The result might fail due to network issues, but the pool should not panic
        match client_result {
            Ok(_) => {
                // If successful, test returning the client
                // pool.return_client(client);
            }
            Err(_) => {
                // Network error is expected in test environment
            }
        }
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections_per_endpoint, 50);
        assert_eq!(config.health_check_interval, Duration::from_secs(30));
        assert_eq!(config.circuit_breaker_threshold, 5);
        assert_eq!(config.circuit_breaker_timeout, Duration::from_secs(60));
        assert!(matches!(config.load_balance_strategy, LoadBalanceStrategy::WeightedRoundRobin));
        assert!(config.enable_connection_multiplexing);
        assert!(config.enable_compression);
    }

    #[test]
    fn test_weighted_endpoint_creation() {
        let endpoint = RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 100,
            timeout_ms: 5000,
            healthy: true,
        };
        
        let weighted = WeightedEndpoint {
            endpoint: endpoint.clone(),
            weight: 0.5,
            priority: endpoint.priority,
            region: "us-east".to_string(),
            response_time_ms: 150.0,
            success_rate: 0.95,
            last_health_check_ms: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
            ),
            consecutive_failures: 0,
        };
        
        assert_eq!(weighted.endpoint.url, endpoint.url);
        assert_eq!(weighted.weight, 0.5);
        assert_eq!(weighted.region, "us-east");
        assert_eq!(weighted.success_rate, 0.95);
        assert_eq!(weighted.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_metrics_aware_client_delegation() {
        // This test would require a mock RpcClientWrapper
        // For now, we just test the structure
        
        let endpoints = create_test_endpoints();
        let config = create_test_config();
        let pool = Arc::new(EnhancedConnectionPool::new(endpoints, config));
        
        // Note: This is a structural test - in practice, you'd mock the inner client
        // to test the metrics collection functionality
        
        let _pool_clone = pool.clone();
        assert_eq!(pool.connection_pools.len(), 3);
    }
}
