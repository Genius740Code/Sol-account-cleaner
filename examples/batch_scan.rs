//! Batch wallet scanning example
//! 
//! This example demonstrates how to scan multiple wallets efficiently
//! using the batch processing capabilities of the solana-recover crate.

use solana_recover::{BatchProcessor, BatchScanRequest, ScanStatus};
use solana_recover::core::types::RpcEndpoint;
use solana_recover::rpc::ConnectionPool;
use std::sync::Arc;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <wallet_addresses_file> [fee_percentage]", args[0]);
        eprintln!();
        eprintln!("wallet_addresses_file: Text file with one wallet address per line");
        eprintln!("fee_percentage: Optional fee percentage (default: 0.15)");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} wallets.txt 0.10", args[0]);
        std::process::exit(1);
    }
    
    let file_path = &args[1];
    let fee_percentage: f64 = args.get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.15);
    
    println!("Loading wallet addresses from: {}", file_path);
    
    // Read wallet addresses from file
    let content = std::fs::read_to_string(file_path)?;
    let wallet_addresses: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| line.trim().to_string())
        .collect();
    
    if wallet_addresses.is_empty() {
        eprintln!("No wallet addresses found in file");
        std::process::exit(1);
    }
    
    println!("Found {} wallet addresses to scan", wallet_addresses.len());
    println!("Fee percentage: {:.1}%", fee_percentage * 100.0);
    println!();
    
    // Create batch processor (simple mode: default mainnet endpoint)
    let rpc_endpoint = RpcEndpoint {
        url: solana_recover::DEFAULT_MAINNET_ENDPOINT.to_string(),
        priority: 0,
        rate_limit_rps: 100,
        timeout_ms: 30000,
        healthy: true,
    };
    let pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 8));
    let processor = BatchProcessor::new_simple(pool, 10);
    
    // Create batch request
    let request = BatchScanRequest {
        id: uuid::Uuid::new_v4(),
        wallet_addresses,
        user_id: None,
        fee_percentage: Some(fee_percentage),
        created_at: chrono::Utc::now(),
    };
    
    println!("Starting batch scan...");
    let start_time = std::time::Instant::now();
    
    // Process batch
    let results = processor.process_batch(&request).await?;
    
    let elapsed = start_time.elapsed();
    
    println!("✓ Batch scan completed in {}ms", elapsed.as_millis());
    println!();
    
    // Display results
    let mut total_recoverable = 0.0;
    let mut total_empty_accounts: u64 = 0;
    let mut successful_scans = 0;
    
    println!("Scan Results Summary:");
    println!("  Total wallets: {}", results.results.len());
    println!("  Successful scans: {}", results.results.iter().filter(|r| r.status == ScanStatus::Completed).count());
    println!("  Failed scans: {}", results.results.iter().filter(|r| r.status == ScanStatus::Failed).count());
    println!();
    
    println!("Detailed Results:");
    for (i, scan_result) in results.results.iter().enumerate() {
        match &scan_result.result {
            Some(result) => {
                successful_scans += 1;
                total_recoverable += result.recoverable_sol;
                total_empty_accounts += result.empty_accounts;
                
                println!("  {}. ✓ {} - {:.9} SOL recoverable ({} empty accounts)", 
                         i + 1, 
                         scan_result.wallet_address,
                         result.recoverable_sol,
                         result.empty_accounts);
            }
            None => {
                println!("  {}. ✗ {} - Error: {}", 
                         i + 1, 
                         scan_result.wallet_address,
                         scan_result.error_message.as_deref().unwrap_or("unknown"));
            }
        }
    }
    
    println!();
    println!("Total Recoverable SOL: {:.9}", total_recoverable);
    println!("Total Empty Accounts: {}", total_empty_accounts);
    
    if successful_scans > 0 {
        println!("Average scan time: {:.2}ms", elapsed.as_millis() as f64 / successful_scans as f64);
    }
    
    // Export results to JSON if requested
    if env::var("EXPORT_JSON").is_ok() {
        let json_output = serde_json::to_string_pretty(&results)?;
        std::fs::write("batch_scan_results.json", json_output)?;
        println!("Results exported to batch_scan_results.json");
    }
    
    Ok(())
}
