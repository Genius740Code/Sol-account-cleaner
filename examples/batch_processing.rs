//! Batch processing example
//! 
//! This example demonstrates how to use the Solana Recover library
//! to scan multiple wallets in batches with concurrent processing.

use solana_recover::*;
use std::sync::Arc;
use tracing::{info, error, warn};
use tokio::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let logging_config = LoggingConfig {
        level: "info".to_string(),
        format: solana_recover::utils::LogFormat::Pretty,
        output: solana_recover::utils::LogOutput::Stdout,
        file_path: None,
        json_fields: vec![],
    };
    
    Logger::init(logging_config)?;
    
    info!("Starting batch processing example");
    
    // Load configuration
    let config = Config::load()?;
    
    // Create RPC endpoints
    let rpc_endpoints: Vec<RpcEndpoint> = config.rpc.endpoints
        .iter()
        .enumerate()
        .map(|(i, url)| RpcEndpoint {
            url: url.clone(),
            priority: i as u8,
            rate_limit_rps: config.rpc.rate_limit_rps,
            timeout_ms: config.rpc.timeout_ms,
            healthy: true,
        })
        .collect();
    
    // Create connection pool
    let connection_pool = Arc::new(ConnectionPool::new(rpc_endpoints, config.rpc.pool_size));
    
    // Create scanner and batch processor
    let scanner = Arc::new(WalletScanner::new(connection_pool));
    let batch_processor = Arc::new(BatchProcessor::new(
        scanner.clone(),
        None, // No cache for this example
        None, // No persistence for this example
        config.scanner.into(),
    ));
    
    // Example wallet addresses to scan
    let wallet_addresses = vec![
        "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "11111111111111111111111111111111112",
        "9z3FPhYcJgq2a9Lk6M4qNkYhjQkxEJm4D4L4Q9F4Q9R",
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
        "1f1Y9WjQJvVzqBvUgJYYBm3K3d3v3v3v3v3v3v3v3v3v3v",
        "2g2X8kKvWzqCvUgKZZCnL4l4w4w4w4w4w4w4w4w4w4w4w",
        "3h3Y9lLwXrDvWhgLAAoM5m5x5x5x5x5x5x5x5x5x5x5x",
    ];
    
    info!("Processing {} wallets in batch", wallet_addresses.len());
    
    // Create batch request
    let batch_request = BatchScanRequest {
        id: uuid::Uuid::new_v4(),
        wallet_addresses: wallet_addresses.clone(),
        user_id: Some("batch_example_user".to_string()),
        fee_percentage: Some(0.15), // 15% fee
        created_at: chrono::Utc::now(),
    };
    
    // Start timing
    let start_time = Instant::now();
    
    // Process batch
    info!("Starting batch scan...");
    match batch_processor.process_batch(&batch_request).await {
        Ok(batch_result) => {
            let duration = start_time.elapsed();
            
            info!("Batch scan completed successfully!");
            info!("Batch ID: {:?}", batch_result.id);
            info!("Total wallets: {}", batch_result.total_wallets);
            info!("Successful scans: {}", batch_result.successful_scans);
            info!("Failed scans: {}", batch_result.failed_scans);
            info!("Duration: {:?}", duration);
            info!("Total recoverable SOL: {:.9}", batch_result.total_recoverable_sol);
            info!("Estimated fee SOL: {:.9}", batch_result.estimated_fee_sol);
            
            // Calculate detailed fee breakdown
            let fee_structure = FeeStructure {
                percentage: 0.15,
                minimum_lamports: 1_000_000,
                maximum_lamports: Some(10_000_000),
                waive_below_lamports: Some(5_000_000),
            };
            
            let mut total_recoverable = 0.0;
            let mut total_fees = 0.0;
            let mut waived_count = 0;
            
            println!("\n=== Detailed Results ===");
            
            for scan_result in &batch_result.results {
                match &scan_result.status {
                    ScanStatus::Completed => {
                        if let Some(wallet_info) = &scan_result.result {
                            let fee_calc = FeeCalculator::calculate_wallet_fee(wallet_info, &fee_structure);
                            total_recoverable += wallet_info.recoverable_sol;
                            total_fees += fee_calc.fee_lamports as f64 / 1_000_000_000.0;
                            
                            if fee_calc.fee_waived {
                                waived_count += 1;
                            }
                            
                            println!(
                                "✅ {}: {:.9} SOL recoverable, {:.9} SOL fee{}",
                                scan_result.wallet_address,
                                wallet_info.recoverable_sol,
                                fee_calc.fee_lamports as f64 / 1_000_000_000.0,
                                if fee_calc.fee_waived { " (WAIVED)" } else { "" }
                            );
                        }
                    }
                    ScanStatus::Failed => {
                        println!(
                            "❌ {}: Failed - {}",
                            scan_result.wallet_address,
                            scan_result.error.as_deref().unwrap_or("Unknown error")
                        );
                    }
                    _ => {
                        println!(
                            "⏳ {}: {:?}",
                            scan_result.wallet_address,
                            scan_result.status
                        );
                    }
                }
            }
            
            println!("\n=== Summary ===");
            println!("Total wallets processed: {}", batch_result.total_wallets);
            println!("Successful scans: {}", batch_result.successful_scans);
            println!("Failed scans: {}", batch_result.failed_scans);
            println!("Total recoverable SOL: {:.9}", total_recoverable);
            println!("Total fees SOL: {:.9}", total_fees);
            println!("Net recoverable SOL: {:.9}", total_recoverable - total_fees);
            println!("Fees waived: {}", waived_count);
            println!("Processing time: {:?}", duration);
            
            if batch_result.total_wallets > 0 {
                let avg_time_per_wallet = duration.as_millis() as f64 / batch_result.total_wallets as f64;
                println!("Average time per wallet: {:.2} ms", avg_time_per_wallet);
            }
        }
        Err(e) => {
            error!("Batch scan failed: {}", e);
            return Err(e.into());
        }
    }
    
    // Demonstrate concurrent batch processing
    info!("\n=== Concurrent Batch Processing Demo ===");
    
    let concurrent_batches = 3;
    let wallets_per_batch = 3;
    let mut handles = vec![];
    
    for batch_num in 0..concurrent_batches {
        let processor_clone = batch_processor.clone();
        
        let start_idx = batch_num * wallets_per_batch;
        let end_idx = (start_idx + wallets_per_batch).min(wallet_addresses.len());
        
        if start_idx >= wallet_addresses.len() {
            break;
        }
        
        let batch_wallets = wallet_addresses[start_idx..end_idx].to_vec();
        
        let handle = tokio::spawn(async move {
            let concurrent_request = BatchScanRequest {
                id: uuid::Uuid::new_v4(),
                wallet_addresses: batch_wallets,
                user_id: Some(format!("concurrent_user_{}", batch_num)),
                fee_percentage: Some(0.10), // 10% fee for concurrent batches
                created_at: chrono::Utc::now(),
            };
            
            let start = Instant::now();
            let result = processor_clone.process_batch(&concurrent_request).await;
            let duration = start.elapsed();
            
            (batch_num, result, duration)
        });
        
        handles.push(handle);
    }
    
    // Wait for all concurrent batches to complete
    for handle in handles {
        match handle.await {
            Ok((batch_num, result, duration)) => {
                match result {
                    Ok(batch_result) => {
                        info!(
                            "Concurrent batch {}: {} wallets, {} successful, {} failed, duration: {:?}",
                            batch_num,
                            batch_result.total_wallets,
                            batch_result.successful_scans,
                            batch_result.failed_scans,
                            duration
                        );
                    }
                    Err(e) => {
                        error!("Concurrent batch {} failed: {}", batch_num, e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to join concurrent batch task: {}", e);
            }
        }
    }
    
    info!("Batch processing example completed");
    Ok(())
}
