//! Simple CLI for the solana-recover crate

use clap::{Parser, Subcommand};
use solana_recover::{scan_wallet_ultra_fast, scan_wallet, recover_sol, WalletInfo, RecoveryRequest, ScanMode, UnifiedScanResult, UnifiedTotalClaimResult};
#[cfg(feature = "nft")]
use solana_recover::{scan_wallet_unified, calculate_total_claimable_unified};
use solana_recover::wallet::{WalletManager, WalletCredentials, WalletType, WalletCredentialData};
use solana_sdk::signature::{Signer, SeedDerivable};
use std::io::{self, Write};
use std::sync::Arc;
use zeroize::Zeroize;

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
        /// Scan mode: sol (SOL accounts only), nft (NFT accounts only), or both (default)
        #[arg(long, default_value = "both")]
        mode: String,
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
        /// Scan mode: sol (SOL accounts only), nft (NFT accounts only), or both (default)
        #[arg(long, default_value = "both")]
        mode: String,
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
        /// Scan mode: sol (SOL accounts only), nft (NFT accounts only), or both (default)
        #[arg(long, default_value = "both")]
        mode: String,
    },
    /// Scan multiple wallets from a file
    Batch {
        /// File containing wallet addresses (one per line)
        file: String,
        /// Show detailed developer information (including account addresses)
        #[arg(short, long, default_value_t = false)]
        dev: bool,
        /// Scan mode: sol (SOL accounts only), nft (NFT accounts only), or both (default)
        #[arg(long, default_value = "both")]
        mode: String,
    },
    /// Scan NFTs in a wallet (NFT-only mode with detailed analysis)
    Nft {
        /// The wallet address to scan for NFTs
        address: String,
        /// RPC endpoint to use (defaults to mainnet)
        #[arg(long)]
        rpc_endpoint: Option<String>,
        /// Show detailed NFT information including metadata and valuation
        #[arg(short, long, default_value_t = false)]
        detailed: bool,
        /// Include security analysis (may take longer)
        #[arg(long, default_value_t = false)]
        security: bool,
    },
    /// Batch scan NFTs from multiple wallets
    NftBatch {
        /// File containing wallet addresses (one per line)
        file: String,
        /// RPC endpoint to use (defaults to mainnet)
        #[arg(long)]
        rpc_endpoint: Option<String>,
        /// Show detailed NFT information
        #[arg(short, long, default_value_t = false)]
        detailed: bool,
        /// Include security analysis
        #[arg(long, default_value_t = false)]
        security: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle private key mode (scan + reclaim)
    if let Some(private_key) = &cli.private_key {
        // Derive wallet address from private key using the same parsing logic as PrivateKeyProvider
        let wallet_address = derive_address_from_private_key(private_key)?;
        
        println!("Scanning wallet: {}", wallet_address);
        println!("Using default mainnet endpoint");
        println!();

        let start_time = std::time::Instant::now();
        let result = scan_wallet_ultra_fast(&wallet_address, None).await?;
        let elapsed = start_time.elapsed();

        print!("✓ Ultra-fast scan completed in {}ms\n", elapsed.as_millis());
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
                    eprintln!("Detailed error: {:?}", e);
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
        let result = scan_wallet_ultra_fast(wallet_address, None).await?;
        let elapsed = start_time.elapsed();

        print!("✓ Ultra-fast scan completed in {}ms\n", elapsed.as_millis());
        print_scan_result(&result, cli.dev);
    }

    // Handle subcommands
    match cli.command {
        Some(Commands::Scan { address, rpc_endpoint, dev, mode }) => {
            println!("Scanning wallet: {}", address);
            if let Some(endpoint) = &rpc_endpoint {
                println!("Using RPC endpoint: {}", endpoint);
            } else {
                println!("Using default mainnet endpoint");
            }
            println!();

            let scan_mode = match mode.as_str() {
                "sol" => ScanMode::SolOnly,
                "nft" => ScanMode::NftOnly,
                "both" => ScanMode::Both,
                _ => {
                    eprintln!("Invalid scan mode: {}. Use 'sol', 'nft', or 'both'", mode);
                    return Err("Invalid scan mode".into());
                }
            };
            
            let start_time = std::time::Instant::now();
            let result = {
                #[cfg(feature = "nft")]
                {
                    scan_wallet_unified(&address, rpc_endpoint.as_deref(), scan_mode).await?
                }
                #[cfg(not(feature = "nft"))]
                {
                // Fallback to SOL-only scanning if NFT feature is not enabled
                if matches!(scan_mode, ScanMode::NftOnly) {
                    eprintln!("NFT scanning requires the 'nft' feature to be enabled");
                    return Err("NFT feature not enabled".into());
                }
                let sol_result = scan_wallet_ultra_fast(&address, rpc_endpoint.as_deref()).await?;
                #[cfg(feature = "nft")]
                {
                    UnifiedScanResult {
                        sol_info: Some(sol_result),
                        nft_info: None,
                        scan_mode,
                        total_scan_time_ms: start_time.elapsed().as_millis() as u64,
                        wallet_address: address.clone(),
                    }
                }
                #[cfg(not(feature = "nft"))]
                {
                    UnifiedScanResult {
                        sol_info: Some(sol_result),
                        scan_mode,
                        total_scan_time_ms: start_time.elapsed().as_millis() as u64,
                        wallet_address: address.clone(),
                    }
                }
                }
            };
            let elapsed = start_time.elapsed();

            print!("✓ Unified scan completed in {}ms\n", elapsed.as_millis());
            print_unified_scan_result(&result, dev);
        }
        Some(Commands::Show { targets, rpc_endpoint, dev, mode }) => {
            let scan_mode = match mode.as_str() {
                "sol" => ScanMode::SolOnly,
                "nft" => ScanMode::NftOnly,
                "both" => ScanMode::Both,
                _ => {
                    eprintln!("Invalid scan mode: {}. Use 'sol', 'nft', or 'both'", mode);
                    return Err("Invalid scan mode".into());
                }
            };
            
            println!("Calculating total claimable assets...");
            if let Some(endpoint) = &rpc_endpoint {
                println!("Using RPC endpoint: {}", endpoint);
            } else {
                println!("Using default mainnet endpoint");
            }
            println!("Scan mode: {:?}", scan_mode);
            println!();

            let start_time = std::time::Instant::now();
            let total = {
                #[cfg(feature = "nft")]
                {
                    calculate_total_claimable_unified(&targets, rpc_endpoint.as_deref(), dev, scan_mode).await?
                }
                #[cfg(not(feature = "nft"))]
                {
                // Fallback to SOL-only calculation if NFT feature is not enabled
                if matches!(scan_mode, ScanMode::NftOnly) {
                    eprintln!("NFT scanning requires the 'nft' feature to be enabled");
                    return Err("NFT feature not enabled".into());
                }
                calculate_total_claimable(&targets, rpc_endpoint.as_deref(), dev).await?
                }
            };
            let elapsed = start_time.elapsed();

            print!("✓ Calculation completed in {}ms\n", elapsed.as_millis());
            {
                #[cfg(feature = "nft")]
                {
                    print_total_claim_result_unified(&total, dev);
                }
                #[cfg(not(feature = "nft"))]
                {
                    print_total_claim_result(&total, dev);
                }
            }
        }
        Some(Commands::Reclaim { targets, destination, rpc_endpoint, force, dev, mode: _ }) => {
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
        Some(Commands::Batch { file, dev, mode: _ }) => {
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
        #[cfg(feature = "nft")]
        Some(Commands::Nft { address, rpc_endpoint, detailed, security }) => {
            println!("Scanning NFTs for wallet: {}", address);
            if let Some(endpoint) = &rpc_endpoint {
                println!("Using RPC endpoint: {}", endpoint);
            } else {
                println!("Using default mainnet endpoint");
            }
            println!("Detailed analysis: {}", if detailed { "enabled" } else { "disabled" });
            println!("Security analysis: {}", if security { "enabled" } else { "disabled" });
            println!();

            let start_time = std::time::Instant::now();
            let result = scan_wallet_unified(&address, rpc_endpoint.as_deref(), ScanMode::NftOnly).await?;
            let elapsed = start_time.elapsed();

            print!("✓ NFT scan completed in {}ms\n", elapsed.as_millis());
            if let Some(nft_info) = result.nft_info {
                print_nft_scan_result(&nft_info, detailed);
            } else {
                println!("No NFT results available. NFT scanning may not be properly configured.");
            }
        }
        #[cfg(feature = "nft")]
        Some(Commands::NftBatch { file, rpc_endpoint, detailed, security }) => {
            println!("Batch scanning NFTs from file: {}", file);
            if let Some(endpoint) = &rpc_endpoint {
                println!("Using RPC endpoint: {}", endpoint);
            } else {
                println!("Using default mainnet endpoint");
            }
            println!("Detailed analysis: {}", if detailed { "enabled" } else { "disabled" });
            println!("Security analysis: {}", if security { "enabled" } else { "disabled" });
            println!();

            // Read wallet addresses from file
            let content = std::fs::read_to_string(&file)?;
            let wallets: Vec<String> = content.lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect();

            println!("Found {} wallet addresses to scan for NFTs", wallets.len());
            println!();

            let mut total_nfts = 0usize;
            let mut total_value = 0u64;
            let mut successful_scans = 0;

            for (i, wallet) in wallets.iter().enumerate() {
                let wallet_display = if true { wallet.clone() } else { "[wallet address hidden]".to_string() };
                print!("Scanning NFTs for wallet {}/{}: {} ... ", i + 1, wallets.len(), wallet_display);
                io::stdout().flush()?;

                match scan_wallet_unified(wallet, rpc_endpoint.as_deref(), ScanMode::NftOnly).await {
                    Ok(result) => {
                        if let Some(nft_info) = result.nft_info {
                            successful_scans += 1;
                            total_nfts += nft_info.nfts.len();
                            total_value += nft_info.total_estimated_value_lamports;
                            println!("✓ {} NFTs ({:.9} SOL)", nft_info.nfts.len(), 
                                nft_info.total_estimated_value_lamports as f64 / 1_000_000_000.0);
                        } else {
                            println!("✗ No NFT data");
                        }
                    }
                    Err(e) => {
                        println!("✗ {}", e);
                    }
                }
            }

            println!();
            println!("Batch NFT Scan Summary:");
            println!("  Total wallets: {}", wallets.len());
            println!("  Successful scans: {}", successful_scans);
            println!("  Failed scans: {}", wallets.len() - successful_scans);
            println!("  Total NFTs found: {}", total_nfts);
            println!("  Total estimated value: {:.9} SOL", total_value as f64 / 1_000_000_000.0);
        }
        #[cfg(not(feature = "nft"))]
        Some(Commands::Nft { address: _, .. }) => {
            eprintln!("NFT scanning requires the 'nft' feature to be enabled");
            eprintln!("Please rebuild with: cargo build --features nft");
            return Err("NFT feature not enabled".into());
        }
        #[cfg(not(feature = "nft"))]
        Some(Commands::NftBatch { file: _, .. }) => {
            eprintln!("NFT batch scanning requires the 'nft' feature to be enabled");
            eprintln!("Please rebuild with: cargo build --features nft");
            return Err("NFT feature not enabled".into());
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
        
        match scan_wallet_ultra_fast(&scan_address, rpc_endpoint).await {
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
        match scan_wallet_ultra_fast(&scan_address, rpc_endpoint).await {
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
                        wallet_connection_id: None, // Will be set properly when wallet connection is established
                        user_id: None,
                        created_at: chrono::Utc::now(),
                    };
                    
                    // Create wallet manager and establish connection for private keys
                    let wallet_manager = if is_private_keys {
                        Some(Arc::new(WalletManager::new()))
                    } else {
                        None
                    };
                    
                    let connection_id = if is_private_keys {
                        let credentials = WalletCredentials {
                            wallet_type: WalletType::PrivateKey,
                            credentials: WalletCredentialData::PrivateKey {
                                private_key: wallet.clone(),
                            },
                        };
                        let connection = wallet_manager.as_ref().unwrap().connect_wallet(credentials).await?;
                        Some(connection.id)
                    } else {
                        None
                    };
                    
                    // Update recovery request with proper connection ID
                    let mut recovery_request = recovery_request;
                    recovery_request.wallet_connection_id = connection_id;
                    
                    // Perform recovery
                    match recover_sol(&recovery_request, rpc_endpoint, wallet_manager).await {
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
                            eprintln!("Detailed error: {:?}", e);
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
    use solana_sdk::signature::Keypair;
    use bs58;
    
    // Try different formats: base58, hex, or array format (same logic as PrivateKeyProvider)
    let mut key_bytes = None;
    
    // Try base58 format (most common for Solana)
    if let Ok(bytes) = bs58::decode(private_key).into_vec() {
        if bytes.len() == 64 {
            key_bytes = Some(bytes);
        }
    }
    
    // Try hex format
    if key_bytes.is_none() {
        let hex_str = private_key.strip_prefix("0x").unwrap_or(private_key);
        if let Ok(bytes) = hex::decode(hex_str) {
            if bytes.len() == 32 {
                // For 32-byte seeds, we need to create a keypair
                if let Ok(kp) = Keypair::from_seed(&bytes) {
                    return Ok(kp.pubkey().to_string());
                }
            } else if bytes.len() == 64 {
                key_bytes = Some(bytes);
            }
        }
    }
    
    // Try JSON array format
    if key_bytes.is_none() {
        if let Ok(bytes_vec) = serde_json::from_str::<Vec<u8>>(private_key) {
            if bytes_vec.len() == 32 {
                if let Ok(kp) = Keypair::from_seed(&bytes_vec) {
                    return Ok(kp.pubkey().to_string());
                }
            } else if bytes_vec.len() == 64 {
                key_bytes = Some(bytes_vec);
            }
        }
    }
    
    // If we have 64-byte keypair data, use it directly
    if let Some(mut bytes) = key_bytes {
        let result = Keypair::from_bytes(&bytes);
        bytes.zeroize(); // Immediately zeroize after use
        let keypair = result.map_err(|_| "Invalid private key format. Expected base58, hex, or array format.")?;
        return Ok(keypair.pubkey().to_string());
    }
    
    Err("Invalid private key format. Expected base58, hex, or array format.".into())
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

fn print_unified_scan_result(result: &UnifiedScanResult, dev: bool) {
    println!();
    println!("Unified Scan Results:");
    println!("  Scan Mode: {:?}", result.scan_mode);
    println!("  Wallet Address: {}", result.wallet_address);
    println!("  Total Scan Time: {}ms", result.total_scan_time_ms);
    println!();
    
    // Print SOL results if available
    if let Some(sol_info) = &result.sol_info {
        println!("SOL Account Results:");
        println!("  Total Accounts: {}", sol_info.total_accounts);
        println!("  Empty Accounts: {}", sol_info.empty_accounts);
        println!("  Recoverable SOL: {:.9} SOL", sol_info.recoverable_sol);
        println!("  SOL Scan Time: {}ms", sol_info.scan_time_ms);
        
        if sol_info.empty_accounts > 0 && dev {
            println!("  Empty Account Details:");
            for (i, account) in sol_info.empty_account_addresses.iter().enumerate() {
                println!("    {}. {}", i + 1, account);
            }
        }
        println!();
    }
    
    // Print NFT results if available
    #[cfg(feature = "nft")]
    if let Some(nft_info) = &result.nft_info {
        println!("NFT Results:");
        println!("  Total NFTs: {}", nft_info.nfts.len());
        println!("  Verified NFTs: {}", nft_info.statistics.verified_nfts);
        println!("  Unverified NFTs: {}", nft_info.statistics.unverified_nfts);
        println!("  NFTs with Security Issues: {}", nft_info.statistics.nfts_with_security_issues);
        println!("  Unique Collections: {}", nft_info.statistics.unique_collections);
        println!("  Total Estimated Value: {:.9} SOL", nft_info.total_estimated_value_lamports as f64 / 1_000_000_000.0);
        println!("  NFT Scan Time: {}ms", nft_info.scan_duration_ms);
        
        if dev && !nft_info.nfts.is_empty() {
            println!("  NFT Details:");
            for (i, nft) in nft_info.nfts.iter().take(10).enumerate() {
                println!("    {}. {} - {}", i + 1, 
                    nft.name.as_ref().unwrap_or(&"Unknown".to_string()),
                    nft.mint_address
                );
                if let Some(collection) = &nft.collection {
                    println!("       Collection: {}{}", 
                        collection.name,
                        if collection.verified { " ✓" } else { " " }
                    );
                }
                if let Some(value) = nft.estimated_value_lamports {
                    println!("       Estimated Value: {:.9} SOL", value as f64 / 1_000_000_000.0);
                }
            }
            if nft_info.nfts.len() > 10 {
                println!("    ... and {} more NFTs", nft_info.nfts.len() - 10);
            }
        }
        println!();
    }
    
    // Summary
    let mut has_recoverable_assets = false;
    if let Some(sol_info) = &result.sol_info {
        if sol_info.recoverable_sol > 0.0 {
            has_recoverable_assets = true;
        }
    }
    
    #[cfg(feature = "nft")]
    if let Some(nft_info) = &result.nft_info {
        if nft_info.total_estimated_value_lamports > 0 {
            has_recoverable_assets = true;
        }
    }
    
    if has_recoverable_assets {
        println!("Summary: This wallet has recoverable assets!");
    } else {
        println!("Summary: No recoverable assets found.");
    }
}

#[cfg(feature = "nft")]
fn print_nft_scan_result(result: &solana_recover::NftScanResult, detailed: bool) {
    println!();
    println!("NFT Scan Results:");
    println!("  Wallet Address: {}", result.wallet_address);
    println!("  Total NFTs: {}", result.nfts.len());
    println!("  Verified NFTs: {}", result.statistics.verified_nfts);
    println!("  Unverified NFTs: {}", result.statistics.unverified_nfts);
    println!("  NFTs with Security Issues: {}", result.statistics.nfts_with_security_issues);
    println!("  Unique Collections: {}", result.statistics.unique_collections);
    println!("  Total Estimated Value: {:.9} SOL", result.total_estimated_value_lamports as f64 / 1_000_000_000.0);
    println!("  Scan Duration: {}ms", result.scan_duration_ms);
    
    if detailed && !result.nfts.is_empty() {
        println!();
        println!("Detailed NFT Information:");
        for (i, nft) in result.nfts.iter().enumerate() {
            println!("\n{}. {}", i + 1, nft.name.as_ref().unwrap_or(&"Unknown".to_string()));
            println!("   Mint Address: {}", nft.mint_address);
            println!("   Symbol: {}", nft.symbol.as_ref().unwrap_or(&"N/A".to_string()));
            
            if let Some(collection) = &nft.collection {
                println!("   Collection: {}{}", 
                    collection.name,
                    if collection.verified { " ✓" } else { " " }
                );
            }
            
            if let Some(description) = &nft.description {
                let short_desc = if description.len() > 200 {
                    format!("{}...", &description[..200])
                } else {
                    description.clone()
                };
                println!("   Description: {}", short_desc);
            }
            
            if let Some(value) = nft.estimated_value_lamports {
                println!("   Estimated Value: {:.9} SOL", value as f64 / 1_000_000_000.0);
            }
            
            println!("   Metadata Verified: {}", if nft.metadata_verified { "✓" } else { "✗" });
            println!("   Image Verified: {}", if nft.image_verified { "✓" } else { "✗" });
            println!("   Security Risk Level: {:?}", nft.security_assessment.risk_level);
            
            if !nft.creators.is_empty() {
                println!("   Creators:");
                for creator in &nft.creators {
                    println!("     - {}{} ({})", 
                        creator.address,
                        if creator.verified { " ✓" } else { " " },
                        creator.share
                    );
                }
            }
            
            if !nft.attributes.is_empty() {
                println!("   Attributes:");
                for attr in &nft.attributes {
                    println!("     - {}: {}", attr.trait_type, attr.value);
                }
            }
        }
    }
    
    if !result.security_issues.is_empty() {
        println!();
        println!("Security Issues Found:");
        for issue in &result.security_issues {
            println!("  [{}] {}: {}", 
                match issue.severity {
                    solana_recover::nft::types::RiskLevel::High => "HIGH",
                    solana_recover::nft::types::RiskLevel::Medium => "MED",
                    solana_recover::nft::types::RiskLevel::Low => "LOW",
                    solana_recover::nft::types::RiskLevel::None => "NONE",
                },
                issue.issue_type,
                issue.description
            );
        }
    }
}

#[allow(dead_code)]
fn print_total_claim_result_unified(result: &UnifiedTotalClaimResult, dev: bool) {
    println!();
    println!("Unified Total Claimable Assets Summary:");
    println!("  Scan Mode: {:?}", result.scan_mode);
    println!("  Total wallets: {}", result.total_wallets);
    println!("  Total recoverable SOL: {:.9}", result.total_recoverable_sol);
    
    #[cfg(feature = "nft")]
    {
        println!("  Total NFTs: {}", result.total_nfts);
        println!("  Total NFT value: {:.9} SOL", result.total_nft_value_lamports as f64 / 1_000_000_000.0);
        
        let total_value = result.total_recoverable_sol + (result.total_nft_value_lamports as f64 / 1_000_000_000.0);
        println!("  Total asset value: {:.9} SOL", total_value);
    }
    
    if !result.wallet_results.is_empty() && dev {
        println!();
        println!("Wallet Breakdown:");
        for (address, scan_result) in &result.wallet_results {
            println!("  {}:", address);
            
            if let Some(sol_info) = &scan_result.sol_info {
                println!("    SOL: {:.9} ({} empty accounts)", 
                    sol_info.recoverable_sol, sol_info.empty_accounts);
            }
            
            #[cfg(feature = "nft")]
            if let Some(nft_info) = &scan_result.nft_info {
                println!("    NFTs: {} ({:.9} SOL)", 
                    nft_info.nfts.len(),
                    nft_info.total_estimated_value_lamports as f64 / 1_000_000_000.0);
            }
        }
    }
    
    let has_recoverable_assets = result.total_recoverable_sol > 0.0;
    #[cfg(feature = "nft")]
    {
        has_recoverable_assets = has_recoverable_assets || result.total_nft_value_lamports > 0;
    }
    
    if has_recoverable_assets {
        println!();
        println!("Total {:.9} SOL in assets is available!", 
            result.total_recoverable_sol + (result.total_nft_value_lamports as f64 / 1_000_000_000.0));
    } else {
        println!();
        println!("No recoverable assets found from any wallets.");
    }
}
