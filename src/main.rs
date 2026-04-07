//! Simple CLI for the solana-recover crate

use clap::{Parser, Subcommand};
use solana_recover::{scan_wallet, recover_sol, WalletInfo, RecoveryRequest};
use solana_recover::wallet::{WalletManager, WalletCredentials, WalletType, WalletCredentialData};
use solana_sdk::signature::{Keypair, Signer};
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "solana-recover")]
#[command(about = "A high-performance Solana wallet scanner and SOL recovery tool")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Single wallet address to scan (quick mode) - mutually exclusive with --private-key
    #[arg(short, long, conflicts_with = "private_key")]
    wallet: Option<String>,
    
    /// Private key for wallet ownership verification (mutually exclusive with --wallet)
    #[arg(short, long, conflicts_with = "wallet")]
    private_key: Option<String>,
    
    /// Destination wallet for SOL recovery (optional - defaults to your wallet if not specified)
    #[arg(short, long)]
    destination: Option<String>,
    
    /// Skip confirmation prompts
    #[arg(long, default_value_t = false)]
    force: bool,
    
    /// Show detailed developer information (including account addresses)
    #[arg(short = 'D', long, default_value_t = false)]
    dev: bool,
    
    #[command(subcommand)]
    command: Option<Commands>,
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
        /// Show detailed developer information (including account addresses)
        #[arg(short, long, default_value_t = false)]
        dev: bool,
    },
    /// Show total claimable SOL for wallets
    Show {
        /// Wallet addresses (comma-separated) or private keys (comma-separated)
        /// Format: wallet:address1,address2 or key:privkey1,privkey2
        #[arg(short, long)]
        targets: String,
        /// RPC endpoint to use (defaults to mainnet)
        #[arg(long)]
        rpc_endpoint: Option<String>,
        /// Show detailed developer information (including account addresses)
        #[arg(short, long, default_value_t = false)]
        dev: bool,
    },
    /// Reclaim SOL from empty accounts
    Reclaim {
        /// Wallet addresses (comma-separated) or private keys (comma-separated)
        /// Format: wallet:address1,address2 or key:privkey1,privkey2
        #[arg(short, long)]
        targets: String,
        /// Destination wallet address for recovered SOL
        #[arg(short, long)]
        destination: String,
        /// RPC endpoint to use (defaults to mainnet)
        #[arg(long)]
        rpc_endpoint: Option<String>,
        /// Skip confirmation prompt
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Show detailed developer information (including account addresses)
        #[arg(short, long, default_value_t = false)]
        dev: bool,
    },
    /// Scan multiple wallets from a file
    Batch {
        /// File containing wallet addresses (one per line)
        file: String,
        /// Show detailed developer information (including account addresses)
        #[arg(short, long, default_value_t = false)]
        dev: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle private key mode (scan + reclaim)
    if let Some(private_key) = &cli.private_key {
        // Derive wallet address from private key
        let key_bytes = bs58::decode(private_key)
            .into_vec()
            .map_err(|_| "Invalid private key format: not valid base58")?;
        
        if key_bytes.len() != 64 {
            return Err("Invalid private key format: expected 64 bytes".into());
        }
        
        let keypair = Keypair::from_bytes(&key_bytes)
            .map_err(|_| "Invalid private key format: cannot create keypair")?;
        let wallet_address = keypair.pubkey().to_string();
        
        println!("Scanning wallet: {}", wallet_address);
        println!("Using default mainnet endpoint");
        println!();

        let start_time = std::time::Instant::now();
        let result = scan_wallet(&wallet_address, None).await?;
        let elapsed = start_time.elapsed();

        print!("✓ Scan completed in {}ms\n", elapsed.as_millis());
        print_scan_result(&result, cli.dev);

        // Auto-reclaim if SOL is available - no confirmation needed for private key
        if result.recoverable_sol > 0.0 {
            let destination = cli.destination.as_ref().unwrap_or(&wallet_address);
            
            println!("✓ Private key validation successful");
            println!();
            println!("Reclaiming SOL to: {}", destination);

            let empty_account_addresses: Vec<String> = result.empty_account_addresses.clone();
            
            // Create shared wallet manager and connect with private key
            let wallet_manager = std::sync::Arc::new(WalletManager::new());
            let credentials = WalletCredentials {
                wallet_type: WalletType::PrivateKey,
                credentials: WalletCredentialData::PrivateKey {
                    private_key: private_key.clone(),
                },
            };
            
            let wallet_connection = wallet_manager.connect_wallet(credentials).await?;
            let connection_id = wallet_connection.id;
            
            let recovery_request = RecoveryRequest {
                id: uuid::Uuid::new_v4(),
                wallet_address: wallet_address.clone(),
                destination_address: destination.clone(),
                empty_accounts: empty_account_addresses,
                max_fee_lamports: Some(10_000_000),
                priority_fee_lamports: None,
                wallet_connection_id: Some(connection_id),
                user_id: None,
                created_at: chrono::Utc::now(),
            };
            
            let start_time = std::time::Instant::now();
            match recover_sol(&recovery_request, None, Some(wallet_manager)).await {
                Ok(recovery_result) => {
                    let elapsed = start_time.elapsed();
                    
                    // Check the actual recovery status, not just Ok()
                    match recovery_result.status {
                        solana_recover::core::types::RecoveryStatus::Completed => {
                            print!("✓ Reclamation completed in {}ms\n", elapsed.as_millis());
                            println!("Successfully reclaimed {:.9} SOL!", recovery_result.net_sol);
                        }
                        solana_recover::core::types::RecoveryStatus::Failed => {
                            print!("✗ Reclamation failed in {}ms\n", elapsed.as_millis());
                            println!("Error: {}", recovery_result.error.unwrap_or_else(|| "Unknown error".to_string()));
                        }
                        _ => {
                            print!("⚠ Reclamation incomplete in {}ms\n", elapsed.as_millis());
                            println!("⚠ Status: {:?}", recovery_result.status);
                        }
                    }
                }
                Err(e) => {
                    println!("✗ Reclamation failed: {}", e);
                }
            }
        } else {
            println!();
            println!("No SOL available to reclaim from this wallet.");
        }
    }
    // Handle wallet address mode (scan only)
    else if let Some(wallet_address) = &cli.wallet {
        println!("Scanning wallet: {}", wallet_address);
        println!("Using default mainnet endpoint");
        println!();

        let start_time = std::time::Instant::now();
        let result = scan_wallet(wallet_address, None).await?;
        let elapsed = start_time.elapsed();

        print!("✓ Scan completed in {}ms\n", elapsed.as_millis());
        print_scan_result(&result, cli.dev);
    }

    // Handle subcommands
    match cli.command {
        Some(Commands::Scan { address, rpc_endpoint, dev }) => {
            println!("Scanning wallet: {}", address);
            if let Some(endpoint) = &rpc_endpoint {
                println!("Using RPC endpoint: {}", endpoint);
            } else {
                println!("Using default mainnet endpoint");
            }
            println!();

            let start_time = std::time::Instant::now();
            let result = scan_wallet(&address, rpc_endpoint.as_deref()).await?;
            let elapsed = start_time.elapsed();

            print!("✓ Scan completed in {}ms\n", elapsed.as_millis());
            print_scan_result(&result, dev);
        }
        Some(Commands::Show { targets, rpc_endpoint, dev }) => {
            println!("Calculating total claimable SOL...");
            if let Some(endpoint) = &rpc_endpoint {
                println!("Using RPC endpoint: {}", endpoint);
            } else {
                println!("Using default mainnet endpoint");
            }
            println!();

            let start_time = std::time::Instant::now();
            let total = calculate_total_claimable(&targets, rpc_endpoint.as_deref(), dev).await?;
            let elapsed = start_time.elapsed();

            print!("✓ Calculation completed in {}ms\n", elapsed.as_millis());
            print_total_claim_result(&total, dev);
        }
        Some(Commands::Reclaim { targets, destination, rpc_endpoint, force, dev }) => {
            println!("Reclaiming SOL from empty accounts...");
            if let Some(endpoint) = &rpc_endpoint {
                println!("Using RPC endpoint: {}", endpoint);
            } else {
                println!("Using default mainnet endpoint");
            }
            println!("Destination: {}", destination);
            println!();

            if !force {
                println!("⚠ This will close empty token accounts and transfer their rent exemption SOL to {}", destination);
                print!("Are you sure you want to continue? [y/N]: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("✗ Reclamation cancelled");
                    return Ok(());
                }
            }

            let start_time = std::time::Instant::now();
            let result = reclaim_sol_from_targets(&targets, &destination, rpc_endpoint.as_deref(), dev).await?;
            let elapsed = start_time.elapsed();

            print!("✅ Reclamation completed in {}ms\n", elapsed.as_millis());
            print_reclaim_result(&result, dev);
        }
        Some(Commands::Batch { file, dev }) => {
            println!("Loading wallets from: {}", file);
            
            let content = std::fs::read_to_string(&file)?;
            let wallets: Vec<String> = content
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .map(|line| line.trim().to_string())
                .collect();

            if wallets.is_empty() {
                eprintln!("✗ No wallet addresses found in file");
                std::process::exit(1);
            }

            println!("Found {} wallet addresses to scan", wallets.len());
            println!();

            let mut total_recoverable = 0.0;
            let mut successful_scans = 0;

            for (i, wallet) in wallets.iter().enumerate() {
                let wallet_display = if dev { wallet.clone() } else { "[wallet address hidden]".to_string() };
                print!("Scanning wallet {}/{}: {} ... ", i + 1, wallets.len(), wallet_display);
                io::stdout().flush()?;

                match scan_wallet(wallet, None).await {
                    Ok(result) => {
                        successful_scans += 1;
                        total_recoverable += result.recoverable_sol;
                        println!("✓ {:.9} SOL", result.recoverable_sol);
                    }
                    Err(e) => {
                        println!("✗ {}", e);
                    }
                }
            }

            println!();
            println!("Batch Scan Summary:");
            println!("  Total wallets: {}", wallets.len());
            println!("  Successful scans: {}", successful_scans);
            println!("  Failed scans: {}", wallets.len() - successful_scans);
            println!("  Total recoverable SOL: {:.9}", total_recoverable);
        }
        None => {
            // No subcommand and no wallet - exit silently
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct TotalClaimResult {
    total_wallets: usize,
    total_recoverable_sol: f64,
    wallet_results: Vec<(String, WalletInfo)>,
}

#[derive(Debug, Clone)]
struct ReclaimResult {
    total_wallets: usize,
    successful_reclaims: usize,
    total_recovered_sol: f64,
    total_fees_paid: f64,
    net_sol: f64,
    reclaim_details: Vec<(String, f64, f64)>, // (wallet, recovered, fees)
}

fn parse_targets(targets: &str) -> Result<(Vec<String>, bool), Box<dyn std::error::Error>> {
    if targets.starts_with("wallet:") {
        let addresses = targets.strip_prefix("wallet:")
            .unwrap()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok((addresses, false)) // false = not private keys
    } else if targets.starts_with("key:") {
        let private_keys = targets.strip_prefix("key:")
            .unwrap()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok((private_keys, true)) // true = private keys
    } else {
        Err("Invalid target format. Use 'wallet:addr1,addr2' or 'key:privkey1,privkey2'".into())
    }
}

async fn calculate_total_claimable(
    targets: &str,
    rpc_endpoint: Option<&str>,
    dev: bool,
) -> Result<TotalClaimResult, Box<dyn std::error::Error>> {
    let (wallets, is_private_keys) = parse_targets(targets)?;
    
    let mut total_recoverable = 0.0;
    let mut wallet_results = Vec::new();
    
    for (i, wallet) in wallets.iter().enumerate() {
        let wallet_display = if is_private_keys { 
            "[private key]".to_string() 
        } else if dev { 
            wallet.clone() 
        } else { 
            "[wallet address hidden]".to_string() 
        };
        print!("🔍 Scanning wallet {}/{}: {} ... ", i + 1, wallets.len(), wallet_display);
        io::stdout().flush()?;
        
        let scan_address = if is_private_keys {
            // Convert private key to wallet address
            derive_address_from_private_key(wallet)?
        } else {
            wallet.clone()
        };
        
        match scan_wallet(&scan_address, rpc_endpoint).await {
            Ok(result) => {
                total_recoverable += result.recoverable_sol;
                wallet_results.push((scan_address, result.clone()));
                println!("✅ {:.9} SOL", result.recoverable_sol);
            }
            Err(e) => {
                println!("❌ {}", e);
            }
        }
    }
    
    Ok(TotalClaimResult {
        total_wallets: wallets.len(),
        total_recoverable_sol: total_recoverable,
        wallet_results,
    })
}

async fn reclaim_sol_from_targets(
    targets: &str,
    destination: &str,
    rpc_endpoint: Option<&str>,
    dev: bool,
) -> Result<ReclaimResult, Box<dyn std::error::Error>> {
    let (wallets, is_private_keys) = parse_targets(targets)?;
    
    let mut total_recovered = 0.0;
    let mut total_fees = 0.0;
    let mut successful_reclaims = 0;
    let mut reclaim_details = Vec::new();
    
    for (i, wallet) in wallets.iter().enumerate() {
        let wallet_display = if is_private_keys { 
            "[private key]".to_string() 
        } else if dev { 
            wallet.clone() 
        } else { 
            "[wallet address hidden]".to_string() 
        };
        print!("Reclaiming from wallet {}/{}: {} ... ", i + 1, wallets.len(), wallet_display);
        io::stdout().flush()?;
        
        let scan_address = if is_private_keys {
            derive_address_from_private_key(wallet)?
        } else {
            wallet.clone()
        };
        
        // First scan to get empty accounts
        match scan_wallet(&scan_address, rpc_endpoint).await {
            Ok(scan_result) => {
                if scan_result.recoverable_sol > 0.0 {
                    // Create recovery request
                    let empty_account_addresses: Vec<String> = scan_result.empty_account_addresses.clone();
                    
                    let recovery_request = RecoveryRequest {
                        id: uuid::Uuid::new_v4(),
                        wallet_address: scan_address.clone(),
                        destination_address: destination.to_string(),
                        empty_accounts: empty_account_addresses,
                        max_fee_lamports: Some(10_000_000), // 0.01 SOL max fee
                        priority_fee_lamports: None,
                        wallet_connection_id: if is_private_keys {
                            Some(format!("private_key:{}", wallet))
                        } else {
                            None // Would need wallet connection for address-based
                        },
                        user_id: None,
                        created_at: chrono::Utc::now(),
                    };
                    
                    // Perform recovery
                    match recover_sol(&recovery_request, rpc_endpoint, None).await {
                        Ok(recovery_result) => {
                            // Check the actual recovery status, not just Ok()
                            match recovery_result.status {
                                solana_recover::core::types::RecoveryStatus::Completed => {
                                    total_recovered += recovery_result.net_sol;
                                    total_fees += recovery_result.total_fees_paid as f64 / 1_000_000_000.0;
                                    successful_reclaims += 1;
                                    reclaim_details.push((
                                        scan_address.clone(),
                                        recovery_result.net_sol,
                                        recovery_result.total_fees_paid as f64 / 1_000_000_000.0
                                    ));
                                    println!("✓ {:.9} SOL (fees: {:.9})", recovery_result.net_sol, 
                                           recovery_result.total_fees_paid as f64 / 1_000_000_000.0);
                                }
                                solana_recover::core::types::RecoveryStatus::Failed => {
                                    println!("✗ Reclamation failed: {}", recovery_result.error.unwrap_or_else(|| "Unknown error".to_string()));
                                }
                                _ => {
                                    println!("⚠ Recovery incomplete: {:?}", recovery_result.status);
                                }
                            }
                        }
                        Err(e) => {
                            println!("✗ Recovery failed: {}", e);
                        }
                    }
                } else {
                    println!("No SOL to reclaim");
                }
            }
            Err(e) => {
                println!("✗ Scan failed: {}", e);
            }
        }
    }
    
    Ok(ReclaimResult {
        total_wallets: wallets.len(),
        successful_reclaims,
        total_recovered_sol: total_recovered,
        total_fees_paid: total_fees,
        net_sol: total_recovered - total_fees,
        reclaim_details,
    })
}

fn derive_address_from_private_key(private_key: &str) -> Result<String, Box<dyn std::error::Error>> {
    use solana_sdk::signature::{Keypair, Signer};
    use bs58;
    
    let keypair_bytes = bs58::decode(private_key)
        .into_vec()
        .map_err(|_| "Invalid private key format")?;
    
    let keypair = Keypair::from_bytes(&keypair_bytes)
        .map_err(|_| "Invalid private key")?;
    
    Ok(keypair.pubkey().to_string())
}

fn print_total_claim_result(result: &TotalClaimResult, dev: bool) {
    println!();
    println!("Total Claimable SOL Summary:");
    println!("  Total wallets: {}", result.total_wallets);
    println!("  Total claimable SOL: {:.9}", result.total_recoverable_sol);
    
    if !result.wallet_results.is_empty() && dev {
        println!();
        println!("Wallet Breakdown:");
        for (address, scan_result) in &result.wallet_results {
            println!("  {}: {:.9} SOL ({} empty accounts)", 
                   address, scan_result.recoverable_sol, scan_result.empty_accounts);
        }
    }
    
    if result.total_recoverable_sol > 0.0 {
        println!();
        println!("Total {:.9} SOL is available for recovery!", result.total_recoverable_sol);
    } else {
        println!();
        println!("No SOL available for recovery from any wallets.");
    }
}

fn print_reclaim_result(result: &ReclaimResult, dev: bool) {
    println!();
    println!("SOL Reclamation Summary:");
    println!("  Total wallets: {}", result.total_wallets);
    println!("  Successful reclaims: {}", result.successful_reclaims);
    println!("  Total SOL recovered: {:.9}", result.total_recovered_sol);
    println!("  Total fees paid: {:.9}", result.total_fees_paid);
    println!("  Net SOL received: {:.9}", result.net_sol);
    
    if !result.reclaim_details.is_empty() && dev {
        println!();
        println!("Reclaim Breakdown:");
        for (address, recovered, fees) in &result.reclaim_details {
            println!("  {}: {:.9} SOL (fees: {:.9})", address, recovered, fees);
        }
    }
    
    if result.net_sol > 0.0 {
        println!();
        println!("Successfully reclaimed {:.9} SOL!", result.net_sol);
    } else {
        println!();
        println!("No SOL was reclaimed.");
    }
}

fn print_scan_result(result: &WalletInfo, dev: bool) {
    println!("Scan Results:");
    if dev {
        println!("  Wallet Address: {}", result.address);
    }
    println!("  Total Accounts: {}", result.total_accounts);
    println!("  Empty Accounts: {}", result.empty_accounts);
    println!("  Recoverable SOL: {:.9} SOL", result.recoverable_sol);
    println!("  Scan Time: {}ms", result.scan_time_ms);

    if result.empty_accounts > 0 && dev {
        println!();
        println!("Empty Account Details:");
        for (i, account) in result.empty_account_addresses.iter().enumerate() {
            println!("  {}. {}", i + 1, account);
        }
    }

    if result.recoverable_sol > 0.0 {
        println!();
        println!("This wallet has {:.9} SOL available for recovery!", result.recoverable_sol);
    } else {
        println!();
        println!("No SOL available for recovery from this wallet.");
    }
}
