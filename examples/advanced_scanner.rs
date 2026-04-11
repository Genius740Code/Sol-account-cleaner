//! Advanced scanner configuration example
//! 
//! This example shows how to use the WalletScanner with custom configuration
//! for optimal performance and advanced features.

use solana_recover::{WalletScanner, ScanConfig};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <wallet_address> [concurrency] [timeout_seconds]", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", args[0]);
        eprintln!("  {} 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM 20 60", args[0]);
        std::process::exit(1);
    }
    
    let wallet_address = &args[1];
    let max_concurrent: usize = args.get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let timeout_seconds: u64 = args.get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    
    println!("Advanced Scanner Configuration:");
    println!("  Wallet: {}", wallet_address);
    println!("  Max Concurrent: {}", max_concurrent);
    println!("  Timeout: {}s", timeout_seconds);
    println!();
    
    // Create custom configuration
    let config = ScanConfig {
        rpc_endpoint: "https://api.mainnet-beta.solana.com".to_string(),
        max_concurrent,
        timeout_seconds,
        enable_cache: true,
    };
    
    println!("Initializing scanner with custom configuration...");
    let scanner = WalletScanner::with_config(config).await?;
    
    println!("✓ Scanner initialized successfully");
    println!();
    
    // Perform scan with timeout
    println!("Starting wallet scan (with {}s timeout)...", timeout_seconds);
    let start_time = std::time::Instant::now();
    
    let scan_result = timeout(
        Duration::from_secs(timeout_seconds),
        scanner.scan_wallet(wallet_address)
    ).await;
    
    let elapsed = start_time.elapsed();
    
    match scan_result {
        Ok(Ok(result)) => {
            println!("✓ Scan completed successfully");
            println!();
            println!("Results:");
            println!("  Total Accounts: {}", result.total_accounts);
            println!("  Empty Accounts: {}", result.empty_accounts.len());
            println!("  Recoverable SOL: {:.9}", result.recoverable_sol);
            println!("  Scan Time: {}ms", result.scan_time_ms);
            println!("  Actual Time: {}ms", elapsed.as_millis());
            
            // Performance metrics
            let accounts_per_second = result.total_accounts as f64 / elapsed.as_secs_f64();
            println!("  Throughput: {:.2} accounts/second", accounts_per_second);
            
            if !result.empty_accounts.is_empty() {
                println!();
                println!("Empty Accounts Summary:");
                let total_lamports: u64 = result.empty_accounts.iter()
                    .map(|acc| acc.lamports)
                    .sum();
                println!("  Total lamports in empty accounts: {}", total_lamports);
                println!("  Average lamports per account: {}", 
                         total_lamports / result.empty_accounts.len() as u64);
            }
        }
        Ok(Err(e)) => {
            eprintln!("✗ Scan failed: {}", e);
            
            // Provide helpful error messages
            if e.to_string().contains("timeout") {
                eprintln!("Try increasing timeout or reducing concurrency");
            } else if e.to_string().contains("rate limit") {
                eprintln!("Try reducing concurrency or using a different RPC endpoint");
            }
        }
        Err(_) => {
            eprintln!("✗ Scan timed out after {} seconds", timeout_seconds);
            eprintln!("Try increasing timeout or reducing concurrency");
        }
    }
    
    // Demonstrate scanner reuse
    println!();
    println!("Testing scanner reuse with another address...");
    let test_address = "11111111111111111111111111111112"; // System Program
    
    match scanner.scan_wallet(test_address).await {
        Ok(result) => {
            println!("✓ Reuse test successful - {} accounts found", result.total_accounts);
        }
        Err(e) => {
            println!("⚠ Reuse test failed: {}", e);
        }
    }
    
    println!();
    println!("Performance Tips:");
    println!("  - Increase max_concurrent for faster scans (watch rate limits)");
    println!("  - Use devnet for testing: https://api.devnet.solana.com");
    println!("  - Enable caching for repeated scans of the same wallets");
    println!("  - Consider using multiple RPC endpoints for production");
    
    Ok(())
}
