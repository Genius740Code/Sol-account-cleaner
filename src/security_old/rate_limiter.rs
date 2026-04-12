//! Advanced rate limiting with token bucket algorithm
//! Provides sophisticated rate limiting for API endpoints and wallet operations

use crate::core::{Result, SolanaRecoverError};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use tracing::{debug, warn, info, error};
use uuid::Uuid;

/// Token bucket rate limiter
pub struct TokenBucketRateLimiter {
    /// Rate limit configurations per client type
    configurations: Arc<RwLock<HashMap<String, RateLimitConfig>>>,
    /// Active token buckets per client
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    /// Global rate limiter statistics
    stats: Arc<RwLock<RateLimiterStats>>,
    /// Rate limiter configuration
    config: RateLimiterConfig,
}

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Client type identifier
    pub client_type: String,
    /// Maximum requests per second
    pub requests_per_second: f64,
    /// Maximum burst size
    pub burst_size: u32,
    /// Token refill interval
    pub refill_interval_ms: u64,
    /// Penalty for exceeded requests (seconds)
    pub penalty_seconds: u64,
    /// Enable adaptive rate limiting
    pub enable_adaptive: bool,
    /// Minimum rate when adaptive (requests per second)
    pub min_rate: f64,
    /// Maximum rate when adaptive (requests per second)
    pub max_rate: f64,
}

/// Token bucket state
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Current token count
    tokens: f64,
    /// Maximum tokens (burst size)
    max_tokens: f64,
    /// Last refill timestamp
    last_refill: Instant,
    /// Request count
    request_count: u64,
    /// Denied request count
    denied_count: u64,
    /// Penalty end time
    penalty_end: Option<Instant>,
    /// Client identifier
    client_id: String,
    /// Bucket creation time
    created_at: Instant,
}

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Default client type
    pub default_client_type: String,
    /// Cleanup interval for inactive buckets
    pub cleanup_interval_secs: u64,
    /// Bucket timeout (inactive duration)
    pub bucket_timeout_secs: u64,
    /// Enable distributed rate limiting
    pub enable_distributed: bool,
    /// Redis connection string for distributed mode
    pub redis_connection: Option<String>,
    /// Enable rate limit bypass for admin
    pub enable_admin_bypass: bool,
    /// Admin client identifiers
    admin_clients: Vec<String>,
}

/// Rate limiter statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RateLimiterStats {
    /// Total requests processed
    pub total_requests: u64,
    /// Total requests allowed
    pub allowed_requests: u64,
    /// Total requests denied
    pub denied_requests: u64,
    /// Active buckets
    pub active_buckets: usize,
    /// Average tokens per bucket
    pub avg_tokens_per_bucket: f64,
    /// Total penalties applied
    pub total_penalties: u64,
    /// Adaptive adjustments made
    pub adaptive_adjustments: u64,
    /// Cleanup operations performed
    pub cleanup_operations: u64,
}

/// Rate limit result
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether request is allowed
    pub allowed: bool,
    /// Remaining tokens
    pub remaining_tokens: f64,
    /// Time until next token (seconds)
    pub retry_after: Option<f64>,
    /// Current rate limit
    pub current_rate: f64,
    /// Bucket status
    pub bucket_status: BucketStatus,
}

/// Bucket status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BucketStatus {
    /// Normal operation
    Normal,
    /// Rate limit exceeded
    Exceeded,
    /// Penalty applied
    Penalized,
    /// Adaptive adjustment in progress
    AdaptiveAdjustment,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("RATE_LIMITING_ENABLED")
                .unwrap_or_else(|_| "true".to_string()) == "true",
            default_client_type: "default".to_string(),
            cleanup_interval_secs: 300, // 5 minutes
            bucket_timeout_secs: 1800, // 30 minutes
            enable_distributed: false,
            redis_connection: std::env::var("REDIS_CONNECTION").ok(),
            enable_admin_bypass: true,
            admin_clients: vec!["admin".to_string(), "system".to_string()],
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            client_type: "default".to_string(),
            requests_per_second: 10.0,
            burst_size: 100,
            refill_interval_ms: 100,
            penalty_seconds: 60,
            enable_adaptive: false,
            min_rate: 1.0,
            max_rate: 100.0,
        }
    }
}

impl TokenBucket {
    /// Create new token bucket
    fn new(client_id: String, config: &RateLimitConfig) -> Self {
        Self {
            tokens: config.burst_size as f64,
            max_tokens: config.burst_size as f64,
            last_refill: Instant::now(),
            request_count: 0,
            denied_count: 0,
            penalty_end: None,
            client_id,
            created_at: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time
    fn refill_tokens(&mut self, config: &RateLimitConfig) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let elapsed_secs = elapsed.as_secs_f64();
        
        let tokens_to_add = config.requests_per_second * elapsed_secs;
        self.tokens = (self.tokens + tokens_to_add).min(self.max_tokens);
        self.last_refill = now;
    }

    /// Check if request is allowed
    fn check_request(&mut self, config: &RateLimitConfig) -> RateLimitResult {
        self.request_count += 1;

        // Check if currently penalized
        if let Some(penalty_end) = self.penalty_end {
            if Instant::now() < penalty_end {
                return RateLimitResult {
                    allowed: false,
                    remaining_tokens: 0.0,
                    retry_after: Some(penalty_end.duration_since(Instant::now()).as_secs_f64()),
                    current_rate: 0.0,
                    bucket_status: BucketStatus::Penalized,
                };
            } else {
                self.penalty_end = None;
            }
        }

        self.refill_tokens(config);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            RateLimitResult {
                allowed: true,
                remaining_tokens: self.tokens,
                retry_after: None,
                current_rate: config.requests_per_second,
                bucket_status: BucketStatus::Normal,
            }
        } else {
            self.denied_count += 1;
            
            // Apply penalty if too many denials
            if self.denied_count > (config.burst_size / 2).into() {
                self.apply_penalty(config);
            }

            let retry_after = (1.0 - self.tokens) / config.requests_per_second;
            
            RateLimitResult {
                allowed: false,
                remaining_tokens: self.tokens,
                retry_after: Some(retry_after),
                current_rate: config.requests_per_second,
                bucket_status: BucketStatus::Exceeded,
            }
        }
    }

    /// Apply penalty for excessive requests
    fn apply_penalty(&mut self, config: &RateLimitConfig) {
        self.penalty_end = Some(Instant::now() + Duration::from_secs(config.penalty_seconds));
        self.tokens = 0.0;
    }

    /// Adaptive rate adjustment based on usage patterns
    fn adjust_rate_adaptively(&mut self, config: &mut RateLimitConfig) {
        if !config.enable_adaptive {
            return;
        }

        let usage_ratio = self.request_count as f64 / (self.created_at.elapsed().as_secs_f64() * config.requests_per_second);
        
        if usage_ratio > 0.8 {
            // High usage, increase rate
            let new_rate = (config.requests_per_second * 1.2).min(config.max_rate);
            config.requests_per_second = new_rate;
        } else if usage_ratio < 0.2 {
            // Low usage, decrease rate
            let new_rate = (config.requests_per_second * 0.8).max(config.min_rate);
            config.requests_per_second = new_rate;
        }
    }
}

impl TokenBucketRateLimiter {
    /// Create new rate limiter
    pub fn new(config: RateLimiterConfig) -> Self {
        let mut configurations = HashMap::new();
        
        // Add default configuration
        configurations.insert("default".to_string(), RateLimitConfig::default());
        
        Self {
            configurations: Arc::new(RwLock::new(configurations)),
            buckets: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RateLimiterStats::default())),
            config,
        }
    }

    /// Add rate limit configuration
    pub async fn add_configuration(&self, config: RateLimitConfig) -> Result<()> {
        let mut configurations = self.configurations.write().await;
        configurations.insert(config.client_type.clone(), config);
        info!("Added rate limit configuration for client type: {}", config.client_type);
        Ok(())
    }

    /// Check if request is allowed
    pub async fn check_rate_limit(&self, client_id: &str, client_type: Option<&str>) -> Result<RateLimitResult> {
        if !self.config.enabled {
            return Ok(RateLimitResult {
                allowed: true,
                remaining_tokens: f64::INFINITY,
                retry_after: None,
                current_rate: f64::INFINITY,
                bucket_status: BucketStatus::Normal,
            });
        }

        // Check admin bypass
        if self.config.enable_admin_bypass && self.config.admin_clients.contains(&client_id.to_string()) {
            return Ok(RateLimitResult {
                allowed: true,
                remaining_tokens: f64::INFINITY,
                retry_after: None,
                current_rate: f64::INFINITY,
                bucket_status: BucketStatus::Normal,
            });
        }

        let client_type = client_type.unwrap_or(&self.config.default_client_type);
        
        // Get configuration
        let config = {
            let configurations = self.configurations.read().await;
            configurations.get(client_type)
                .cloned()
                .unwrap_or_else(|| RateLimitConfig::default())
        };

        // Get or create bucket
        let bucket_key = format!("{}:{}", client_type, client_id);
        let mut bucket = {
            let mut buckets = self.buckets.write().await;
            buckets.entry(bucket_key.clone())
                .or_insert_with(|| TokenBucket::new(client_id.to_string(), &config))
                .clone()
        };

        // Check request
        let mut result = bucket.check_request(&config);

        // Adaptive adjustment if enabled
        if config.enable_adaptive {
            bucket.adjust_rate_adaptively(&mut config.clone());
            result.current_rate = config.requests_per_second;
            result.bucket_status = BucketStatus::AdaptiveAdjustment;
        }

        // Update bucket in storage
        {
            let mut buckets = self.buckets.write().await;
            buckets.insert(bucket_key, bucket);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
            if result.allowed {
                stats.allowed_requests += 1;
            } else {
                stats.denied_requests += 1;
            }
            stats.active_buckets = self.buckets.read().await.len();
        }

        debug!("Rate limit check for client '{}': allowed={}, remaining_tokens={:.2}", 
               client_id, result.allowed, result.remaining_tokens);

        Ok(result)
    }

    /// Check multiple requests (batch)
    pub async fn check_rate_limits_batch(&self, requests: &[(String, Option<String>)]) -> Vec<Result<RateLimitResult>> {
        let mut results = Vec::with_capacity(requests.len());
        
        for (client_id, client_type) in requests {
            let result = self.check_rate_limit(client_id, client_type.as_deref()).await;
            results.push(result);
        }
        
        results
    }

    /// Get rate limit statistics
    pub async fn get_stats(&self) -> RateLimiterStats {
        let stats = self.stats.read().await;
        let buckets = self.buckets.read().await;
        
        let avg_tokens = if buckets.is_empty() {
            0.0
        } else {
            buckets.values()
                .map(|bucket| bucket.tokens)
                .sum::<f64>() / buckets.len() as f64
        };

        RateLimiterStats {
            total_requests: stats.total_requests,
            allowed_requests: stats.allowed_requests,
            denied_requests: stats.denied_requests,
            active_buckets: stats.active_buckets,
            avg_tokens_per_bucket: avg_tokens,
            total_penalties: stats.total_penalties,
            adaptive_adjustments: stats.adaptive_adjustments,
            cleanup_operations: stats.cleanup_operations,
        }
    }

    /// Get bucket status for specific client
    pub async fn get_bucket_status(&self, client_id: &str, client_type: Option<&str>) -> Option<serde_json::Value> {
        let client_type = client_type.unwrap_or(&self.config.default_client_type);
        let bucket_key = format!("{}:{}", client_type, client_id);
        
        let buckets = self.buckets.read().await;
        if let Some(bucket) = buckets.get(&bucket_key) {
            Some(serde_json::json!({
                "client_id": bucket.client_id,
                "tokens": bucket.tokens,
                "max_tokens": bucket.max_tokens,
                "request_count": bucket.request_count,
                "denied_count": bucket.denied_count,
                "penalty_end": bucket.penalty_end,
                "created_at": bucket.created_at,
                "last_refill": bucket.last_refill,
            }))
        } else {
            None
        }
    }

    /// Reset rate limits for specific client
    pub async fn reset_client_limits(&self, client_id: &str, client_type: Option<&str>) -> Result<()> {
        let client_type = client_type.unwrap_or(&self.config.default_client_type);
        let bucket_key = format!("{}:{}", client_type, client_id);
        
        let mut buckets = self.buckets.write().await;
        if let Some(bucket) = buckets.get_mut(&bucket_key) {
            bucket.tokens = bucket.max_tokens;
            bucket.penalty_end = None;
            bucket.denied_count = 0;
            info!("Reset rate limits for client '{}'", client_id);
        }
        
        Ok(())
    }

    /// Cleanup inactive buckets
    pub async fn cleanup_inactive_buckets(&self) -> Result<usize> {
        let mut buckets = self.buckets.write().await;
        let initial_count = buckets.len();
        
        let timeout = Duration::from_secs(self.config.bucket_timeout_secs);
        let now = Instant::now();
        
        buckets.retain(|_, bucket| {
            let is_active = now.duration_since(bucket.last_refill) < timeout;
            if !is_active {
                debug!("Removing inactive bucket for client '{}'", bucket.client_id);
            }
            is_active
        });
        
        let removed_count = initial_count - buckets.len();
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.cleanup_operations += 1;
            stats.active_buckets = buckets.len();
        }
        
        if removed_count > 0 {
            info!("Cleaned up {} inactive rate limit buckets", removed_count);
        }
        
        Ok(removed_count)
    }

    /// Get comprehensive rate limiter report
    pub async fn get_report(&self) -> serde_json::Value {
        let stats = self.get_stats().await;
        let configurations = self.configurations.read().await;
        
        serde_json::json!({
            "enabled": self.config.enabled,
            "default_client_type": self.config.default_client_type,
            "statistics": stats,
            "configurations": configurations,
            "config": self.config,
        })
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>) {
        let cleanup_interval = Duration::from_secs(self.config.cleanup_interval_secs);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                if let Ok(removed) = self.cleanup_inactive_buckets().await {
                    if removed > 0 {
                        info!("Rate limiter cleanup: removed {} inactive buckets", removed);
                    }
                }
            }
        });
    }
}

/// Builder for rate limit configuration
pub struct RateLimitConfigBuilder {
    config: RateLimitConfig,
}

impl RateLimitConfigBuilder {
    /// Create new builder
    pub fn new(client_type: String) -> Self {
        Self {
            config: RateLimitConfig {
                client_type,
                ..Default::default()
            },
        }
    }

    /// Set requests per second
    pub fn requests_per_second(mut self, rps: f64) -> Self {
        self.config.requests_per_second = rps;
        self
    }

    /// Set burst size
    pub fn burst_size(mut self, size: u32) -> Self {
        self.config.burst_size = size;
        self
    }

    /// Set penalty seconds
    pub fn penalty_seconds(mut self, seconds: u64) -> Self {
        self.config.penalty_seconds = seconds;
        self
    }

    /// Enable adaptive rate limiting
    pub fn enable_adaptive(mut self, min_rate: f64, max_rate: f64) -> Self {
        self.config.enable_adaptive = true;
        self.config.min_rate = min_rate;
        self.config.max_rate = max_rate;
        self
    }

    /// Build configuration
    pub fn build(self) -> RateLimitConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let config = RateLimiterConfig::default();
        let limiter = TokenBucketRateLimiter::new(config);

        // Add test configuration
        let rate_config = RateLimitConfigBuilder::new("test".to_string())
            .requests_per_second(10.0)
            .burst_size(20)
            .build();
        
        limiter.add_configuration(rate_config).await.unwrap();

        // Test rate limiting
        for i in 0..25 {
            let result = limiter.check_rate_limit("test_client", Some("test")).await.unwrap();
            if i < 20 {
                assert!(result.allowed, "Request {} should be allowed", i);
            } else {
                assert!(!result.allowed, "Request {} should be denied", i);
            }
        }

        let stats = limiter.get_stats().await;
        assert_eq!(stats.total_requests, 25);
        assert_eq!(stats.allowed_requests, 20);
        assert_eq!(stats.denied_requests, 5);
    }

    #[tokio::test]
    async fn test_admin_bypass() {
        let mut config = RateLimiterConfig::default();
        config.enable_admin_bypass = true;
        config.admin_clients.push("admin_user".to_string());
        
        let limiter = TokenBucketRateLimiter::new(config);

        // Admin should bypass rate limits
        for _ in 0..100 {
            let result = limiter.check_rate_limit("admin_user", None).await.unwrap();
            assert!(result.allowed);
        }
    }

    #[tokio::test]
    async fn test_adaptive_rate_limiting() {
        let config = RateLimiterConfig::default();
        let limiter = TokenBucketRateLimiter::new(config);

        // Add adaptive configuration
        let rate_config = RateLimitConfigBuilder::new("adaptive".to_string())
            .requests_per_second(10.0)
            .burst_size(20)
            .enable_adaptive(5.0, 50.0)
            .build();
        
        limiter.add_configuration(rate_config).await.unwrap();

        // Test adaptive behavior
        let client_id = "adaptive_client";
        
        // Make many requests to trigger adaptation
        for _ in 0..50 {
            let result = limiter.check_rate_limit(client_id, Some("adaptive")).await.unwrap();
            // Some requests should be allowed due to adaptation
            if result.allowed {
                break;
            }
        }

        let bucket_status = limiter.get_bucket_status(client_id, Some("adaptive")).await;
        assert!(bucket_status.is_some());
    }

    #[tokio::test]
    async fn test_batch_rate_limiting() {
        let config = RateLimiterConfig::default();
        let limiter = TokenBucketRateLimiter::new(config);

        let requests = vec![
            ("client1".to_string(), Some("default".to_string())),
            ("client2".to_string(), Some("default".to_string())),
            ("client1".to_string(), Some("default".to_string())),
        ];

        let results = limiter.check_rate_limits_batch(&requests).await;
        assert_eq!(results.len(), 3);
        
        // All should be allowed initially
        for result in &results {
            assert!(result.as_ref().unwrap().allowed);
        }
    }

    #[tokio::test]
    async fn test_cleanup_inactive_buckets() {
        let mut config = RateLimiterConfig::default();
        config.bucket_timeout_secs = 1; // 1 second timeout
        
        let limiter = TokenBucketRateLimiter::new(config);

        // Create some buckets
        limiter.check_rate_limit("client1", None).await.unwrap();
        limiter.check_rate_limit("client2", None).await.unwrap();

        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Cleanup
        let removed = limiter.cleanup_inactive_buckets().await.unwrap();
        assert_eq!(removed, 2);

        let stats = limiter.get_stats().await;
        assert_eq!(stats.active_buckets, 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_report() {
        let config = RateLimiterConfig::default();
        let limiter = TokenBucketRateLimiter::new(config);

        // Make some requests
        limiter.check_rate_limit("test_client", None).await.unwrap();
        limiter.check_rate_limit("test_client", None).await.unwrap();

        let report = limiter.get_report().await;
        
        assert!(report["enabled"].as_bool().unwrap());
        assert!(report["statistics"]["total_requests"].as_u64().unwrap() > 0);
        assert!(report["configurations"].as_object().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_reset_client_limits() {
        let config = RateLimiterConfig::default();
        let limiter = TokenBucketRateLimiter::new(config);

        // Exhaust rate limit
        for _ in 0..150 {
            limiter.check_rate_limit("test_client", None).await.unwrap();
        }

        // Should be rate limited
        let result = limiter.check_rate_limit("test_client", None).await.unwrap();
        assert!(!result.allowed);

        // Reset limits
        limiter.reset_client_limits("test_client", None).await.unwrap();

        // Should be allowed again
        let result = limiter.check_rate_limit("test_client", None).await.unwrap();
        assert!(result.allowed);
    }
}
