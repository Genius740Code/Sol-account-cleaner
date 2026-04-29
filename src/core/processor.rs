use crate::core::{BatchScanRequest, BatchScanResult, ScanResult, ScanStatus, Result, FeeStructure, SolanaRecoverError};
use crate::core::adaptive_parallel_processor::AdaptiveParallelProcessor;
use crate::core::processor_metrics::ProcessorMetrics;
use crate::rpc::ConnectionPool;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use chrono::Utc;
use tracing::info;

#[derive(Clone)]
pub struct BatchProcessor {
    scanner: Arc<crate::core::scanner::WalletScanner>,
    #[allow(dead_code)]
    cache_manager: Option<Arc<crate::storage::CacheManager>>,
    #[allow(dead_code)]
    persistence_manager: Option<Arc<dyn crate::storage::PersistenceManager>>,
    max_concurrent_scans: usize,
    #[allow(dead_code)]
    batch_size: usize,
    #[allow(dead_code)]
    retry_attempts: u32,
    #[allow(dead_code)]
    retry_delay_ms: u64,
    intelligent_processor: Option<Arc<AdaptiveParallelProcessor>>,
    config: ProcessorConfig,
}

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub batch_size: usize,
    pub max_concurrent_wallets: usize,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub enable_intelligent_processing: bool,
    pub num_workers: Option<usize>,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_concurrent_wallets: 1000,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            enable_intelligent_processing: true,
            num_workers: None,
        }
    }
}

impl BatchProcessor {
    pub fn new(
        scanner: Arc<crate::core::scanner::WalletScanner>,
        cache_manager: Option<Arc<crate::storage::CacheManager>>,
        persistence_manager: Option<Arc<dyn crate::storage::PersistenceManager>>,
        config: ProcessorConfig,
    ) -> Result<Self> {
        let intelligent_processor = if config.enable_intelligent_processing {
            let processor_config = crate::core::adaptive_parallel_processor::ProcessorConfig {
                max_workers: config.num_workers.unwrap_or(4),
                max_concurrent_tasks: config.max_concurrent_wallets,
                work_stealing_enabled: true,
                cpu_affinity_enabled: false,
                adaptive_batching: true,
                resource_monitoring: true,
                load_balancing_strategy: crate::core::adaptive_parallel_processor::LoadBalancingStrategy::WorkStealing,
                task_timeout: std::time::Duration::from_secs(30),
                worker_idle_timeout: std::time::Duration::from_secs(60),
            };
            Some(Arc::new(AdaptiveParallelProcessor::new(
                scanner.clone(),
                processor_config,
            )?))
        } else {
            None
        };
        
        Ok(Self {
            scanner,
            cache_manager,
            persistence_manager,
            max_concurrent_scans: config.max_concurrent_wallets,
            batch_size: config.batch_size,
            retry_attempts: config.retry_attempts,
            retry_delay_ms: config.retry_delay_ms,
            intelligent_processor,
            config,
        })
    }

    pub fn new_simple(connection_pool: Arc<ConnectionPool>, max_concurrent_scans: usize) -> Self {
        let scanner = Arc::new(crate::core::scanner::WalletScanner::new(connection_pool));
        let config = ProcessorConfig::default();
        Self {
            scanner,
            cache_manager: None,
            persistence_manager: None,
            max_concurrent_scans,
            batch_size: 100,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            intelligent_processor: None,
            config,
        }
    }

    pub async fn process_batch(&self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        
        // Use intelligent processor if available, otherwise fall back to legacy processing
        if let Some(_processor) = &self.intelligent_processor {
            info!("Using intelligent parallel processor for batch of {} wallets", request.wallet_addresses.len());
            // Note: This requires a mutable processor, so we need to handle this differently
            // For now, we'll clone the processor or use a different approach
            // Use adaptive parallel processor with default config
            let processor_config = crate::core::adaptive_parallel_processor::ProcessorConfig {
                max_workers: 4,
                max_concurrent_tasks: self.config.max_concurrent_wallets,
                work_stealing_enabled: true,
                cpu_affinity_enabled: false,
                adaptive_batching: true,
                resource_monitoring: true,
                load_balancing_strategy: crate::core::adaptive_parallel_processor::LoadBalancingStrategy::WorkStealing,
                task_timeout: std::time::Duration::from_secs(30),
                worker_idle_timeout: std::time::Duration::from_secs(60),
            };
            let processor_clone = AdaptiveParallelProcessor::new(
                self.scanner.clone(),
                processor_config,
            ).map_err(|e| SolanaRecoverError::InternalError(format!("Failed to create processor clone: {}", e)))?;
            
            let batch_result = processor_clone.process_batch_adaptive(request).await?;
            return Ok(batch_result);
        } else {
            // Legacy processing with work-stealing
            let scanner = self.scanner.clone();
            let chunk_size = (request.wallet_addresses.len() / rayon::current_num_threads()).max(1);
            
            let results: Vec<ScanResult> = request.wallet_addresses
                .par_chunks(chunk_size)
                .map(|chunk| {
                    let runtime = tokio::runtime::Handle::current();
                    runtime.block_on(async {
                        let mut chunk_results = Vec::with_capacity(chunk.len());
                        
                        // Process with controlled concurrency
                        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent_scans));
                        let mut tasks = Vec::new();
                        
                        for wallet_address in chunk {
                            let scanner = scanner.clone();
                            let semaphore = semaphore.clone();
                            let wallet_addr = wallet_address.clone();
                            
                            let task = tokio::spawn(async move {
                                let _permit = semaphore.acquire().await.unwrap();
                                scanner.scan_wallet(&wallet_addr).await
                            });
                            tasks.push(task);
                        }
                        
                        // Wait for all tasks in this chunk
                        for task in tasks {
                            match task.await.unwrap() {
                                Ok(result) => chunk_results.push(result),
                                Err(e) => {
                                    // Create error result
                                    chunk_results.push(ScanResult {
                                        id: Uuid::new_v4(),
                                        wallet_address: "unknown".to_string(), // Will be set properly
                                        status: ScanStatus::Failed,
                                        result: None,
                                        empty_accounts_found: 0,
                                        recoverable_sol: 0.0,
                                        scan_time_ms: 0,
                                        created_at: Utc::now(),
                                        completed_at: Some(Utc::now()),
                                        error_message: Some(e.to_string()),
                                    });
                                }
                            }
                        }
                        
                        chunk_results
                    })
                })
                .flatten()
                .collect();

            let completed_wallets = results.iter()
                .filter(|r| r.status == ScanStatus::Completed)
                .count();

            let failed_wallets = results.iter()
                .filter(|r| r.status == ScanStatus::Failed)
                .count();

            let total_recoverable_sol: f64 = results.iter()
                .filter_map(|r| r.result.as_ref())
                .map(|w| w.recoverable_sol)
                .sum();

            let fee_structure = request.fee_percentage
                .map(|p| FeeStructure { percentage: p, ..Default::default() })
                .unwrap_or_default();

            let estimated_fee_sol = self.calculate_fee(total_recoverable_sol, &fee_structure);
            let duration_ms = start_time.elapsed().as_millis() as u64;

            return Ok(BatchScanResult {
                request_id: request.id,
                batch_id: Some(request.id.to_string()),
                total_wallets: request.wallet_addresses.len(),
                successful_scans: completed_wallets,
                failed_scans: failed_wallets,
                completed_wallets,
                failed_wallets,
                total_recoverable_sol,
                estimated_fee_sol,
                results,
                created_at: request.created_at,
                completed_at: Some(Utc::now()),
                duration_ms: Some(duration_ms),
                scan_time_ms: duration_ms,
            });
        }
    }

    pub async fn process_batch_streaming(&self, request: &BatchScanRequest) -> Result<tokio::sync::mpsc::UnboundedReceiver<ScanResult>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let scanner = self.scanner.clone();
        let wallet_addresses = request.wallet_addresses.clone();
        let max_concurrent = self.max_concurrent_scans;

        tokio::spawn(async move {
            let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
            let mut tasks = Vec::new();
            
            for wallet_address in wallet_addresses {
                let scanner = scanner.clone();
                let semaphore = semaphore.clone();
                let tx = tx.clone();
                let wallet_addr = wallet_address.clone();
                
                let task = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    
                    match scanner.scan_wallet(&wallet_addr).await {
                        Ok(result) => {
                            if tx.send(result).is_err() {
                                return; // Receiver dropped
                            }
                        }
                        Err(e) => {
                            let error_result = ScanResult {
                                id: Uuid::new_v4(),
                                wallet_address: wallet_addr,
                                status: ScanStatus::Failed,
                                result: None,
                                empty_accounts_found: 0,
                                recoverable_sol: 0.0,
                                scan_time_ms: 0,
                                created_at: Utc::now(),
                                completed_at: Some(Utc::now()),
                                error_message: Some(e.to_string()),
                            };
                            let _ = tx.send(error_result); // Ignore send errors
                        }
                    }
                });
                
                tasks.push(task);
            }
            
            // Wait for all tasks to complete
            for task in tasks {
                let _ = task.await;
            }
            
            drop(tx); // Close the channel
        });
        
        Ok(rx)
    }
}

impl BatchProcessor {
    /// Calculate fee based on recovered amount and fee structure
    fn calculate_fee(&self, total_recoverable_sol: f64, fee_structure: &FeeStructure) -> f64 {
        total_recoverable_sol * fee_structure.percentage / 100.0
    }
    
    /// Get resource metrics from intelligent processor
    pub async fn get_resource_metrics(&self) -> Option<crate::core::adaptive_parallel_processor::ProcessorMetrics> {
        if let Some(processor) = &self.intelligent_processor {
            Some(processor.get_metrics().await)
        } else {
            None
        }
    }
    
    /// Get active batches count
    pub async fn get_active_batches(&self) -> usize {
        0 // Placeholder implementation
    }
    
    /// Get current processing metrics
    pub async fn get_metrics(&self) -> ProcessorMetrics {
        ProcessorMetrics {
            active_scans: 0,
            completed_scans: 0,
            failed_scans: 0,
            average_scan_time_ms: 0.0,
            total_recovered_sol: 0.0,
            cache_hit_rate: 0.0,
            connection_pool_health: 100.0,
            total_wallets_processed: 0,
            throughput_wallets_per_second: 0.0,
        }
    }
}
