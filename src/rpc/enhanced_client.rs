use crate::core::{Result, SolanaRecoverError};
use crate::storage::{HierarchicalCache, HierarchicalCacheConfig, CachedWalletInfo};
use crate::utils::memory_integration::{MemoryIntegrationLayer, RpcMemoryManager};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_client::rpc_filter::{RpcFilterType, Memcmp, MemcmpEncodedBytes};
use solana_client::rpc_config::{RpcProgramAccountsConfig, RpcAccountInfoConfig};
use solana_sdk::{pubkey::Pubkey, commitment_config::CommitmentConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};
use moka::future::Cache;
use base64::Engine;
use std::str::FromStr;
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};

/// Enhanced RPC client with integrated memory management
pub struct EnhancedRpcClientWrapper {
    /// Base RPC client
    client: Arc<RpcClient>,
    
    /// Rate limiter
    rate_limiter: Arc<dyn RateLimiter>,
    
    /// Request timeout
    request_timeout: Duration,
    
    /// Rent cache
    rent_cache: Cache<usize, u64>,
    
    /// Hierarchical cache
    hierarchical_cache: Option<Arc<HierarchicalCache>>,
    
    /// Memory management integration
    memory_integration: Arc<MemoryIntegrationLayer>,
    rpc_memory_manager: RpcMemoryManager,
    
    /// Enhanced client configuration
    config: EnhancedRpcConfig,
    
    /// Performance metrics
    metrics: Arc<RpcMetrics>,
}

#[derive(Debug, Clone)]
pub struct EnhancedRpcConfig {
    /// Enable memory pooling for RPC operations
    pub enable_memory_pooling: bool,
    
    /// Enable performance tracking
    pub enable_performance_tracking: bool,
    
    /// Enable buffer pooling for network operations
    pub enable_buffer_pooling: bool,
    
    /// Request/response configuration
    pub request_config: RequestConfig,
    
    /// Memory optimization settings
    pub memory_config: RpcMemoryConfig,
    
    /// Caching configuration
    pub cache_config: RpcCacheConfig,
}

#[derive(Debug, Clone)]
pub struct RequestConfig {
    /// Maximum request size in bytes
    pub max_request_size: usize,
    
    /// Maximum response size in bytes
    pub max_response_size: usize,
    
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    
    /// Enable request compression
    pub enable_compression: bool,
    
    /// Retry configuration
    pub retry_config: RetryConfig,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,
    
    /// Initial retry delay in milliseconds
    pub initial_delay_ms: u64,
    
    /// Maximum retry delay in milliseconds
    pub max_delay_ms: u64,
    
    /// Enable exponential backoff
    pub enable_exponential_backoff: bool,
    
    /// Enable jitter
    pub enable_jitter: bool,
}

#[derive(Debug, Clone)]
pub struct RpcMemoryConfig {
    /// Pool size for request buffers
    pub request_buffer_pool_size: usize,
    
    /// Pool size for response buffers
    pub response_buffer_pool_size: usize,
    
    /// Pool size for account data buffers
    pub account_data_buffer_pool_size: usize,
    
    /// Enable memory tracking for RPC operations
    pub enable_memory_tracking: bool,
    
    /// Memory optimization interval in seconds
    pub memory_optimization_interval_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct RpcCacheConfig {
    /// Enable hierarchical caching
    pub enable_hierarchical_cache: bool,
    
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    
    /// Maximum cache size
    pub max_cache_size: usize,
    
    /// Enable cache warming
    pub enable_cache_warming: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RpcMetrics {
    /// Total RPC requests
    pub total_requests: u64,
    
    /// Successful requests
    pub successful_requests: u64,
    
    /// Failed requests
    pub failed_requests: u64,
    
    /// Total request time in milliseconds
    pub total_request_time_ms: u64,
    
    /// Average request time in milliseconds
    pub average_request_time_ms: f64,
    
    /// Cache hits
    pub cache_hits: u64,
    
    /// Cache misses
    pub cache_misses: u64,
    
    /// Memory operations
    pub memory_operations: u64,
    
    /// Memory saved through pooling (bytes)
    pub memory_saved_bytes: usize,
}

impl Default for EnhancedRpcConfig {
    fn default() -> Self {
        Self {
            enable_memory_pooling: true,
            enable_performance_tracking: true,
            enable_buffer_pooling: true,
            request_config: RequestConfig::default(),
            memory_config: RpcMemoryConfig::default(),
            cache_config: RpcCacheConfig::default(),
        }
    }
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            max_request_size: 64 * 1024,  // 64KB
            max_response_size: 1024 * 1024, // 1MB
            request_timeout_ms: 30000,      // 30 seconds
            enable_compression: false,       // Enable when compression library is available
            retry_config: RetryConfig::default(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            enable_exponential_backoff: true,
            enable_jitter: true,
        }
    }
}

impl Default for RpcMemoryConfig {
    fn default() -> Self {
        Self {
            request_buffer_pool_size: 1000,
            response_buffer_pool_size: 1000,
            account_data_buffer_pool_size: 2000,
            enable_memory_tracking: true,
            memory_optimization_interval_seconds: 300, // 5 minutes
        }
    }
}

impl Default for RpcCacheConfig {
    fn default() -> Self {
        Self {
            enable_hierarchical_cache: true,
            cache_ttl_seconds: 300, // 5 minutes
            max_cache_size: 10000,
            enable_cache_warming: false,
        }
    }
}

impl EnhancedRpcClientWrapper {
    /// Create new enhanced RPC client
    pub fn new(client: Arc<RpcClient>, rate_limiter: Arc<dyn RateLimiter>) -> Self {
        Self::with_config(client, rate_limiter, EnhancedRpcConfig::default())
    }
    
    /// Create enhanced RPC client with custom configuration
    pub fn with_config(
        client: Arc<RpcClient>,
        rate_limiter: Arc<dyn RateLimiter>,
        config: EnhancedRpcConfig,
    ) -> Self {
        let rent_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(300))
            .build();
        
        let memory_integration = crate::utils::memory_integration::get_global_memory_integration();
        let rpc_memory_manager = memory_integration.create_rpc_memory_manager();
        
        Self {
            client,
            rate_limiter,
            request_timeout: Duration::from_millis(config.request_config.request_timeout_ms),
            rent_cache,
            hierarchical_cache: None,
            memory_integration,
            rpc_memory_manager,
            config,
            metrics: Arc::new(RpcMetrics::default()),
        }
    }
    
    /// Create enhanced RPC client from URL
    pub fn from_url(url: &str, config: EnhancedRpcConfig) -> Result<Self> {
        let client = Arc::new(RpcClient::new_with_timeout(
            url.to_string(),
            Duration::from_millis(config.request_config.request_timeout_ms),
        ));
        let rate_limiter = Arc::new(TokenBucketRateLimiter::new(100));
        
        Ok(Self::with_config(client, rate_limiter, config))
    }
    
    /// Set hierarchical cache
    pub async fn with_hierarchical_cache(mut self, cache_config: HierarchicalCacheConfig) -> Result<Self> {
        if self.config.cache_config.enable_hierarchical_cache {
            let cache = HierarchicalCache::new(cache_config).await?;
            self.hierarchical_cache = Some(Arc::new(cache));
        }
        Ok(self)
    }
    
    /// Enhanced method to get all recoverable accounts with memory optimization
    pub async fn get_all_recoverable_accounts_enhanced(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        let start_time = Instant::now();
        
        // Update metrics
        if self.config.enable_performance_tracking {
            let mut metrics = self.metrics.clone();
            metrics.total_requests += 1;
        }
        
        let cache_key = format!("recoverable_accounts:{}", pubkey);
        
        // Try hierarchical cache first
        if let Some(ref cache) = self.hierarchical_cache {
            if let Ok(Some(cached_accounts)) = cache.get::<Vec<solana_client::rpc_response::RpcKeyedAccount>>(&cache_key).await {
                debug!("Cache hit for recoverable accounts of {}", pubkey);
                
                if self.config.enable_performance_tracking {
                    self.metrics.cache_hits += 1;
                }
                
                return Ok(cached_accounts);
            } else if self.config.enable_performance_tracking {
                self.metrics.cache_misses += 1;
            }
        }
        
        // Cache miss - fetch from RPC with memory optimization
        let mut all_accounts = self.get_token_accounts_enhanced(pubkey).await?;
        
        // Add OpenBook accounts
        let openbook_accounts = self.get_openbook_accounts_enhanced(pubkey).await?;
        all_accounts.extend(openbook_accounts);
        
        // Cache the result
        if let Some(ref cache) = self.hierarchical_cache {
            if let Err(e) = cache.set(&cache_key, &all_accounts).await {
                warn!("Failed to cache recoverable accounts: {}", e);
            }
        }
        
        // Update metrics
        if self.config.enable_performance_tracking {
            let duration = start_time.elapsed();
            let mut metrics = self.metrics.clone();
            metrics.successful_requests += 1;
            metrics.total_request_time_ms += duration.as_millis() as u64;
            metrics.average_request_time_ms = metrics.total_request_time_ms as f64 / metrics.successful_requests as f64;
        }
        
        Ok(all_accounts)
    }
    
    /// Enhanced token account fetching with memory pooling
    async fn get_token_accounts_enhanced(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        // Use pooled request buffer
        let _request_buffer = if self.config.enable_memory_pooling {
            Some(self.rpc_memory_manager.acquire_request_buffer(1024))
        } else {
            None
        };
        
        // Apply rate limiting
        self.rate_limiter.acquire().await?;
        
        // Create request with retry logic
        let result = self.execute_with_retry(|| {
            let client = self.client.clone();
            let pubkey = *pubkey;
            async move {
                let filter = TokenAccountsFilter::ProgramId(spl_token::id());
                let config = RpcProgramAccountsConfig {
                    filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                        0,
                        MemcmpEncodedBytes::Base58(base64::encode(pubkey.to_bytes())),
                    ))]),
                    account_config: RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        data_slice: None,
                        commitment: Some(CommitmentConfig::confirmed()),
                        min_context_slot: None,
                    },
                    with_context: false,
                };
                
                client.get_program_accounts_with_config(&spl_token::id(), config).await
            }
        }).await?;
        
        // Use pooled response buffer for processing
        if self.config.enable_memory_pooling {
            let _response_buffer = self.rpc_memory_manager.acquire_response_buffer("token_accounts");
        }
        
        Ok(result)
    }
    
    /// Enhanced OpenBook account fetching with memory pooling
    async fn get_openbook_accounts_enhanced(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        // Use pooled request buffer
        let _request_buffer = if self.config.enable_memory_pooling {
            Some(self.rpc_memory_manager.acquire_request_buffer(1024))
        } else {
            None
        };
        
        // Apply rate limiting
        self.rate_limiter.acquire().await?;
        
        // Create request with retry logic
        let result = self.execute_with_retry(|| {
            let client = self.client.clone();
            let pubkey = *pubkey;
            async move {
                let filter = TokenAccountsFilter::ProgramId(spl_token::id());
                let config = RpcProgramAccountsConfig {
                    filters: Some(vec![
                        RpcFilterType::Memcmp(Memcmp::new(
                            0,
                            MemcmpEncodedBytes::Base58(base64::encode(pubkey.to_bytes())),
                        )),
                        // Add OpenBook-specific filters
                    ]),
                    account_config: RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        data_slice: None,
                        commitment: Some(CommitmentConfig::confirmed()),
                        min_context_slot: None,
                    },
                    with_context: false,
                };
                
                client.get_program_accounts_with_config(&spl_token::id(), config).await
            }
        }).await?;
        
        // Filter for OpenBook accounts (simplified)
        let openbook_accounts: Vec<_> = result.into_iter()
            .filter(|account| {
                // Check if this is an OpenBook account based on account data
                // This is a simplified check - in practice you'd decode the account data
                account.account.owner == spl_token::id() && account.account.lamports == 0
            })
            .collect();
        
        Ok(openbook_accounts)
    }
    
    /// Execute request with retry logic and memory optimization
    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, solana_client::rpc_client::RpcError>>,
    {
        let mut attempt = 0;
        let mut delay = self.config.request_config.retry_config.initial_delay_ms;
        
        loop {
            attempt += 1;
            
            // Optimize memory before request if needed
            if self.config.memory_config.enable_memory_tracking && attempt > 1 {
                self.optimize_memory_for_request().await;
            }
            
            match operation().await {
                Ok(result) => {
                    return Ok(result);
                }
                Err(e) => {
                    if attempt >= self.config.request_config.retry_config.max_attempts {
                        error!("Request failed after {} attempts: {}", attempt, e);
                        
                        if self.config.enable_performance_tracking {
                            self.metrics.failed_requests += 1;
                        }
                        
                        return Err(SolanaRecoverError::RpcError(e.to_string()));
                    }
                    
                    warn!("Request attempt {} failed: {}, retrying in {}ms", attempt, e, delay);
                    
                    // Wait before retry
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    
                    // Calculate next delay
                    if self.config.request_config.retry_config.enable_exponential_backoff {
                        delay = (delay * 2).min(self.config.request_config.retry_config.max_delay_ms);
                    }
                    
                    // Add jitter if enabled
                    if self.config.request_config.retry_config.enable_jitter {
                        delay += rand::random::<u64>() % 100;
                    }
                }
            }
        }
    }
    
    /// Optimize memory for RPC requests
    async fn optimize_memory_for_request(&self) {
        debug!("Optimizing memory for RPC request");
        
        // Check memory pressure
        let memory_stats = self.memory_integration.get_memory_manager().get_memory_stats();
        if memory_stats.memory_pressure > 75.0 {
            // Trigger GC
            self.memory_integration.get_gc_scheduler().schedule_gc(memory_stats.memory_pressure).await;
        }
        
        // Clean up buffer pools
        let buffer_pool = self.memory_integration.get_buffer_pool();
        buffer_pool.cleanup_old_buffers().await;
    }
    
    /// Get RPC client performance metrics
    pub fn get_rpc_metrics(&self) -> RpcMetrics {
        self.metrics.clone()
    }
    
    /// Get comprehensive RPC performance report
    pub fn get_performance_report(&self) -> serde_json::Value {
        let metrics = self.get_rpc_metrics();
        let rpc_stats = self.rpc_memory_manager.get_rpc_stats();
        let memory_stats = self.memory_integration.get_memory_manager().get_memory_stats();
        
        serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "metrics": metrics,
            "rpc_memory_stats": rpc_stats,
            "memory_stats": memory_stats,
            "config": self.config,
            "recommendations": self.generate_rpc_recommendations(&metrics),
        })
    }
    
    fn generate_rpc_recommendations(&self, metrics: &RpcMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if metrics.cache_hits + metrics.cache_misses > 0 {
            let hit_rate = metrics.cache_hits as f64 / (metrics.cache_hits + metrics.cache_misses) as f64 * 100.0;
            if hit_rate < 50.0 {
                recommendations.push(format!("Low cache hit rate: {:.1}%. Consider cache warming or increasing cache size.", hit_rate));
            }
        }
        
        if metrics.average_request_time_ms > 1000.0 {
            recommendations.push("High average request time detected. Consider optimizing network configuration or enabling request compression.".to_string());
        }
        
        if metrics.memory_saved_bytes < 1024 * 1024 { // Less than 1MB saved
            recommendations.push("Low memory savings detected. Consider increasing buffer pool sizes or enabling memory pooling.".to_string());
        }
        
        if !self.config.enable_memory_pooling {
            recommendations.push("Memory pooling is disabled. Enable it for improved performance.".to_string());
        }
        
        if !self.config.enable_performance_tracking {
            recommendations.push("Performance tracking is disabled. Enable it for better monitoring and optimization.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("RPC client is configured optimally. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

impl std::fmt::Debug for EnhancedRpcClientWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnhancedRpcClientWrapper")
            .field("request_timeout", &self.request_timeout)
            .field("config", &self.config)
            .finish()
    }
}

// Rate limiter trait (simplified)
#[async_trait::async_trait]
pub trait RateLimiter: Send + Sync {
    async fn acquire(&self) -> Result<()>;
}

// Token bucket rate limiter implementation
pub struct TokenBucketRateLimiter {
    tokens: Arc<tokio::sync::Mutex<usize>>,
    max_tokens: usize,
    refill_rate: usize,
    last_refill: Arc<tokio::sync::Mutex<Instant>>,
}

impl TokenBucketRateLimiter {
    pub fn new(requests_per_second: usize) -> Self {
        Self {
            tokens: Arc::new(tokio::sync::Mutex::new(requests_per_second)),
            max_tokens: requests_per_second,
            refill_rate: requests_per_second,
            last_refill: Arc::new(tokio::sync::Mutex::new(Instant::now())),
        }
    }
}

#[async_trait::async_trait]
impl RateLimiter for TokenBucketRateLimiter {
    async fn acquire(&self) -> Result<()> {
        let mut tokens = self.tokens.lock().await;
        let mut last_refill = self.last_refill.lock().await;
        
        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);
        let tokens_to_add = (elapsed.as_secs() as usize * self.refill_rate) / 1;
        
        if tokens_to_add > 0 {
            *tokens = (*tokens + tokens_to_add).min(self.max_tokens);
            *last_refill = now;
        }
        
        if *tokens > 0 {
            *tokens -= 1;
            Ok(())
        } else {
            Err(SolanaRecoverError::RateLimitExceeded("Rate limit exceeded".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::mock::MockRpcClient;
    
    #[tokio::test]
    async fn test_enhanced_rpc_client_creation() {
        let client = Arc::new(MockRpcClient::new());
        let rate_limiter = Arc::new(TokenBucketRateLimiter::new(100));
        let enhanced_client = EnhancedRpcClientWrapper::new(client, rate_limiter);
        
        let metrics = enhanced_client.get_rpc_metrics();
        assert_eq!(metrics.total_requests, 0);
    }
    
    #[tokio::test]
    async fn test_enhanced_rpc_client_from_url() {
        let config = EnhancedRpcConfig::default();
        let enhanced_client = EnhancedRpcClientWrapper::from_url("https://api.mainnet-beta.solana.com", config);
        
        assert!(enhanced_client.is_ok());
    }
    
    #[tokio::test]
    async fn test_performance_report() {
        let client = Arc::new(MockRpcClient::new());
        let rate_limiter = Arc::new(TokenBucketRateLimiter::new(100));
        let enhanced_client = EnhancedRpcClientWrapper::new(client, rate_limiter);
        
        let report = enhanced_client.get_performance_report();
        
        assert!(report.get("timestamp").is_some());
        assert!(report.get("metrics").is_some());
        assert!(report.get("config").is_some());
        assert!(report.get("recommendations").is_some());
    }
    
    #[tokio::test]
    async fn test_retry_logic() {
        let client = Arc::new(MockRpcClient::new());
        let rate_limiter = Arc::new(TokenBucketRateLimiter::new(100));
        let mut config = EnhancedRpcConfig::default();
        config.request_config.retry_config.max_attempts = 3;
        
        let enhanced_client = EnhancedRpcClientWrapper::new(client, rate_limiter);
        
        // Test retry logic would require a mock client that fails initially
        // This is a placeholder for the actual test
    }
}
