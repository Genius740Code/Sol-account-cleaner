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
        
        let results: Vec<ScanResult> = request.wallet_addresses
            .par_chunks(self.max_concurrent_scans)
            .flat_map(|chunk| {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    let mut chunk_results = Vec::new();
                    for wallet_address in chunk {
                        match self.scanner.scan_wallet(wallet_address).await {
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
                    }
                    chunk_results
                })
            })
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

        tokio::spawn(async move {
            for wallet_address in wallet_addresses {
                match scanner.scan_wallet(&wallet_address).await {
                    Ok(result) => {
                        if tx.send(result).is_err() {
                            break; // Receiver dropped
                        }
                    }
                    Err(e) => {
                        let error_result = ScanResult {
                            id: Uuid::new_v4(),
                            wallet_address,
                            status: ScanStatus::Failed,
                            result: None,
                            error: Some(e.to_string()),
                            created_at: Utc::now(),
                        };
                        if tx.send(error_result).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    pub async fn get_metrics(&self) -> serde_json::Value {
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
            "rpc_requests_per_second": 0.0
        })
    }

    pub async fn get_active_batches(&self) -> usize {
        0
    }
}
