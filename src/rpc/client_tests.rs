#[cfg(test)]
mod tests {
    use crate::core::{SolanaRecoverError};
    use solana_sdk::pubkey::Pubkey;
    use std::time::Duration;

    #[tokio::test]
    async fn test_token_bucket_rate_limiter() {
        let rate_limiter = TokenBucketRateLimiter::new(10); // 10 requests per second
        
        // Should be able to acquire 10 tokens immediately
        for _ in 0..10 {
            let result = rate_limiter.acquire().await;
            assert!(result.is_ok());
        }
        
        // The 11th request should fail
        let result = rate_limiter.acquire().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SolanaRecoverError::RateLimitExceeded));
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let rate_limiter = TokenBucketRateLimiter::new(2); // 2 requests per second
        
        // Use all tokens
        rate_limiter.acquire().await.unwrap();
        rate_limiter.acquire().await.unwrap();
        
        // Should fail now
        let result = rate_limiter.acquire().await;
        assert!(result.is_err());
        
        // Wait for refill (slightly longer than the interval)
        tokio::time::sleep(Duration::from_millis(600)).await;
        
        // Should be able to acquire again
        let result = rate_limiter.acquire().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_rate_limiter_creation() {
        let rate_limiter = TokenBucketRateLimiter::new(100);
        assert_eq!(rate_limiter.max_tokens, 100);
    }

    #[tokio::test]
    async fn test_new_with_url() {
        // This test uses a mock/non-existent URL, so it should work for client creation
        let result = RpcClientWrapper::new_with_url("https://api.mainnet-beta.solana.com", 30000);
        assert!(result.is_ok());
        
        let client = result.unwrap();
        assert_eq!(client.request_timeout, Duration::from_millis(30000));
    }

    #[tokio::test]
    async fn test_from_url() {
        let result = RpcClientWrapper::from_url("https://api.devnet.solana.com", 15000);
        assert!(result.is_ok());
        
        let client = result.unwrap();
        assert_eq!(client.request_timeout, Duration::from_millis(15000));
    }

    #[tokio::test]
    async fn test_get_health_timeout() {
        let client = RpcClientWrapper::new_with_url("http://invalid-url-that-will-timeout.com", 100).unwrap();
        
        // This should timeout quickly
        let result = client.get_health().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SolanaRecoverError::NetworkError(_)));
    }

    #[test]
    fn test_debug_format() {
        let client = RpcClientWrapper::new_with_url("https://api.mainnet-beta.solana.com", 30000).unwrap();
        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("RpcClientWrapper"));
        assert!(debug_str.contains("request_timeout"));
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        // Create a client with cache
        let client = RpcClientWrapper::new_with_url("https://api.mainnet-beta.solana.com", 30000).unwrap();
        
        // Test that cache exists and can be used
        // Note: We can't easily test the actual caching without a real RPC connection
        // But we can verify the cache is initialized
        
        // The cache should be empty initially
        let cache_size = client.rent_cache.entry_count();
        assert_eq!(cache_size, 0);
        
        // Cache should have the correct capacity
        let cache_capacity = client.rent_cache.max_capacity();
        assert_eq!(cache_capacity, 1000);
    }
}
