use crate::core::{Result, SolanaRecoverError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Token bucket implementation for rate limiting
#[derive(Debug)]
struct TokenBucket {
    max_tokens: u32,
    tokens: tokio::sync::Mutex<u32>,
    refill_interval: Duration,
    last_refill: tokio::sync::Mutex<Instant>,
}

impl TokenBucket {
    fn new(max_tokens: u32) -> Self {
        Self {
            max_tokens,
            tokens: tokio::sync::Mutex::new(max_tokens),
            refill_interval: Duration::from_secs(1) / max_tokens as u32,
            last_refill: tokio::sync::Mutex::new(Instant::now()),
        }
    }

    async fn acquire(&self) -> Result<()> {
        self.refill_tokens().await;
        
        let mut tokens = self.tokens.lock().await;
        if *tokens > 0 {
            *tokens -= 1;
            Ok(())
        } else {
            Err(SolanaRecoverError::RateLimitExceeded("Rate limit exceeded".to_string()))
        }
    }

    async fn refill_tokens(&self) {
        let mut tokens = self.tokens.lock().await;
        let mut last_refill = self.last_refill.lock().await;
        
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(*last_refill);
        
        if elapsed >= self.refill_interval {
            let tokens_to_add = (elapsed.as_millis() / self.refill_interval.as_millis()) as u32;
            *tokens = (*tokens + tokens_to_add).min(self.max_tokens);
            *last_refill = now;
        }
    }
}

/// Distributed rate limiter that prevents bypass through concurrent connections
#[derive(Debug)]
pub struct DistributedRateLimiter {
    // Per-key rate limiting buckets
    buckets: Arc<DashMap<String, Arc<TokenBucket>>>,
    // Global rate limiting
    global_limit: Arc<AtomicU64>,
    window_start: Arc<AtomicU64>,
    max_requests_per_window: u64,
    window_duration: Duration,
    // Configuration
    default_bucket_size: u32,
    max_buckets: usize,
}

impl DistributedRateLimiter {
    /// Create a new distributed rate limiter
    /// 
    /// # Arguments
    /// * `max_requests_per_window` - Maximum requests per time window globally
    /// * `window_duration` - Duration of the time window
    /// * `default_bucket_size` - Default tokens per bucket
    /// * `max_buckets` - Maximum number of buckets to prevent memory exhaustion
    pub fn new(
        max_requests_per_window: u64,
        window_duration: Duration,
        default_bucket_size: u32,
        max_buckets: usize,
    ) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            global_limit: Arc::new(AtomicU64::new(0)),
            window_start: Arc::new(AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() / window_duration.as_secs()
            )),
            max_requests_per_window,
            window_duration,
            default_bucket_size,
            max_buckets,
        }
    }

    /// Create with sensible defaults
    pub fn with_defaults() -> Self {
        Self::new(
            1000, // 1000 requests per minute globally
            Duration::from_secs(60),
            100,  // 100 requests per second per key
            10000, // Max 10k buckets
        )
    }

    /// Acquire a permit for the given key
    pub async fn acquire(&self, key: &str) -> Result<()> {
        // Check global rate limit first
        self.check_global_limit().await?;
        
        // Check per-key rate limit
        self.check_key_limit(key).await?;
        
        // Increment global counter
        self.global_limit.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }

    /// Check global rate limit
    async fn check_global_limit(&self) -> Result<()> {
        let current_window = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() / self.window_duration.as_secs();
        let window_start = self.window_start.load(Ordering::Relaxed);
        
        // Reset if we're in a new window
        if current_window != window_start {
            self.window_start.store(current_window, Ordering::Relaxed);
            self.global_limit.store(0, Ordering::Relaxed);
        }
        
        let global_count = self.global_limit.load(Ordering::Relaxed);
        if global_count >= self.max_requests_per_window {
            return Err(SolanaRecoverError::RateLimitExceeded(
                format!("Global rate limit exceeded: {}/{}", global_count, self.max_requests_per_window)
            ));
        }
        
        Ok(())
    }

    /// Check per-key rate limit
    async fn check_key_limit(&self, key: &str) -> Result<()> {
        // Enforce maximum number of buckets to prevent memory exhaustion
        if self.buckets.len() >= self.max_buckets {
            // Try to clean up old buckets
            self.cleanup_old_buckets().await;
            
            if self.buckets.len() >= self.max_buckets {
                return Err(SolanaRecoverError::RateLimitExceeded(
                    "Too many concurrent rate limiters".to_string()
                ));
            }
        }
        
        let bucket = self.buckets.entry(key.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(self.default_bucket_size)));
        
        bucket.acquire().await
    }

    /// Clean up old buckets to prevent memory leaks
    async fn cleanup_old_buckets(&self) {
        let now = Instant::now();
        let mut to_remove = Vec::new();
        
        for entry in self.buckets.iter() {
            let bucket = entry.value();
            let last_refill = bucket.last_refill.lock().await;
            
            // Remove buckets that haven't been used for 5 minutes
            if now.duration_since(*last_refill) > Duration::from_secs(300) {
                to_remove.push(entry.key().clone());
            }
        }
        
        for key in to_remove {
            self.buckets.remove(&key);
        }
    }

    /// Get current statistics
    pub fn get_stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            global_requests: self.global_limit.load(Ordering::Relaxed),
            max_global_requests: self.max_requests_per_window,
            active_buckets: self.buckets.len(),
            max_buckets: self.max_buckets,
        }
    }

    /// Reset all rate limits (for testing/admin use)
    pub async fn reset(&self) {
        self.global_limit.store(0, Ordering::Relaxed);
        self.buckets.clear();
    }

    /// Set custom rate limit for a specific key
    pub async fn set_key_rate_limit(&self, key: &str, tokens_per_second: u32) -> Result<()> {
        let bucket = Arc::new(TokenBucket::new(tokens_per_second));
        self.buckets.insert(key.to_string(), bucket);
        Ok(())
    }

    /// Remove rate limit for a specific key
    pub fn remove_key_rate_limit(&self, key: &str) {
        self.buckets.remove(key);
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub global_requests: u64,
    pub max_global_requests: u64,
    pub active_buckets: usize,
    pub max_buckets: usize,
}

impl RateLimiterStats {
    /// Calculate global utilization percentage
    pub fn global_utilization(&self) -> f64 {
        if self.max_global_requests == 0 {
            0.0
        } else {
            (self.global_requests as f64 / self.max_global_requests as f64) * 100.0
        }
    }

    /// Calculate bucket utilization percentage
    pub fn bucket_utilization(&self) -> f64 {
        if self.max_buckets == 0 {
            0.0
        } else {
            (self.active_buckets as f64 / self.max_buckets as f64) * 100.0
        }
    }
}

#[async_trait::async_trait]
impl crate::rpc::RateLimiter for DistributedRateLimiter {
    async fn acquire(&self) -> Result<()> {
        // Use a default key for the trait implementation
        self.acquire("default").await
    }
}

/// Enhanced rate limiter with IP-based and user-based limiting
#[derive(Debug)]
pub struct EnhancedRateLimiter {
    inner: Arc<DistributedRateLimiter>,
    // IP-based limiting
    ip_buckets: Arc<DashMap<String, Arc<TokenBucket>>>,
    // User-based limiting
    user_buckets: Arc<DashMap<String, Arc<TokenBucket>>>,
    // Endpoint-based limiting
    endpoint_buckets: Arc<DashMap<String, Arc<TokenBucket>>>,
}

impl EnhancedRateLimiter {
    pub fn new(config: RateLimiterConfig) -> Self {
        let inner = Arc::new(DistributedRateLimiter::new(
            config.global_requests_per_window,
            config.window_duration,
            config.default_bucket_size,
            config.max_buckets,
        ));

        Self {
            inner,
            ip_buckets: Arc::new(DashMap::new()),
            user_buckets: Arc::new(DashMap::new()),
            endpoint_buckets: Arc::new(DashMap::new()),
        }
    }

    /// Acquire permit with multiple limiting strategies
    pub async fn acquire_multi(&self, request: &RateLimitRequest) -> Result<()> {
        // Check global limit
        self.inner.acquire("global").await?;
        
        // Check IP limit if provided
        if let Some(ip) = &request.ip_address {
            self.check_ip_limit(ip).await?;
        }
        
        // Check user limit if provided
        if let Some(user_id) = &request.user_id {
            self.check_user_limit(user_id).await?;
        }
        
        // Check endpoint limit if provided
        if let Some(endpoint) = &request.endpoint {
            self.check_endpoint_limit(endpoint).await?;
        }
        
        Ok(())
    }

    async fn check_ip_limit(&self, ip: &str) -> Result<()> {
        let bucket = self.ip_buckets.entry(ip.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(50))); // 50 requests per second per IP
        
        bucket.acquire().await
    }

    async fn check_user_limit(&self, user_id: &str) -> Result<()> {
        let bucket = self.user_buckets.entry(user_id.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(20))); // 20 requests per second per user
        
        bucket.acquire().await
    }

    async fn check_endpoint_limit(&self, endpoint: &str) -> Result<()> {
        let bucket = self.endpoint_buckets.entry(endpoint.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(200))); // 200 requests per second per endpoint
        
        bucket.acquire().await
    }

    /// Get comprehensive statistics
    pub fn get_comprehensive_stats(&self) -> ComprehensiveRateLimiterStats {
        let inner_stats = self.inner.get_stats();
        
        ComprehensiveRateLimiterStats {
            global: inner_stats,
            ip_buckets: self.ip_buckets.len(),
            user_buckets: self.user_buckets.len(),
            endpoint_buckets: self.endpoint_buckets.len(),
        }
    }
}

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    pub global_requests_per_window: u64,
    pub window_duration: Duration,
    pub default_bucket_size: u32,
    pub max_buckets: usize,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            global_requests_per_window: 1000,
            window_duration: Duration::from_secs(60),
            default_bucket_size: 100,
            max_buckets: 10000,
        }
    }
}

/// Rate limit request
#[derive(Debug, Clone)]
pub struct RateLimitRequest {
    pub ip_address: Option<String>,
    pub user_id: Option<String>,
    pub endpoint: Option<String>,
}

/// Comprehensive rate limiter statistics
#[derive(Debug, Clone)]
pub struct ComprehensiveRateLimiterStats {
    pub global: RateLimiterStats,
    pub ip_buckets: usize,
    pub user_buckets: usize,
    pub endpoint_buckets: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_distributed_rate_limiter() {
        let limiter = DistributedRateLimiter::new(10, Duration::from_secs(1), 5, 100);
        
        // Should allow first 5 requests
        for i in 0..5 {
            let result = limiter.acquire("test_key").await;
            assert!(result.is_ok(), "Request {} should succeed", i);
        }
        
        // 6th request should fail
        let result = limiter.acquire("test_key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_global_rate_limit() {
        let limiter = DistributedRateLimiter::new(3, Duration::from_secs(1), 10, 100);
        
        // Should allow first 3 requests globally
        for i in 0..3 {
            let result = limiter.acquire(&format!("key_{}", i)).await;
            assert!(result.is_ok(), "Global request {} should succeed", i);
        }
        
        // 4th request should fail globally
        let result = limiter.acquire("another_key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_enhanced_rate_limiter() {
        let config = RateLimiterConfig::default();
        let limiter = EnhancedRateLimiter::new(config);
        
        let request = RateLimitRequest {
            ip_address: Some("127.0.0.1".to_string()),
            user_id: Some("test_user".to_string()),
            endpoint: Some("/api/scan".to_string()),
        };
        
        // Should succeed
        let result = limiter.acquire_multi(&request).await;
        assert!(result.is_ok());
    }
}
