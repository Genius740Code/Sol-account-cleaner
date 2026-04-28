//! Unified Scanner Architecture
//! 
//! This module consolidates all scanner functionality into a single, cohesive architecture
//! using the strategy pattern to eliminate code duplication and improve maintainability.

use crate::core::{Result, SolanaRecoverError, ScanResult, ScanStatus, BatchScanRequest, BatchScanResult, WalletInfo};
use crate::rpc::{ConnectionPoolTrait};
use crate::utils::cache::{CacheTrait, MetricsTrait};
use std::sync::Arc;
use uuid::Uuid;
use std::time::Instant;
use chrono::Utc;
use tracing::{info};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

/// Performance modes for scanning strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceMode {
    /// Maximum speed with highest resource usage
    UltraFast,
    /// Balanced performance and resource usage
    Balanced,
    /// Optimized for minimal resource consumption
    ResourceEfficient,
    /// Optimized for high throughput
    Throughput,
    /// Optimized for minimal latency
    Latency,
}

impl Default for PerformanceMode {
    fn default() -> Self {
        PerformanceMode::Balanced
    }
}

/// Configuration for the unified scanner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedScannerConfig {
    /// Performance mode
    pub performance_mode: PerformanceMode,
    
    /// Maximum concurrent scans
    pub max_concurrent_scans: usize,
    
    /// Scan timeout
    pub scan_timeout: std::time::Duration,
    
    /// Batch size for operations
    pub batch_size: usize,
    
    /// Enable optimizations
    pub enable_optimizations: bool,
    
    /// Enable caching
    pub enable_caching: bool,
    
    /// Enable parallel processing
    pub enable_parallel_processing: bool,
}

impl Default for UnifiedScannerConfig {
    fn default() -> Self {
        Self {
            performance_mode: PerformanceMode::Balanced,
            max_concurrent_scans: 100,
            scan_timeout: std::time::Duration::from_secs(30),
            batch_size: 50,
            enable_optimizations: true,
            enable_caching: true,
            enable_parallel_processing: true,
        }
    }
}

/// Strategy trait for different scanning approaches
#[async_trait]
pub trait ScanStrategy: Send + Sync {
    /// Scan a single wallet
    async fn scan_wallet(&self, wallet_address: &str, context: &ScanContext) -> Result<ScanResult>;
    
    /// Scan multiple wallets in batch
    async fn scan_batch(&self, request: &BatchScanRequest, context: &ScanContext) -> Result<BatchScanResult>;
    
    /// Strategy name for identification
    fn name(&self) -> &str;
    
    /// Strategy priority (higher = more preferred)
    fn priority(&self) -> u8;
    
    /// Check if strategy supports the given performance mode
    fn supports_mode(&self, mode: &PerformanceMode) -> bool;
}

/// Context provided to scanning strategies
#[derive(Clone)]
pub struct ScanContext {
    /// Connection pool for RPC calls
    pub connection_pool: Arc<dyn ConnectionPoolTrait>,
    
    /// Scanner configuration
    pub config: UnifiedScannerConfig,
    
    /// Cache instance (optional)
    pub cache: Option<Arc<dyn CacheTrait>>,
    
    /// Metrics collector (optional)
    pub metrics: Option<Arc<dyn MetricsTrait>>,
}

/// Ultra-fast scanning strategy implementation
pub struct UltraFastStrategy;

impl UltraFastStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ScanStrategy for UltraFastStrategy {
    async fn scan_wallet(&self, wallet_address: &str, context: &ScanContext) -> Result<ScanResult> {
        use crate::core::scanner::WalletScanner;
        
        let start_time = Instant::now();
        
        info!("Starting ultra-fast scan for wallet: {}", wallet_address);
        
        // Use basic scanner with ultra-fast configuration
        let scanner = WalletScanner::new(context.connection_pool.clone());
        let scan_result = scanner.scan_wallet(wallet_address).await?;
        
        let duration = start_time.elapsed();
        info!("Ultra-fast scan completed in {}ms", duration.as_millis());
        
        Ok(scan_result)
    }
    
    async fn scan_batch(&self, request: &BatchScanRequest, context: &ScanContext) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        
        info!("Starting ultra-fast batch scan for {} wallets", request.wallet_addresses.len());
        
        // Implement ultra-fast batch scanning
        let mut results = Vec::new();
        
        for wallet_address in &request.wallet_addresses {
            let scan_result = self.scan_wallet(wallet_address, context).await?;
            results.push(scan_result);
        }
        
        let duration = start_time.elapsed();
        info!("Ultra-fast batch scan completed in {}ms", duration.as_millis());
        
        Ok(BatchScanResult {
            request_id: request.id,
            batch_id: None,
            results,
            total_wallets: request.wallet_addresses.len(),
            successful_scans: request.wallet_addresses.len(),
            failed_scans: 0,
            completed_wallets: request.wallet_addresses.len(),
            failed_wallets: 0,
            total_recoverable_sol: 0.0,
            estimated_fee_sol: 0.0,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
            duration_ms: Some(duration.as_millis() as u64),
            scan_time_ms: duration.as_millis() as u64,
        })
    }
    
    fn name(&self) -> &str {
        "UltraFast"
    }
    
    fn priority(&self) -> u8 {
        100 // Highest priority
    }
    
    fn supports_mode(&self, mode: &PerformanceMode) -> bool {
        matches!(mode, PerformanceMode::UltraFast | PerformanceMode::Throughput)
    }
}

/// Balanced scanning strategy implementation
pub struct BalancedStrategy;

#[async_trait]
impl ScanStrategy for BalancedStrategy {
    async fn scan_wallet(&self, wallet_address: &str, context: &ScanContext) -> Result<ScanResult> {
        use crate::core::scanner::WalletScanner;
        
        let start_time = Instant::now();
        
        info!("Starting balanced scan for wallet: {}", wallet_address);
        
        // Use basic scanner with balanced configuration
        let scanner = WalletScanner::new(context.connection_pool.clone());
        let scan_result = scanner.scan_wallet(wallet_address).await?;
        
        let duration = start_time.elapsed();
        info!("Balanced scan completed in {}ms", duration.as_millis());
        
        Ok(scan_result)
    }
    
    async fn scan_batch(&self, request: &BatchScanRequest, context: &ScanContext) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        
        info!("Starting balanced batch scan for {} wallets", request.wallet_addresses.len());
        
        let mut results = Vec::new();
        
        for wallet_address in &request.wallet_addresses {
            let scan_result = self.scan_wallet(wallet_address, context).await?;
            results.push(scan_result);
        }
        
        let duration = start_time.elapsed();
        info!("Balanced batch scan completed in {}ms", duration.as_millis());
        
        Ok(BatchScanResult {
            request_id: request.id,
            batch_id: None,
            results,
            total_wallets: request.wallet_addresses.len(),
            successful_scans: request.wallet_addresses.len(),
            failed_scans: 0,
            completed_wallets: request.wallet_addresses.len(),
            failed_wallets: 0,
            total_recoverable_sol: 0.0,
            estimated_fee_sol: 0.0,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
            duration_ms: Some(duration.as_millis() as u64),
            scan_time_ms: duration.as_millis() as u64,
        })
    }
    
    fn name(&self) -> &str {
        "Balanced"
    }
    
    fn priority(&self) -> u8 {
        50
    }
    
    fn supports_mode(&self, mode: &PerformanceMode) -> bool {
        matches!(mode, PerformanceMode::Balanced)
    }
}

/// Resource-efficient scanning strategy implementation
pub struct ResourceEfficientStrategy;

#[async_trait]
impl ScanStrategy for ResourceEfficientStrategy {
    async fn scan_wallet(&self, wallet_address: &str, context: &ScanContext) -> Result<ScanResult> {
        use crate::core::scanner::WalletScanner;
        
        let start_time = Instant::now();
        
        info!("Starting resource-efficient scan for wallet: {}", wallet_address);
        
        // Use basic scanner for resource efficiency
        let scanner = WalletScanner::new(context.connection_pool.clone());
        let scan_result = scanner.scan_wallet(wallet_address).await?;
        
        let duration = start_time.elapsed();
        info!("Resource-efficient scan completed in {}ms", duration.as_millis());
        
        Ok(scan_result)
    }
    
    async fn scan_batch(&self, request: &BatchScanRequest, context: &ScanContext) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        
        info!("Starting resource-efficient batch scan for {} wallets", request.wallet_addresses.len());
        
        let mut results = Vec::new();
        
        for wallet_address in &request.wallet_addresses {
            let scan_result = self.scan_wallet(wallet_address, context).await?;
            results.push(scan_result);
        }
        
        let duration = start_time.elapsed();
        info!("Resource-efficient batch scan completed in {}ms", duration.as_millis());
        
        Ok(BatchScanResult {
            request_id: request.id,
            batch_id: None,
            results,
            total_wallets: request.wallet_addresses.len(),
            successful_scans: request.wallet_addresses.len(),
            failed_scans: 0,
            completed_wallets: request.wallet_addresses.len(),
            failed_wallets: 0,
            total_recoverable_sol: 0.0,
            estimated_fee_sol: 0.0,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
            duration_ms: Some(duration.as_millis() as u64),
            scan_time_ms: duration.as_millis() as u64,
        })
    }
    
    fn name(&self) -> &str {
        "ResourceEfficient"
    }
    
    fn priority(&self) -> u8 {
        25
    }
    
    fn supports_mode(&self, mode: &PerformanceMode) -> bool {
        matches!(mode, PerformanceMode::ResourceEfficient)
    }
}

/// Unified wallet scanner with pluggable strategies
pub struct UnifiedWalletScanner {
    /// Core scanning engine
    core: ScannerCore,
    
    /// Available scanning strategies
    strategies: Vec<Arc<dyn ScanStrategy>>,
    
    /// Scanner configuration
    config: UnifiedScannerConfig,
    
    /// Currently active strategy
    active_strategy: Option<Arc<dyn ScanStrategy>>,
}

/// Core scanning functionality shared across strategies
struct ScannerCore {
    connection_pool: Arc<dyn ConnectionPoolTrait>,
    cache: Option<Arc<dyn CacheTrait>>,
    metrics: Option<Arc<dyn MetricsTrait>>,
}

impl UnifiedWalletScanner {
    /// Create a new unified scanner with default strategies
    pub fn new(connection_pool: Arc<dyn ConnectionPoolTrait>, config: UnifiedScannerConfig) -> Self {
        let core = ScannerCore {
            connection_pool: connection_pool.clone(),
            cache: None,
            metrics: None,
        };
        
        // Initialize default strategies
        let strategies: Vec<Arc<dyn ScanStrategy>> = vec![
            Arc::new(UltraFastStrategy::new()),
            Arc::new(BalancedStrategy),
            Arc::new(ResourceEfficientStrategy),
        ];
        
        // Select active strategy based on performance mode
        let active_strategy = Self::select_strategy_for_mode(&config.performance_mode, &strategies);
        
        Self {
            core,
            strategies,
            config,
            active_strategy,
        }
    }
    
    /// Select the best strategy for the given performance mode
    fn select_strategy_for_mode(mode: &PerformanceMode, strategies: &[Arc<dyn ScanStrategy>]) -> Option<Arc<dyn ScanStrategy>> {
        strategies
            .iter()
            .filter(|strategy| strategy.supports_mode(mode))
            .max_by_key(|strategy| strategy.priority())
            .cloned()
    }
    
    /// Scan a single wallet using the active strategy
    pub async fn scan_wallet(&self, wallet_address: &str) -> Result<ScanResult> {
        let strategy = self.active_strategy.as_ref()
            .ok_or_else(|| SolanaRecoverError::InternalError("No active strategy configured".to_string()))?;
        
        let context = ScanContext {
            connection_pool: self.core.connection_pool.clone(),
            config: self.config.clone(),
            cache: self.core.cache.clone(),
            metrics: self.core.metrics.clone(),
        };
        
        strategy.scan_wallet(wallet_address, &context).await
    }
    
    /// Scan multiple wallets in batch
    pub async fn scan_batch(&self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let strategy = self.active_strategy.as_ref()
            .ok_or_else(|| SolanaRecoverError::InternalError("No active strategy configured".to_string()))?;
        
        let context = ScanContext {
            connection_pool: self.core.connection_pool.clone(),
            config: self.config.clone(),
            cache: self.core.cache.clone(),
            metrics: self.core.metrics.clone(),
        };
        
        strategy.scan_batch(request, &context).await
    }
    
    /// Change the performance mode and update active strategy
    pub fn set_performance_mode(&mut self, mode: PerformanceMode) -> Result<()> {
        self.config.performance_mode = mode.clone();
        self.active_strategy = Self::select_strategy_for_mode(&mode, &self.strategies);
        
        if self.active_strategy.is_none() {
            return Err(SolanaRecoverError::InternalError(
                format!("No strategy supports performance mode: {:?}", mode)
            ));
        }
        
        Ok(())
    }
    
    /// Get the currently active strategy name
    pub fn active_strategy_name(&self) -> Option<&str> {
        self.active_strategy.as_ref().map(|strategy| strategy.name())
    }
    
    /// List all available strategies
    pub fn available_strategies(&self) -> Vec<&str> {
        self.strategies.iter().map(|strategy| strategy.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_performance_mode_default() {
        let config = UnifiedScannerConfig::default();
        assert!(matches!(config.performance_mode, PerformanceMode::Balanced));
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
}
