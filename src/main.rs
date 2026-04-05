//! Simple CLI for the solana-recover crate

use clap::{Parser, Subcommand};
use solana_recover::scan_wallet;
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "solana-recover")]
#[command(about = "A high-performance Solana wallet scanner and SOL recovery tool")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a single wallet for empty accounts
    Scan {
        /// The wallet address to scan
        address: String,
        /// RPC endpoint to use (defaults to mainnet)
        #[arg(long)]
        rpc_endpoint: Option<String>,
    },
    /// Scan multiple wallets from a file
    Batch {
        /// File containing wallet addresses (one per line)
        file: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { address, rpc_endpoint } => {
            println!("🔍 Scanning wallet: {}", address);
            if let Some(endpoint) = &rpc_endpoint {
                println!("📍 Using RPC endpoint: {}", endpoint);
            } else {
                println!("📍 Using default mainnet endpoint");
            }
            println!();

            let start_time = std::time::Instant::now();
            let result = scan_wallet(&address, rpc_endpoint.as_deref()).await?;
            let elapsed = start_time.elapsed();

            print!("✅ Scan completed in {}ms\n", elapsed.as_millis());
            print_scan_result(&result);
        }
        Commands::Batch { file } => {
            println!("📁 Loading wallets from: {}", file);
            
            let content = std::fs::read_to_string(&file)?;
            let wallets: Vec<String> = content
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .map(|line| line.trim().to_string())
                .collect();

            if wallets.is_empty() {
                eprintln!("❌ No wallet addresses found in file");
                std::process::exit(1);
            }

            println!("📊 Found {} wallet addresses to scan", wallets.len());
            println!();

            let mut total_recoverable = 0.0;
            let mut successful_scans = 0;

            for (i, wallet) in wallets.iter().enumerate() {
                print!("🔍 Scanning wallet {}/{}: {} ... ", i + 1, wallets.len(), wallet);
                io::stdout().flush()?;

                match scan_wallet(wallet, None).await {
                    Ok(result) => {
                        successful_scans += 1;
                        total_recoverable += result.recoverable_sol;
                        println!("✅ {:.9} SOL", result.recoverable_sol);
                    }
                    Err(e) => {
                        println!("❌ {}", e);
                    }
                }
            }

            println!();
            println!("📊 Batch Scan Summary:");
            println!("  Total wallets: {}", wallets.len());
            println!("  Successful scans: {}", successful_scans);
            println!("  Failed scans: {}", wallets.len() - successful_scans);
            println!("  Total recoverable SOL: {:.9}", total_recoverable);
        }
    }

    Ok(())
}

fn print_scan_result(result: &solana_recover::WalletScanResult) {
    println!("📋 Scan Results:");
    println!("  Wallet Address: {}", result.wallet_address);
    println!("  Total Accounts: {}", result.total_accounts);
    println!("  Empty Accounts: {}", result.empty_accounts.len());
    println!("  Recoverable SOL: {:.9} SOL", result.recoverable_sol);
    println!("  Scan Time: {}ms", result.scan_time_ms);

    if !result.empty_accounts.is_empty() {
        println!();
        println!("💰 Empty Account Details:");
        for (i, account) in result.empty_accounts.iter().enumerate() {
            println!("  {}. {} ({} lamports)", i + 1, account.address, account.lamports);
        }
    }

    if result.recoverable_sol > 0.0 {
        println!();
        println!("💎 This wallet has {:.9} SOL available for recovery!", result.recoverable_sol);
    } else {
        println!();
        println!("💸 No SOL available for recovery from this wallet.");
    }
}
