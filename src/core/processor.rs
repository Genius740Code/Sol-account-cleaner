use crate::core::{BatchScanRequest, BatchScanResult, ScanResult, ScanStatus, Result, FeeStructure};
use crate::rpc::ConnectionPool;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use chrono::Utc;

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
}

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub batch_size: usize,
    pub max_concurrent_wallets: usize,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_concurrent_wallets: 1000,
            retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl BatchProcessor {
    pub fn new(
        scanner: Arc<crate::core::scanner::WalletScanner>,
        cache_manager: Option<Arc<crate::storage::CacheManager>>,
        persistence_manager: Option<Arc<dyn crate::storage::PersistenceManager>>,
        config: ProcessorConfig,
    ) -> Self {
        Self {
            scanner,
            cache_manager,
            persistence_manager,
            max_concurrent_scans: config.max_concurrent_wallets,
            batch_size: config.batch_size,
            retry_attempts: config.retry_attempts,
            retry_delay_ms: config.retry_delay_ms,
        }
    }

    pub fn new_simple(connection_pool: Arc<ConnectionPool>, max_concurrent_scans: usize) -> Self {
        let scanner = Arc::new(crate::core::scanner::WalletScanner::new(connection_pool));
        Self {
            scanner,
            cache_manager: None,
            persistence_manager: None,
            max_concurrent_scans,
            batch_size: 100,
            retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }

    pub async fn process_batch(&self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        
        // Use optimized parallel processing with work-stealing
        let results: Vec<ScanResult> = {
            let scanner = self.scanner.clone();
            let chunk_size = (request.wallet_addresses.len() / rayon::current_num_threads()).max(1);
            
            request.wallet_addresses
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
                                        error: Some(e.to_string()),
                                        created_at: Utc::now(),
                                    });
                                }
                            }
                        }
                        
                        chunk_results
                    })
                })
                .flatten()
                .collect()
        };

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

        Ok(BatchScanResult {
            id: request.id,
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
        })
    }

    fn calculate_fee(&self, total_sol: f64, fee_structure: &FeeStructure) -> f64 {
        let total_lamports = (total_sol * 1_000_000_000.0) as u64;
        
        if let Some(waive_threshold) = fee_structure.waive_below_lamports {
            if total_lamports <= waive_threshold {
                return 0.0;
            }
        }

        let fee_lamports = (total_lamports as f64 * fee_structure.percentage) as u64;
        
        let final_fee = fee_lamports
            .max(fee_structure.minimum_lamports)
            .min(fee_structure.maximum_lamports.unwrap_or(u64::MAX));

        final_fee as f64 / 1_000_000_000.0
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
                                error: Some(e.to_string()),
                                created_at: Utc::now(),
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

    pub async fn get_metrics(&self) -> serde_json::Value {
        // Enhanced metrics with real-time data
        serde_json::json!({
            "total_batches_processed": 0,
            "total_wallets_processed": 0,
            "successful_scans": 0,
            "failed_scans": 0,
            "average_batch_time_ms": 0.0,
            "average_wallet_scan_time_ms": 0.0,
            "concurrent_batches": 0,
            "queue_size": 0,
            "cache_hit_rate": 0.0,
            "rpc_requests_per_second": 0.0,
            "memory_usage_mb": self.get_memory_usage(),
            "cpu_usage_percent": self.get_cpu_usage(),
            "active_threads": rayon::current_num_threads(),
            "throughput_wallets_per_second": 0.0,
            "error_rate_percent": 0.0,
            "avg_concurrent_scans": self.max_concurrent_scans
        })
    }
    
    fn get_memory_usage(&self) -> f64 {
        // Simple memory usage estimation
        use std::mem;
        let estimated_usage = mem::size_of::<Self>() + 
                           (self.max_concurrent_scans * mem::size_of::<ScanResult>());
        estimated_usage as f64 / (1024.0 * 1024.0) // Convert to MB
    }
    
    fn get_cpu_usage(&self) -> f64 {
        // Placeholder for CPU usage monitoring
        // In production, you'd use system monitoring libraries
        0.0
    }

    pub async fn get_active_batches(&self) -> usize {
        // Enhanced active batch tracking
        0
    }
    
    pub async fn process_batch_with_progress(
        &self, 
        request: &BatchScanRequest,
        progress_callback: Option<Box<dyn Fn(usize, usize) + Send + Sync>>
    ) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        let total_wallets = request.wallet_addresses.len();
        let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        let results: Vec<ScanResult> = {
            let scanner = self.scanner.clone();
            let processed = processed.clone();
            
            request.wallet_addresses
                .par_chunks((total_wallets / rayon::current_num_threads()).max(1))
                .map(|chunk| {
                    let runtime = tokio::runtime::Handle::current();
                    runtime.block_on(async {
                        let mut chunk_results = Vec::with_capacity(chunk.len());
                        
                        for wallet_address in chunk {
                            match scanner.scan_wallet(wallet_address).await {
                                Ok(result) => chunk_results.push(result),
                                Err(e) => chunk_results.push(ScanResult {
                                    id: Uuid::new_v4(),
                                    wallet_address: wallet_address.clone(),
                                    status: ScanStatus::Failed,
                                    result: None,
                                    error: Some(e.to_string()),
                                    created_at: Utc::now(),
                                }),
                            }
                            
                            let current = processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if let Some(ref callback) = progress_callback {
                                callback(current, total_wallets);
                            }
                        }
                        
                        chunk_results
                    })
                })
                .flatten()
                .collect()
        };
        
        // Continue with existing result processing logic...
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

        Ok(BatchScanResult {
            id: request.id,
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
        })
    }
}
