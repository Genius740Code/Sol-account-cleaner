use crate::core::{BatchScanRequest, BatchScanResult, ScanResult, ScanStatus, Result, FeeStructure};
use crate::rpc::ConnectionPool;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use chrono::Utc;

pub struct BatchProcessor {
    scanner: Arc<crate::core::scanner::WalletScanner>,
    max_concurrent_scans: usize,
}

impl BatchProcessor {
    pub fn new(connection_pool: Arc<ConnectionPool>, max_concurrent_scans: usize) -> Self {
        let scanner = Arc::new(crate::core::scanner::WalletScanner::new(connection_pool));
        Self {
            scanner,
            max_concurrent_scans,
        }
    }

    pub async fn process_batch(&self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let _start_time = Instant::now();
        
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

        Ok(BatchScanResult {
            id: request.id,
            total_wallets: request.wallet_addresses.len(),
            completed_wallets,
            failed_wallets,
            total_recoverable_sol,
            estimated_fee_sol,
            results,
            created_at: request.created_at,
            completed_at: Some(Utc::now()),
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
}
