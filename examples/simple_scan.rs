//! Simple wallet scanning example
//! 
//! This example demonstrates the basic usage of the solana-recover crate
//! to scan a single wallet for empty token accounts.

use solana_recover::scan_wallet;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <wallet_address> [rpc_endpoint]", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", args[0]);
        eprintln!("  {} 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM https://api.devnet.solana.com", args[0]);
        std::process::exit(1);
    }
    
    let wallet_address = &args[1];
    let rpc_endpoint = args.get(2).map(|s| s.as_str());
    
    println!("Scanning wallet: {}", wallet_address);
    if let Some(endpoint) = rpc_endpoint {
        println!("Using RPC endpoint: {}", endpoint);
    } else {
        println!("Using default mainnet endpoint");
    }
    println!();
    
    let start_time = std::time::Instant::now();
    let result = scan_wallet(wallet_address, rpc_endpoint).await?;
    let elapsed = start_time.elapsed();
    
    println!("Scan Results:");
    println!("  Wallet Address: {}", result.address);
    println!("  Total Accounts: {}", result.total_accounts);
    println!("  Empty Accounts: {}", result.empty_accounts);
    println!("  Recoverable SOL: {:.9} SOL", result.recoverable_sol);
    println!("  Scan Time: {}ms (reported)", result.scan_time_ms);
    println!("  Total Time: {}ms (actual)", elapsed.as_millis());
    
    if result.empty_accounts > 0 {
        println!();
        println!("Empty Account Addresses:");
        for (i, account_address) in result.empty_account_addresses.iter().enumerate() {
            println!("  {}. {}", i + 1, account_address);
        }
    }
    
    if result.recoverable_sol > 0.0 {
        println!();
        println!("This wallet has {:.9} SOL available for recovery", result.recoverable_sol);
    } else {
        println!();
        println!("No SOL available for recovery from this wallet.");
    }
    
    Ok(())
}
