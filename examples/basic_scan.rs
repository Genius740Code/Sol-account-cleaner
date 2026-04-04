//! Basic wallet scanning example
//! 
//! This example demonstrates how to use the Solana Recover library
//! to scan a single wallet for recoverable SOL.

use solana_recover::*;
use std::sync::Arc;
use tracing::{info, error};

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
    
    info!("Starting basic wallet scan example");
    
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
    
    // Create wallet scanner
    let scanner = Arc::new(WalletScanner::new(connection_pool));
    
    // Example wallet addresses to scan
    let wallet_addresses = vec![
        "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
    ];
    
    // Scan each wallet
    for wallet_address in wallet_addresses {
        info!("Scanning wallet: {}", wallet_address);
        
        match scanner.scan_wallet(wallet_address).await {
            Ok(scan_result) => {
                info!("Scan completed for wallet: {}", wallet_address);
                info!("Scan ID: {}", scan_result.id);
                info!("Status: {:?}", scan_result.status);
                
                if let Some(wallet_info) = scan_result.result {
                    info!("Total accounts: {}", wallet_info.total_accounts);
                    info!("Empty accounts: {}", wallet_info.empty_accounts);
                    info!("Recoverable SOL: {:.9}", wallet_info.recoverable_sol);
                    info!("Recoverable lamports: {}", wallet_info.recoverable_lamports);
                    
                    if !wallet_info.empty_account_addresses.is_empty() {
                        info!("Empty account addresses:");
                        for (i, addr) in wallet_info.empty_account_addresses.iter().enumerate() {
                            info!("  {}. {}", i + 1, addr);
                        }
                    }
                    
                    // Calculate fees
                    let fee_structure = FeeStructure::default();
                    let fee_calculation = FeeCalculator::calculate_wallet_fee(&wallet_info, &fee_structure);
                    
                    info!("Fee calculation:");
                    info!("  Fee amount: {:.9} SOL", fee_calculation.fee_lamports as f64 / 1_000_000_000.0);
                    info!("  Net recoverable: {:.9} SOL", fee_calculation.net_recoverable_lamports as f64 / 1_000_000_000.0);
                    info!("  Fee waived: {}", fee_calculation.fee_waived);
                }
                
                if let Some(error) = scan_result.error {
                    error!("Scan error: {}", error);
                }
            }
            Err(e) => {
                error!("Failed to scan wallet {}: {}", wallet_address, e);
            }
        }
        
        println!("{}", "-".repeat(60));
    }
    
    info!("Basic wallet scan example completed");
    Ok(())
}
