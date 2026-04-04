#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RpcEndpoint, Result, SolanaRecoverError};

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let endpoints = vec![
            RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 0,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            }
        ];

        let pool = ConnectionPool::new(endpoints.clone(), 5);
        
        // Test that pool was created successfully
        let client = pool.get_client().await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_connection_pool_multiple_endpoints() {
        let endpoints = vec![
            RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 0,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            },
            RpcEndpoint {
                url: "https://solana-api.projectserum.com".to_string(),
                priority: 1,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            }
        ];

        let pool = ConnectionPool::new(endpoints, 3);
        
        // Test that pool handles multiple endpoints
        let client = pool.get_client().await;
        assert!(client.is_ok());
    }

    #[test]
    fn test_rpc_endpoint_creation() {
        let endpoint = RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 100,
            timeout_ms: 5000,
            healthy: true,
        };

        assert_eq!(endpoint.priority, 1);
        assert_eq!(endpoint.rate_limit_rps, 100);
        assert_eq!(endpoint.timeout_ms, 5000);
        assert!(endpoint.healthy);
    }

    #[test]
    fn test_rpc_endpoint_serialization() {
        let endpoint = RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 0,
            rate_limit_rps: 100,
            timeout_ms: 5000,
            healthy: true,
        };

        // Test JSON serialization
        let json = serde_json::to_string(&endpoint);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: RpcEndpoint = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
    }
}
