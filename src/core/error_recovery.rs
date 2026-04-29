//! Error Recovery System with Circuit Breaker Pattern
//! 
//! This module provides comprehensive error recovery mechanisms including circuit breakers,
//! retry policies, fallback strategies, and graceful degradation.

use crate::core::{Result, SolanaRecoverError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed and allowing requests
    Closed,
    /// Circuit is open and rejecting requests
    Open,
    /// Circuit is half-open and testing requests
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: usize,
    
    /// Time to wait before transitioning from Open to HalfOpen
    pub recovery_timeout: Duration,
    
    /// Number of successful requests in HalfOpen before closing
    pub success_threshold: usize,
    
    /// Percentage of requests that can fail before opening
    pub failure_rate_threshold: f64,
    
    /// Minimum number of requests before considering failure rate
    pub minimum_requests: usize,
    
    /// Time window for tracking requests
    pub sliding_window_size: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 3,
            failure_rate_threshold: 0.5,
            minimum_requests: 10,
            sliding_window_size: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Request tracking for circuit breaker
#[derive(Debug, Clone)]
struct RequestRecord {
    timestamp: Instant,
    success: bool,
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<usize>>,
    success_count: Arc<RwLock<usize>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    request_history: Arc<RwLock<Vec<RequestRecord>>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }
    
    /// Create a new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            request_history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Execute an operation with circuit breaker protection
    pub async fn execute<F, T, Fut>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if circuit allows the request
        if !self.can_execute().await {
            return Err(SolanaRecoverError::CircuitBreakerOpen(
                "Circuit breaker is open, rejecting request".to_string()
            ));
        }
        
        // Execute the operation
        let result = operation().await;
        
        // Record the result
        self.record_result(result.is_ok()).await;
        
        result
    }
    
    /// Check if the circuit breaker allows requests
    async fn can_execute(&self) -> bool {
        let state = self.state.read().await;
        
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if recovery timeout has passed
                let last_failure = self.last_failure_time.read().await;
                if let Some(failure_time) = *last_failure {
                    failure_time.elapsed() >= self.config.recovery_timeout
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
    
    /// Record the result of an operation
    async fn record_result(&self, success: bool) {
        let now = Instant::now();
        
        // Record the request
        {
            let mut history = self.request_history.write().await;
            history.push(RequestRecord { timestamp: now, success });
            
            // Clean old records outside the sliding window
            let cutoff = now - self.config.sliding_window_size;
            history.retain(|record| record.timestamp >= cutoff);
        }
        
        // Update counters and state
        if success {
            self.record_success().await;
        } else {
            self.record_failure().await;
        }
    }
    
    async fn record_success(&self) {
        {
            let mut success_count = self.success_count.write().await;
            *success_count += 1;
        }
        
        let mut state = self.state.write().await;
        match *state {
            CircuitState::HalfOpen => {
                // Check if we've reached success threshold
                let success_count = *self.success_count.read().await;
                if success_count >= self.config.success_threshold {
                    info!("Circuit breaker closing after {} successful requests", success_count);
                    *state = CircuitState::Closed;
                    self.reset_counters().await;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                let mut failure_count = self.failure_count.write().await;
                *failure_count = 0;
            }
            _ => {}
        }
    }
    
    async fn record_failure(&self) {
        {
            let mut failure_count = self.failure_count.write().await;
            *failure_count += 1;
        }
        
        {
            let mut last_failure = self.last_failure_time.write().await;
            *last_failure = Some(Instant::now());
        }
        
        let mut state = self.state.write().await;
        match *state {
            CircuitState::Closed => {
                if self.should_open_circuit().await {
                    warn!("Circuit breaker opening after {} failures", *self.failure_count.read().await);
                    *state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state opens the circuit again
                warn!("Circuit breaker opening again due to failure in half-open state");
                *state = CircuitState::Open;
            }
            _ => {}
        }
    }
    
    async fn should_open_circuit(&self) -> bool {
        let failure_count = *self.failure_count.read().await;
        
        // Check absolute failure threshold
        if failure_count >= self.config.failure_threshold {
            return true;
        }
        
        // Check failure rate threshold
        let history = self.request_history.read().await;
        if history.len() >= self.config.minimum_requests {
            let failure_rate = history.iter()
                .filter(|record| !record.success)
                .count() as f64 / history.len() as f64;
            
            if failure_rate >= self.config.failure_rate_threshold {
                return true;
            }
        }
        
        false
    }
    
    async fn reset_counters(&self) {
        let mut failure_count = self.failure_count.write().await;
        let mut success_count = self.success_count.write().await;
        *failure_count = 0;
        *success_count = 0;
    }
    
    /// Get the current circuit state
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }
    
    /// Force the circuit to open
    pub async fn force_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open;
        info!("Circuit breaker forced open");
    }
    
    /// Force the circuit to close
    pub async fn force_close(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.reset_counters().await;
        info!("Circuit breaker forced closed");
    }
}

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    
    /// Base delay between retries
    pub base_delay: Duration,
    
    /// Maximum delay between retries
    pub max_delay: Duration,
    
    /// Backoff multiplier (for exponential backoff)
    pub backoff_multiplier: f64,
    
    /// Jitter factor to add randomness
    pub jitter_factor: f64,
    
    /// Retryable error types
    pub retryable_errors: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
            retryable_errors: vec![
                "timeout".to_string(),
                "network".to_string(),
                "rate_limit".to_string(),
                "temporary".to_string(),
            ],
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for the given attempt number
    pub fn calculate_delay(&self, attempt: usize) -> Duration {
        let multiplier = self.backoff_multiplier.powi(attempt as i32 - 1);
        let exponential_delay_ms = self.base_delay.as_millis() as f64 * multiplier;
        let delay_ms = exponential_delay_ms.min(self.max_delay.as_millis() as f64);
        
        // Add jitter
        let jitter = delay_ms * self.jitter_factor;
        let jitter_delay_ms = delay_ms + jitter;
        
        Duration::from_millis(jitter_delay_ms as u64)
    }
    
    /// Check if an error is retryable
    pub fn is_retryable(&self, error: &SolanaRecoverError) -> bool {
        let error_string = format!("{:?}", error).to_lowercase();
        
        self.retryable_errors.iter().any(|retryable_error| {
            error_string.contains(&retryable_error.to_lowercase())
        })
    }
}

/// Retry mechanism with exponential backoff
pub struct RetryMechanism {
    policy: RetryPolicy,
}

impl RetryMechanism {
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy }
    }
    
    pub fn with_default_policy() -> Self {
        Self::new(RetryPolicy::default())
    }
    
    /// Execute an operation with retry logic
    pub async fn execute<F, T, Fut>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;
        
        for attempt in 1..=self.policy.max_attempts {
            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        info!("Operation succeeded on attempt {}", attempt);
                    }
                    return Ok(result);
                }
                Err(error) => {
                    last_error = Some(error.clone());
                    
                    if attempt == self.policy.max_attempts {
                        error!("Operation failed after {} attempts", attempt);
                        break;
                    }
                    
                    if !self.policy.is_retryable(&error) {
                        warn!("Error is not retryable: {:?}", error);
                        break;
                    }
                    
                    let delay = self.policy.calculate_delay(attempt);
                    warn!("Attempt {} failed, retrying in {:?}: {:?}", attempt, delay, error);
                    tokio::time::sleep(delay).await;
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| {
            SolanaRecoverError::InternalError("Operation failed with no error recorded".to_string())
        }))
    }
}

/// Fallback strategy trait
#[async_trait]
pub trait FallbackStrategy: Send + Sync {
    async fn execute_fallback(&self, wallet_address: &str) -> Result<crate::core::WalletInfo>;
    fn name(&self) -> &str;
}

/// Simple RPC fallback strategy
pub struct SimpleRpcFallback {
    rpc_endpoint: String,
}

impl SimpleRpcFallback {
    pub fn new(rpc_endpoint: String) -> Self {
        Self { rpc_endpoint }
    }
}

#[async_trait]
impl FallbackStrategy for SimpleRpcFallback {
    async fn execute_fallback(&self, wallet_address: &str) -> Result<crate::core::WalletInfo> {
        use crate::core::scanner::WalletScanner;
        use crate::rpc::ConnectionPool;
        use crate::core::types::RpcEndpoint;
        use std::sync::Arc;
        
        info!("Executing simple RPC fallback for wallet: {}", wallet_address);
        
        // Create a simple connection pool with fallback endpoint
        let rpc_endpoint = RpcEndpoint {
            url: self.rpc_endpoint.clone(),
            priority: 1,
            rate_limit_rps: 50, // Conservative rate limit for fallback
            timeout_ms: 60000,  // Longer timeout for fallback
            healthy: true,
        };
        
        let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 2));
        let scanner = WalletScanner::new(connection_pool);
        
        // Perform simple scan
        let scan_result = scanner.scan_wallet(wallet_address).await?;
        
        scan_result.result.ok_or_else(|| {
            SolanaRecoverError::InternalError("Fallback scan returned empty result".to_string())
        })
    }
    
    fn name(&self) -> &str {
        "SimpleRpcFallback"
    }
}

/// Cache-only fallback strategy
pub struct CacheOnlyFallback {
    cache: Arc<dyn crate::utils::cache::CacheTrait>,
}

impl CacheOnlyFallback {
    pub fn new(cache: Arc<dyn crate::utils::cache::CacheTrait>) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl FallbackStrategy for CacheOnlyFallback {
    async fn execute_fallback(&self, wallet_address: &str) -> Result<crate::core::WalletInfo> {
        info!("Executing cache-only fallback for wallet: {}", wallet_address);
        
        // Try to get cached result
        let cache_key = format!("wallet_scan:{}", wallet_address);
        if let Some(cached_data) = self.cache.get(&cache_key).await? {
            // Try to deserialize cached wallet info
            if let Ok(wallet_info) = bincode::deserialize::<crate::core::WalletInfo>(&cached_data) {
                info!("Found cached result for wallet: {}", wallet_address);
                return Ok(wallet_info);
            }
        }
        
        Err(SolanaRecoverError::InternalError(
            "No cached data found for wallet".to_string()
        ))
    }
    
    fn name(&self) -> &str {
        "CacheOnlyFallback"
    }
}

/// Minimal fallback strategy
pub struct MinimalFallback;

impl MinimalFallback {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FallbackStrategy for MinimalFallback {
    async fn execute_fallback(&self, wallet_address: &str) -> Result<crate::core::WalletInfo> {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;
        
        info!("Executing minimal fallback for wallet: {}", wallet_address);
        
        // Very minimal scan - just validate the address and return empty result
        let _pubkey = Pubkey::from_str(wallet_address)
            .map_err(|_| SolanaRecoverError::InvalidWalletAddress(wallet_address.to_string()))?;
        
        // Return minimal wallet info with no accounts found
        let wallet_info = crate::core::WalletInfo {
            address: wallet_address.to_string(),
            pubkey: _pubkey,
            total_accounts: 0,
            empty_accounts: 0,
            recoverable_lamports: 0,
            recoverable_sol: 0.0,
            empty_account_addresses: vec![],
            scan_time_ms: 0,
        };
        
        Ok(wallet_info)
    }
    
    fn name(&self) -> &str {
        "MinimalFallback"
    }
}

/// Resilient scanner with error recovery
pub struct ResilientScanner {
    primary_scanner: Arc<dyn crate::core::unified_scanner::ScanStrategy>,
    circuit_breaker: CircuitBreaker,
    retry_mechanism: RetryMechanism,
    fallback_strategies: Vec<Arc<dyn FallbackStrategy>>,
    // Store context for fallback operations
    connection_pool: Option<Arc<dyn crate::rpc::ConnectionPoolTrait>>,
    config: Option<crate::core::unified_scanner::UnifiedScannerConfig>,
}

impl ResilientScanner {
    pub fn new(
        primary_scanner: Arc<dyn crate::core::unified_scanner::ScanStrategy>,
        circuit_breaker_config: CircuitBreakerConfig,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            primary_scanner,
            circuit_breaker: CircuitBreaker::with_config(circuit_breaker_config),
            retry_mechanism: RetryMechanism::new(retry_policy),
            fallback_strategies: Vec::new(),
            connection_pool: None,
            config: None,
        }
    }
    
    pub fn with_context(
        primary_scanner: Arc<dyn crate::core::unified_scanner::ScanStrategy>,
        circuit_breaker_config: CircuitBreakerConfig,
        retry_policy: RetryPolicy,
        connection_pool: Arc<dyn crate::rpc::ConnectionPoolTrait>,
        config: crate::core::unified_scanner::UnifiedScannerConfig,
    ) -> Self {
        Self {
            primary_scanner,
            circuit_breaker: CircuitBreaker::with_config(circuit_breaker_config),
            retry_mechanism: RetryMechanism::new(retry_policy),
            fallback_strategies: Vec::new(),
            connection_pool: Some(connection_pool),
            config: Some(config),
        }
    }
    
    pub fn add_fallback_strategy(mut self, strategy: Arc<dyn FallbackStrategy>) -> Self {
        self.fallback_strategies.push(strategy);
        self
    }
    
    /// Create a resilient scanner with default fallback strategies
    pub fn with_defaults(
        primary_scanner: Arc<dyn crate::core::unified_scanner::ScanStrategy>,
    ) -> Self {
        let circuit_breaker_config = CircuitBreakerConfig::default();
        let retry_policy = RetryPolicy::default();
        
        let mut scanner = Self::new(primary_scanner, circuit_breaker_config, retry_policy);
        
        // Add default fallback strategies
        scanner = scanner.add_fallback_strategy(Arc::new(SimpleRpcFallback::new(
            "https://api.mainnet-beta.solana.com".to_string()
        )));
        scanner = scanner.add_fallback_strategy(Arc::new(MinimalFallback::new()));
        
        scanner
    }
    
    /// Scan a wallet with full error recovery
    pub async fn scan_wallet(&self, wallet_address: &str) -> Result<crate::core::WalletInfo> {
        // Try primary scanner with circuit breaker and retry
        let result = self.circuit_breaker.execute(|| async {
            self.retry_mechanism.execute(|| async {
                // Create context from stored values or use defaults
                let context = crate::core::unified_scanner::ScanContext {
                    connection_pool: self.connection_pool.clone()
                        .ok_or_else(|| SolanaRecoverError::InternalError(
                            "No connection pool available for resilient scanner".to_string()
                        ))?,
                    config: self.config.clone()
                        .ok_or_else(|| SolanaRecoverError::InternalError(
                            "No config available for resilient scanner".to_string()
                        ))?,
                    cache: None,
                    metrics: None,
                };
                
                let scan_result = self.primary_scanner.scan_wallet(wallet_address, &context).await?;
                scan_result.result.ok_or_else(|| {
                    SolanaRecoverError::InternalError("Scan result is empty".to_string())
                })
            }).await
        }).await;
        
        // If primary scanner fails, try fallback strategies
        if let Err(error) = result {
            warn!("Primary scanner failed for wallet {}: {:?}", wallet_address, error);
            
            for fallback in &self.fallback_strategies {
                info!("Trying fallback strategy: {}", fallback.name());
                match fallback.execute_fallback(wallet_address).await {
                    Ok(result) => {
                        info!("Fallback strategy {} succeeded", fallback.name());
                        return Ok(result);
                    }
                    Err(fallback_error) => {
                        warn!("Fallback strategy {} failed: {:?}", fallback.name(), fallback_error);
                    }
                }
            }
            
            // All strategies failed
            Err(error)
        } else {
            result
        }
    }
    
    /// Get circuit breaker state
    pub async fn circuit_breaker_state(&self) -> CircuitState {
        self.circuit_breaker.get_state().await
    }
    
    /// Force circuit breaker to open
    pub async fn force_circuit_breaker_open(&self) {
        self.circuit_breaker.force_open().await;
    }
    
    /// Force circuit breaker to close
    pub async fn force_circuit_breaker_close(&self) {
        self.circuit_breaker.force_close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, Duration::from_secs(60));
    }
    
    #[test]
    fn test_retry_policy_delay_calculation() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
            retryable_errors: vec!["timeout".to_string()],
        };
        
        let delay1 = policy.calculate_delay(1);
        let delay2 = policy.calculate_delay(2);
        let delay3 = policy.calculate_delay(3);
        
        assert_eq!(delay1, Duration::from_millis(100));
        assert_eq!(delay2, Duration::from_millis(200));
        assert_eq!(delay3, Duration::from_millis(400));
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
    
    #[tokio::test]
    async fn test_retry_mechanism_success() {
        let retry = RetryMechanism::with_default_policy();
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        let result = retry.execute(|| {
            let count = call_count.clone();
            async move {
                let current = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if current < 1 {
                    Err(SolanaRecoverError::InternalError("temporary".to_string()))
                } else {
                    Ok("success")
                }
            }
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
