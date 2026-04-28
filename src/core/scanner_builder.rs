//! Scanner Builder with Dependency Injection
//! 
//! This module provides a builder pattern for constructing scanners with proper
//! dependency injection, eliminating tight coupling between components.

use crate::core::{Result, SolanaRecoverError};
use crate::core::unified_scanner::{UnifiedWalletScanner, UnifiedScannerConfig, PerformanceMode};
use crate::rpc::{ConnectionPoolTrait};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Trait abstractions for dependency injection
#[async_trait::async_trait]
pub trait CacheTrait: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<std::time::Duration>) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
}

pub trait MetricsTrait: Send + Sync {
    fn increment_counter(&self, name: &str, tags: &[(&str, &str)]);
    fn record_histogram(&self, name: &str, value: f64, tags: &[(&str, &str)]);
    fn set_gauge(&self, name: &str, value: f64, tags: &[(&str, &str)]);
}

#[async_trait::async_trait]
pub trait RateLimiterTrait: Send + Sync {
    async fn check_rate_limit(&self, key: &str, limit: u32, window: std::time::Duration) -> Result<bool>;
}

/// Scanner builder with dependency injection
pub struct ScannerBuilder {
    connection_pool: Option<Arc<dyn ConnectionPoolTrait>>,
    cache: Option<Arc<dyn CacheTrait>>,
    metrics: Option<Arc<dyn MetricsTrait>>,
    rate_limiter: Option<Arc<dyn RateLimiterTrait>>,
    config: Option<UnifiedScannerConfig>,
}

impl Default for ScannerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ScannerBuilder {
    /// Create a new scanner builder
    pub fn new() -> Self {
        Self {
            connection_pool: None,
            cache: None,
            metrics: None,
            rate_limiter: None,
            config: None,
        }
    }
    
    /// Set the connection pool
    pub fn with_connection_pool(mut self, pool: Arc<dyn ConnectionPoolTrait>) -> Self {
        self.connection_pool = Some(pool);
        self
    }
    
    /// Set the cache implementation
    pub fn with_cache(mut self, cache: Arc<dyn CacheTrait>) -> Self {
        self.cache = Some(cache);
        self
    }
    
    /// Set the metrics implementation
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsTrait>) -> Self {
        self.metrics = Some(metrics);
        self
    }
    
    /// Set the rate limiter
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<dyn RateLimiterTrait>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }
    
    /// Set the scanner configuration
    pub fn with_config(mut self, config: UnifiedScannerConfig) -> Self {
        self.config = Some(config);
        self
    }
    
    /// Set performance mode (convenience method)
    pub fn with_performance_mode(mut self, mode: PerformanceMode) -> Self {
        let mut config = self.config.unwrap_or_default();
        config.performance_mode = mode;
        self.config = Some(config);
        self
    }
    
    /// Build the unified scanner
    pub fn build(self) -> Result<UnifiedWalletScanner> {
        let connection_pool = self.connection_pool.ok_or_else(|| {
            SolanaRecoverError::InternalError("Connection pool is required".to_string())
        })?;
        
        let config = self.config.unwrap_or_default();
        
        let scanner = UnifiedWalletScanner::new(connection_pool, config);
        
        // Additional dependencies can be injected here
        // This would require modifying the UnifiedWalletScanner to accept them
        
        Ok(scanner)
    }
    
    /// Build scanner with ultra-fast configuration
    pub fn build_ultra_fast(self) -> Result<UnifiedWalletScanner> {
        self.with_performance_mode(PerformanceMode::UltraFast).build()
    }
    
    /// Build scanner with balanced configuration
    pub fn build_balanced(self) -> Result<UnifiedWalletScanner> {
        self.with_performance_mode(PerformanceMode::Balanced).build()
    }
    
    /// Build scanner with resource-efficient configuration
    pub fn build_resource_efficient(self) -> Result<UnifiedWalletScanner> {
        self.with_performance_mode(PerformanceMode::ResourceEfficient).build()
    }
}

/// Dependency injection container for managing scanner components
pub struct ScannerContainer {
    connection_pool: Arc<dyn ConnectionPoolTrait>,
    cache: Option<Arc<dyn CacheTrait>>,
    metrics: Option<Arc<dyn MetricsTrait>>,
    rate_limiter: Option<Arc<dyn RateLimiterTrait>>,
}

impl ScannerContainer {
    /// Create a new container with required dependencies
    pub fn new(connection_pool: Arc<dyn ConnectionPoolTrait>) -> Self {
        Self {
            connection_pool,
            cache: None,
            metrics: None,
            rate_limiter: None,
        }
    }
    
    /// Set optional dependencies
    pub fn with_cache(mut self, cache: Arc<dyn CacheTrait>) -> Self {
        self.cache = Some(cache);
        self
    }
    
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsTrait>) -> Self {
        self.metrics = Some(metrics);
        self
    }
    
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<dyn RateLimiterTrait>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }
    
    /// Create a builder using this container's dependencies
    pub fn builder(&self) -> ScannerBuilder {
        let builder = ScannerBuilder::new()
            .with_connection_pool(self.connection_pool.clone());
        
        let builder = if let Some(cache) = &self.cache {
            builder.with_cache(cache.clone())
        } else {
            builder
        };
        
        let builder = if let Some(metrics) = &self.metrics {
            builder.with_metrics(metrics.clone())
        } else {
            builder
        };
        
        let builder = if let Some(rate_limiter) = &self.rate_limiter {
            builder.with_rate_limiter(rate_limiter.clone())
        } else {
            builder
        };
        
        builder
    }
}

/// Factory for creating pre-configured scanners
pub struct ScannerFactory;

impl ScannerFactory {
    /// Create an ultra-fast scanner with default optimizations
    pub fn create_ultra_fast(connection_pool: Arc<dyn ConnectionPoolTrait>) -> Result<UnifiedWalletScanner> {
        ScannerBuilder::new()
            .with_connection_pool(connection_pool)
            .with_performance_mode(PerformanceMode::UltraFast)
            .build()
    }
    
    /// Create a balanced scanner for general use
    pub fn create_balanced(connection_pool: Arc<dyn ConnectionPoolTrait>) -> Result<UnifiedWalletScanner> {
        ScannerBuilder::new()
            .with_connection_pool(connection_pool)
            .with_performance_mode(PerformanceMode::Balanced)
            .build()
    }
    
    /// Create a resource-efficient scanner for low-resource environments
    pub fn create_resource_efficient(connection_pool: Arc<dyn ConnectionPoolTrait>) -> Result<UnifiedWalletScanner> {
        ScannerBuilder::new()
            .with_connection_pool(connection_pool)
            .with_performance_mode(PerformanceMode::ResourceEfficient)
            .build()
    }
    
    /// Create a scanner with custom configuration
    pub fn create_with_config(
        connection_pool: Arc<dyn ConnectionPoolTrait>,
        config: UnifiedScannerConfig,
    ) -> Result<UnifiedWalletScanner> {
        ScannerBuilder::new()
            .with_connection_pool(connection_pool)
            .with_config(config)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Mock implementations for testing
    struct MockConnectionPool;
    
    impl ConnectionPoolTrait for MockConnectionPool {
        // Implement required methods...
    }
    
    struct MockCache;
    
    impl CacheTrait for MockCache {
        async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>> {
            Ok(None)
        }
        
        async fn set(&self, _key: &str, _value: Vec<u8>, _ttl: Option<std::time::Duration>) -> Result<()> {
            Ok(())
        }
        
        async fn delete(&self, _key: &str) -> Result<()> {
            Ok(())
        }
    }
    
    #[test]
    fn test_scanner_builder_requires_connection_pool() {
        let result = ScannerBuilder::new().build();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_scanner_builder_with_connection_pool() {
        let connection_pool = Arc::new(MockConnectionPool);
        let result = ScannerBuilder::new()
            .with_connection_pool(connection_pool)
            .build();
        
        // Should succeed (assuming UnifiedWalletScanner construction works)
        assert!(result.is_ok());
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
        let cache = Arc::new(MockCache);
        
        let container = ScannerContainer::new(connection_pool)
            .with_cache(cache);
        
        let builder = container.builder();
        let result = builder.build();
        
        assert!(result.is_ok());
    }
}
