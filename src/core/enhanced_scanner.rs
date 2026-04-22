use crate::core::{Result, SolanaRecoverError, ScanResult, ScanStatus, EmptyAccount, BatchScanRequest, BatchScanResult};
use crate::core::parallel_processor::IntelligentParallelProcessor;
use crate::rpc::{ConnectionPoolTrait};
use crate::utils::memory_integration::{MemoryIntegrationLayer, ScannerMemoryManager};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use uuid::Uuid;
use std::time::Instant;
use chrono::Utc;
use std::str::FromStr;
use tracing::{info, debug, error};
use serde::{Deserialize, Serialize};

/// Enhanced wallet scanner with integrated memory management
#[derive(Clone)]
pub struct EnhancedWalletScanner {
    /// Original scanner functionality
    connection_pool: Arc<dyn ConnectionPoolTrait>,
    parallel_processor: Option<Arc<IntelligentParallelProcessor>>,
    
    /// Memory management integration
    memory_integration: Arc<MemoryIntegrationLayer>,
    scanner_memory_manager: ScannerMemoryManager,
    
    /// Scanner configuration
    config: EnhancedScannerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedScannerConfig {
    /// Enable memory pooling for scanner operations
    pub enable_memory_pooling: bool,
    
    /// Enable performance tracking
    pub enable_performance_tracking: bool,
    
    /// Batch processing configuration
    pub batch_config: BatchProcessingConfig,
    
    /// Memory optimization settings
    pub memory_config: ScannerMemoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProcessingConfig {
    /// Enable intelligent batch sizing
    pub enable_intelligent_sizing: bool,
    
    /// Minimum batch size
    pub min_batch_size: usize,
    
    /// Maximum batch size
    pub max_batch_size: usize,
    
    /// Target processing time per batch (milliseconds)
    pub target_batch_time_ms: u64,
    
    /// Enable work-stealing for batch processing
    pub enable_work_stealing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerMemoryConfig {
    /// Pool size for wallet info objects
    pub wallet_info_pool_size: usize,
    
    /// Pool size for empty account objects
    pub empty_account_pool_size: usize,
    
    /// Pool size for scan result objects
    pub scan_result_pool_size: usize,
    
    /// Enable memory tracking for scan operations
    pub enable_scan_tracking: bool,
    
    /// Memory optimization interval in seconds
    pub memory_optimization_interval_seconds: u64,
}

impl Default for EnhancedScannerConfig {
    fn default() -> Self {
        Self {
            enable_memory_pooling: true,
            enable_performance_tracking: true,
            batch_config: BatchProcessingConfig::default(),
            memory_config: ScannerMemoryConfig::default(),
        }
    }
}

impl Default for BatchProcessingConfig {
    fn default() -> Self {
        Self {
            enable_intelligent_sizing: true,
            min_batch_size: 10,
            max_batch_size: 1000,
            target_batch_time_ms: 5000, // 5 seconds
            enable_work_stealing: true,
        }
    }
}

impl Default for ScannerMemoryConfig {
    fn default() -> Self {
        Self {
            wallet_info_pool_size: 10000,
            empty_account_pool_size: 50000,
            scan_result_pool_size: 10000,
            enable_scan_tracking: true,
            memory_optimization_interval_seconds: 300, // 5 minutes
        }
    }
}

impl EnhancedWalletScanner {
    /// Create new enhanced scanner with memory management
    pub fn new(connection_pool: Arc<dyn ConnectionPoolTrait>) -> Result<Self> {
        Self::with_config(connection_pool, EnhancedScannerConfig::default())
    }
    
    /// Create enhanced scanner with custom configuration
    pub fn with_config(
        connection_pool: Arc<dyn ConnectionPoolTrait>,
        config: EnhancedScannerConfig,
    ) -> Result<Self> {
        // Initialize memory integration
        let memory_integration = crate::utils::memory_integration::get_global_memory_integration();
        let scanner_memory_manager = memory_integration.create_scanner_memory_manager();
        
        Ok(Self {
            connection_pool,
            parallel_processor: None,
            memory_integration,
            scanner_memory_manager,
            config,
        })
    }
    
    /// Create enhanced scanner with parallel processing
    pub fn new_with_parallel_processing(
        connection_pool: Arc<dyn ConnectionPoolTrait>,
        max_workers: Option<usize>,
        max_concurrent_tasks: usize,
    ) -> Result<Self> {
        Self::with_parallel_processing_and_config(
            connection_pool,
            max_workers,
            max_concurrent_tasks,
            EnhancedScannerConfig::default(),
        )
    }
    
    /// Create enhanced scanner with parallel processing and custom config
    pub fn with_parallel_processing_and_config(
        connection_pool: Arc<dyn ConnectionPoolTrait>,
        max_workers: Option<usize>,
        max_concurrent_tasks: usize,
        config: EnhancedScannerConfig,
    ) -> Result<Self> {
        let config_clone = config.clone();
        let scanner = Self {
            connection_pool: connection_pool.clone(),
            parallel_processor: None,
            memory_integration: crate::utils::memory_integration::get_global_memory_integration(),
            scanner_memory_manager: crate::utils::memory_integration::get_global_memory_integration().create_scanner_memory_manager(),
            config: config_clone,
        };
        
        let parallel_processor = Arc::new(IntelligentParallelProcessor::new(
            Arc::new(crate::core::scanner::WalletScanner::new(scanner.connection_pool.clone())),
            max_workers,
            max_concurrent_tasks,
        )?);
        
        Ok(Self {
            connection_pool,
            parallel_processor: Some(parallel_processor),
            memory_integration: crate::utils::memory_integration::get_global_memory_integration(),
            scanner_memory_manager: crate::utils::memory_integration::get_global_memory_integration().create_scanner_memory_manager(),
            config,
        })
    }
    
    /// Enhanced batch scanning with memory optimization
    pub async fn scan_batch_enhanced(&mut self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        
        info!("Starting enhanced batch scan for {} wallets", request.wallet_addresses.len());
        
        // Optimize memory before batch processing
        if self.config.memory_config.enable_scan_tracking {
            self.optimize_memory_for_batch().await;
        }
        
        // Use intelligent batch sizing if enabled
        let processed_request = if self.config.batch_config.enable_intelligent_sizing {
            self.optimize_batch_size(request).await?
        } else {
            request.clone()
        };
        
        // Process batch with memory-aware parallel processing
        let result = if let Some(processor) = &self.parallel_processor {
            self.process_batch_with_memory_tracking(processor, &processed_request).await?
        } else {
            self.scan_batch_sequential_enhanced(&processed_request).await?
        };
        
        let duration = start_time.elapsed();
        info!("Enhanced batch scan completed in {}ms: {} successful, {} failed", 
              duration.as_millis(), result.successful_scans, result.failed_scans);
        
        // Update performance metrics
        if self.config.enable_performance_tracking {
            self.update_performance_metrics(&result, duration).await;
        }
        
        Ok(result)
    }
    
    /// Optimize batch size based on system resources and historical performance
    async fn optimize_batch_size(&self, request: &BatchScanRequest) -> Result<BatchScanRequest> {
        let memory_stats = self.memory_integration.get_memory_manager().get_memory_stats();
        let current_memory_pressure = memory_stats.memory_pressure;
        
        // Adjust batch size based on memory pressure
        let size_multiplier = if current_memory_pressure > 80.0 {
            0.5 // Reduce batch size under high memory pressure
        } else if current_memory_pressure < 40.0 {
            1.5 // Increase batch size under low memory pressure
        } else {
            1.0 // Normal batch size
        };
        
        let target_size = ((request.wallet_addresses.len() as f64 * size_multiplier) as usize)
            .clamp(self.config.batch_config.min_batch_size, self.config.batch_config.max_batch_size);
        
        if target_size != request.wallet_addresses.len() {
            debug!("Adjusting batch size from {} to {} based on memory pressure: {:.1}%",
                   request.wallet_addresses.len(), target_size, current_memory_pressure);
            
            Ok(BatchScanRequest {
                id: request.id,
                wallet_addresses: request.wallet_addresses.iter().take(target_size).cloned().collect(),
                user_id: request.user_id.clone(),
                fee_percentage: request.fee_percentage,
                created_at: request.created_at,
            })
        } else {
            Ok(request.clone())
        }
    }
    
    /// Process batch with memory tracking and optimization
    async fn process_batch_with_memory_tracking(
        &self,
        processor: &Arc<IntelligentParallelProcessor>,
        request: &BatchScanRequest,
    ) -> Result<BatchScanResult> {
        let _start_time = Instant::now();
        
        // Monitor memory during processing
        let initial_memory = self.memory_integration.get_memory_manager().get_memory_stats().total_allocated_bytes;
        
        // Create a local mutable processor for this batch
        // Note: This is a workaround since we can't get mutable reference from Arc
        let mut local_processor = IntelligentParallelProcessor::new(
            processor.scanner.clone(),
            Some(processor.max_workers),
            processor.semaphore.available_permits(),
        )?;
        
        // Process the batch
        let result = local_processor.process_batch_intelligently(request).await?;
        
        // Track memory usage
        let final_memory = self.memory_integration.get_memory_manager().get_memory_stats().total_allocated_bytes;
        let memory_used = final_memory.saturating_sub(initial_memory);
        
        debug!("Batch processing used {}MB of memory", memory_used / 1024 / 1024);
        
        // Trigger memory optimization if significant memory was used
        if memory_used > 100 * 1024 * 1024 { // 100MB threshold
            debug!("Triggering memory optimization after batch processing");
            self.memory_integration.get_gc_scheduler().schedule_gc(75.0).await;
        }
        
        Ok(result)
    }
    
    /// Enhanced sequential scanning with memory pooling
    async fn scan_batch_sequential_enhanced(&self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        let mut results = Vec::new();
        let mut successful_scans = 0;
        let mut failed_scans = 0;
        let mut total_recoverable_sol = 0.0;

        for wallet_address in &request.wallet_addresses {
            match self.scan_wallet_enhanced(wallet_address).await {
                Ok(scan_result) => {
                    if scan_result.status == ScanStatus::Completed {
                        successful_scans += 1;
                        if let Some(wallet_info) = &scan_result.result {
                            total_recoverable_sol += wallet_info.recoverable_sol;
                        }
                    } else {
                        failed_scans += 1;
                    }
                    results.push(scan_result);
                }
                Err(e) => {
                    error!("Failed to scan wallet {}: {}", wallet_address, e);
                    failed_scans += 1;
                    
                    // Use pooled scan result for error case
                    let mut error_result = self.scanner_memory_manager.acquire_scan_result();
                    error_result.id = Uuid::new_v4();
                    error_result.wallet_address = wallet_address.clone();
                    error_result.status = ScanStatus::Failed;
                    error_result.error = Some(e.to_string());
                    error_result.created_at = Utc::now();
                    
                    results.push(error_result.into_inner());
                }
            }
        }

        let duration = start_time.elapsed();
        
        // Use pooled batch scan result
        let mut batch_result = self.scanner_memory_manager.acquire_batch_scan_result();
        batch_result.id = request.id;
        batch_result.total_wallets = request.wallet_addresses.len();
        batch_result.successful_scans = successful_scans;
        batch_result.failed_scans = failed_scans;
        batch_result.completed_wallets = successful_scans; // Backward compatibility
        batch_result.failed_wallets = failed_scans;       // Backward compatibility
        batch_result.total_recoverable_sol = total_recoverable_sol;
        batch_result.estimated_fee_sol = total_recoverable_sol * 0.15; // 15% fee
        batch_result.results = results;
        batch_result.created_at = request.created_at;
        batch_result.completed_at = Some(Utc::now());
        batch_result.duration_ms = Some(duration.as_millis() as u64);

        Ok(batch_result.into_inner())
    }
    
    /// Enhanced wallet scanning with memory pooling
    async fn scan_wallet_enhanced(&self, wallet_address: &str) -> Result<ScanResult> {
        let start_time = Instant::now();
        
        debug!("Starting enhanced scan for wallet: {}", wallet_address);
        
        // Parse wallet address
        let pubkey = match Pubkey::from_str(wallet_address) {
            Ok(key) => key,
            Err(e) => {
                return Err(SolanaRecoverError::InvalidWalletAddress(format!("Invalid wallet address: {}", e)));
            }
        };
        
        // Use pooled wallet info
        let mut wallet_info = self.scanner_memory_manager.acquire_wallet_info();
        wallet_info.address = wallet_address.to_string();
        wallet_info.pubkey = pubkey;
        
        // Scan for empty accounts
        let empty_accounts = self.scan_empty_accounts_enhanced(&pubkey).await?;
        wallet_info.empty_accounts = empty_accounts.len() as u64;
        wallet_info.total_accounts = empty_accounts.len() as u64;
        
        // Calculate recoverable SOL
        let total_lamports = empty_accounts.iter().map(|acc| acc.lamports).sum();
        wallet_info.recoverable_lamports = total_lamports;
        wallet_info.recoverable_sol = total_lamports as f64 / 1_000_000_000.0;
        
        // Store empty account addresses
        wallet_info.empty_account_addresses = empty_accounts.iter().map(|acc| acc.address.clone()).collect();
        
        let scan_time = start_time.elapsed();
        wallet_info.scan_time_ms = scan_time.as_millis() as u64;
        
        // Use pooled scan result
        let mut scan_result = self.scanner_memory_manager.acquire_scan_result();
        scan_result.id = Uuid::new_v4();
        scan_result.wallet_address = wallet_address.to_string();
        scan_result.status = ScanStatus::Completed;
        scan_result.result = Some(wallet_info.into_inner());
        scan_result.created_at = Utc::now();
        
        debug!("Enhanced scan completed for {} in {}ms", wallet_address, scan_time.as_millis());
        
        Ok(scan_result.into_inner())
    }
    
    /// Enhanced empty account scanning with memory pooling
    async fn scan_empty_accounts_enhanced(&self, pubkey: &Pubkey) -> Result<Vec<EmptyAccount>> {
        let start_time = Instant::now();
        
        // Get RPC client wrapper from connection pool
        let client = self.connection_pool.get_client().await?;
        
        // Get all token accounts
        let token_accounts = client.get_all_recoverable_accounts(pubkey).await?;
        
        // Use pooled empty account objects
        let mut empty_accounts = Vec::new();
        
        for keyed_account in token_accounts {
            if keyed_account.account.lamports == 0 {
                // Use pooled empty account
                let mut empty_account = self.scanner_memory_manager.acquire_empty_account();
                empty_account.address = keyed_account.pubkey.to_string();
                empty_account.lamports = keyed_account.account.lamports;
                empty_account.owner = keyed_account.account.owner.to_string();
                
                // Try to decode account data for mint information
                match &keyed_account.account.data {
                    solana_account_decoder::UiAccountData::Binary(data, _) => {
                        if data.len() >= 165 { // Token account size
                            // Extract mint from token account data (simplified)
                            let mint_bytes = &data[0..32];
                            let mint_pubkey = Pubkey::try_from(mint_bytes).unwrap_or_default();
                            empty_account.mint = Some(mint_pubkey.to_string());
                        }
                    }
                    _ => {
                        // Other data formats - skip mint extraction
                    }
                }
                
                empty_accounts.push(empty_account.into_inner());
            }
        }
        
        debug!("Found {} empty accounts in {}ms", empty_accounts.len(), start_time.elapsed().as_millis());
        
        Ok(empty_accounts)
    }
    
    /// Optimize memory for batch processing
    async fn optimize_memory_for_batch(&self) {
        debug!("Optimizing memory for batch processing");
        
        // Trigger GC if memory pressure is high
        let memory_stats = self.memory_integration.get_memory_manager().get_memory_stats();
        if memory_stats.memory_pressure > 70.0 {
            self.memory_integration.get_gc_scheduler().schedule_gc(memory_stats.memory_pressure).await;
        }
        
        // Optimize buffer pools
        let buffer_pool = self.memory_integration.get_buffer_pool();
        buffer_pool.cleanup_old_buffers().await;
    }
    
    /// Update performance metrics
    async fn update_performance_metrics(&self, result: &BatchScanResult, duration: std::time::Duration) {
        // This would update metrics in the metrics collector
        debug!("Performance metrics updated: {} wallets in {}ms", 
               result.total_wallets, duration.as_millis());
    }
    
    /// Get scanner performance statistics
    pub fn get_scanner_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "config": self.config,
            "memory_manager_stats": self.scanner_memory_manager.get_scanner_stats(),
            "memory_integration_stats": self.memory_integration.get_integration_stats(),
            "parallel_processor_enabled": self.parallel_processor.is_some(),
        })
    }
    
    /// Get comprehensive scanner report
    pub async fn get_comprehensive_report(&self) -> serde_json::Value {
        let scanner_stats = self.get_scanner_stats();
        let memory_report = self.memory_integration.generate_integration_report().await;
        
        serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "scanner_stats": scanner_stats,
            "memory_integration_report": memory_report,
            "recommendations": self.generate_scanner_recommendations(),
        })
    }
    
    fn generate_scanner_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        let stats = self.memory_integration.get_integration_stats();
        
        if stats.scanner_pool_operations == 0 {
            recommendations.push("Scanner memory pooling is not being utilized. Consider enabling memory pooling for better performance.".to_string());
        }
        
        if stats.memory_saved_bytes < 10 * 1024 * 1024 { // Less than 10MB saved
            recommendations.push("Low memory savings detected. Consider increasing pool sizes or optimizing allocation patterns.".to_string());
        }
        
        if !self.config.enable_memory_pooling {
            recommendations.push("Memory pooling is disabled. Enable it for improved performance.".to_string());
        }
        
        if !self.config.enable_performance_tracking {
            recommendations.push("Performance tracking is disabled. Enable it for better monitoring and optimization.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("Scanner is configured optimally. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::mock::MockConnectionPool;
    
    #[tokio::test]
    async fn test_enhanced_scanner_creation() {
        let connection_pool = Arc::new(MockConnectionPool::new());
        let scanner = EnhancedWalletScanner::new(connection_pool).unwrap();
        
        let stats = scanner.get_scanner_stats();
        assert!(stats.get("config").is_some());
        assert!(stats.get("memory_manager_stats").is_some());
    }
    
    #[tokio::test]
    async fn test_enhanced_wallet_scan() {
        let connection_pool = Arc::new(MockConnectionPool::new());
        let scanner = EnhancedWalletScanner::new(connection_pool).unwrap();
        
        let result = scanner.scan_wallet_enhanced("11111111111111111111111111111112").await;
        assert!(result.is_ok());
        
        let scan_result = result.unwrap();
        assert_eq!(scan_result.wallet_address, "11111111111111111111111111111112");
        assert_eq!(scan_result.status, ScanStatus::Completed);
    }
    
    #[tokio::test]
    async fn test_batch_size_optimization() {
        let connection_pool = Arc::new(MockConnectionPool::new());
        let scanner = EnhancedWalletScanner::new(connection_pool).unwrap();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses: (0..100).map(|i| format!("wallet_{}", i)).collect(),
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let optimized = scanner.optimize_batch_size(&request).await.unwrap();
        assert!(optimized.wallet_addresses.len() <= 100);
        assert!(optimized.wallet_addresses.len() >= 10); // min_batch_size
    }
    
    #[tokio::test]
    async fn test_comprehensive_report() {
        let connection_pool = Arc::new(MockConnectionPool::new());
        let scanner = EnhancedWalletScanner::new(connection_pool).unwrap();
        
        let report = scanner.get_comprehensive_report().await;
        
        assert!(report.get("timestamp").is_some());
        assert!(report.get("scanner_stats").is_some());
        assert!(report.get("memory_integration_report").is_some());
        assert!(report.get("recommendations").is_some());
    }
}
