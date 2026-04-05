use clap::{Parser, Subcommand};
use solana_recover::{
    Config, WalletScanner, ConnectionPool, BatchProcessor, 
    CacheManager, SqlitePersistenceManager, RpcEndpoint, RecoveryManager, RecoveryConfig
};
use solana_recover::wallet::WalletManager;
use std::sync::Arc;
use tracing::{info, error, warn};

#[derive(Parser)]
#[command(name = "solana-recover")]
#[command(about = "A scalable Solana wallet scanner for finding recoverable SOL")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,
    
    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a single wallet for recoverable SOL
    Scan {
        /// Wallet address to scan
        #[arg(help = "Solana wallet public key")]
        address: String,
        
        /// Output format (json, table)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Scan multiple wallets from a file
    Batch {
        /// File containing wallet addresses (one per line)
        #[arg(help = "Path to file with wallet addresses")]
        file: String,
        
        /// Output directory for results
        #[arg(short, long, default_value = "./results")]
        output: String,
    },
    /// Recover SOL from empty accounts
    Recover {
        /// Wallet address containing empty accounts
        #[arg(help = "Source wallet address")]
        wallet_address: String,
        
        /// Destination wallet address
        #[arg(help = "Destination wallet address")]
        destination: String,
        
        /// File containing empty account addresses (one per line)
        #[arg(help = "Path to file with empty account addresses")]
        accounts_file: String,
        
        /// Wallet connection ID for signing
        #[arg(long)]
        connection_id: Option<String>,
        
        /// Maximum fee in lamports
        #[arg(long)]
        max_fee: Option<u64>,
        
        /// Priority fee in lamports
        #[arg(long)]
        priority_fee: Option<u64>,
        
        /// Output format (json, table)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Estimate recovery fees
    EstimateFees {
        /// File containing empty account addresses (one per line)
        #[arg(help = "Path to file with empty account addresses")]
        accounts_file: String,
    },
    /// Start the API server
    Server {
        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: Option<String>,
        
        /// Port to bind to
        #[arg(short = 'p', long)]
        port: Option<u16>,
    },
    /// Show configuration
    Config {
        /// Show current configuration
        #[arg(short, long)]
        show: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Load configuration
    let mut config = if let Some(config_path) = &cli.config {
        Config::from_file(config_path)?
    } else {
        Config::load()?
    };
    
    // Override log level from CLI
    config.logging.level = cli.log_level.clone();
    
    // Validate configuration
    config.validate()?;
    
    // Initialize logging
    init_logging(&config.logging)?;
    
    info!("Starting Solana Recover v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration loaded successfully");
    
    // Initialize core components
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
    
    let connection_pool = Arc::new(ConnectionPool::new(rpc_endpoints, config.rpc.pool_size));
    let cache_manager = Arc::new(CacheManager::new(config.cache.clone().into()));
    let persistence_manager = Arc::new(
        SqlitePersistenceManager::new(config.database.clone().into()).await?
    );
    let wallet_manager = Arc::new(WalletManager::new());
    
    let scanner = Arc::new(WalletScanner::new(connection_pool.clone()));
    let batch_processor = Arc::new(BatchProcessor::new(
        scanner.clone(),
        Some(cache_manager.clone()),
        Some(persistence_manager.clone()),
        config.scanner.clone().into(),
    ));
    
    let recovery_config = RecoveryConfig::default();
    let recovery_manager = Arc::new(RecoveryManager::new(
        connection_pool.clone(),
        wallet_manager.clone(),
        recovery_config,
    ));
    
    // Execute command
    match cli.command {
        Commands::Scan { address, format } => {
            info!("Scanning wallet: {}", address);
            
            match scanner.scan_wallet(&address).await {
                Ok(result) => {
                    match format.as_str() {
                        "json" => {
                            println!("{}", serde_json::to_string_pretty(&result)?);
                        }
                        "table" => {
                            print_scan_result(&result);
                        }
                        _ => {
                            error!("Unsupported format: {}", format);
                            return Err("Unsupported format".into());
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to scan wallet: {}", e);
                    return Err(e.into());
                }
            }
        }
        
        Commands::Batch { file, output } => {
            info!("Starting batch scan from file: {}", file);
            
            // Read wallet addresses from file
            let addresses = std::fs::read_to_string(&file)?
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect::<Vec<String>>();
            
            if addresses.is_empty() {
                warn!("No wallet addresses found in file: {}", file);
                return Ok(());
            }
            
            info!("Found {} wallet addresses to scan", addresses.len());
            
            // Create output directory
            std::fs::create_dir_all(&output)?;
            
            // Execute batch scan
            let batch_request = solana_recover::BatchScanRequest {
                id: uuid::Uuid::new_v4(),
                wallet_addresses: addresses,
                user_id: Some("cli_user".to_string()),
                fee_percentage: None,
                created_at: chrono::Utc::now(),
            };
            
            match batch_processor.process_batch(&batch_request).await {
                Ok(result) => {
                    // Save results to file
                    let results_file = format!("{}/batch_results.json", output);
                    std::fs::write(&results_file, serde_json::to_string_pretty(&result)?)?;
                    
                    info!("Batch scan completed successfully");
                    info!("Results saved to: {}", results_file);
                    print_batch_summary(&result);
                }
                Err(e) => {
                    error!("Batch scan failed: {}", e);
                    return Err(e.into());
                }
            }
        }
        
        Commands::Server { host, port } => {
            info!("Starting API server");
            
            // Override host/port from CLI if provided
            if let Some(host) = host {
                config.server.host = host;
            }
            if let Some(port) = port {
                config.server.port = port;
            }
            
            // Start API server
            let api_state = solana_recover::api::server::ApiState {
                scanner: scanner.clone(),
                batch_processor: batch_processor.clone(),
                recovery_manager: recovery_manager.clone(),
                wallet_manager: wallet_manager.clone(),
                cache_manager: cache_manager.clone(),
                persistence_manager: persistence_manager.clone(),
                config: config.clone(),
            };
            
            let server = solana_recover::api::server::start_server(api_state, &config.server).await?;
            
            info!("API server started on {}:{}", config.server.host, config.server.port);
            
            // Wait for shutdown signal
            tokio::signal::ctrl_c().await?;
            info!("Received shutdown signal");
            
            server.shutdown().await?;
            info!("Server shutdown complete");
        }
        
        Commands::Config { show } => {
            if show {
                println!("Current Configuration:");
                println!("{}", serde_json::to_string_pretty(&config)?);
            }
        }
        
        Commands::Recover { 
            wallet_address, 
            destination, 
            accounts_file, 
            connection_id, 
            max_fee, 
            priority_fee, 
            format 
        } => {
            info!("Starting SOL recovery for wallet: {}", wallet_address);
            
            // Read empty account addresses from file
            let empty_accounts = std::fs::read_to_string(&accounts_file)?
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect::<Vec<String>>();
            
            if empty_accounts.is_empty() {
                warn!("No empty account addresses found in file: {}", accounts_file);
                return Ok(());
            }
            
            info!("Found {} empty accounts to recover", empty_accounts.len());
            
            // Create recovery request
            let recovery_request = solana_recover::RecoveryRequest {
                id: uuid::Uuid::new_v4(),
                wallet_address,
                empty_accounts,
                destination_address: destination,
                wallet_connection_id: connection_id,
                max_fee_lamports: max_fee,
                priority_fee_lamports: priority_fee,
                user_id: Some("cli_user".to_string()),
                created_at: chrono::Utc::now(),
            };
            
            // Validate request
            recovery_manager.validate_recovery_request(&recovery_request).await?;
            
            // Execute recovery
            match recovery_manager.recover_sol(&recovery_request).await {
                Ok(result) => {
                    match format.as_str() {
                        "json" => {
                            println!("{}", serde_json::to_string_pretty(&result)?);
                        }
                        "table" => {
                            print_recovery_result(&result);
                        }
                        _ => {
                            error!("Unsupported format: {}", format);
                            return Err("Unsupported format".into());
                        }
                    }
                }
                Err(e) => {
                    error!("Recovery failed: {}", e);
                    return Err(e.into());
                }
            }
        }
        
        Commands::EstimateFees { accounts_file } => {
            info!("Estimating recovery fees for accounts in: {}", accounts_file);
            
            // Read empty account addresses from file
            let empty_accounts = std::fs::read_to_string(&accounts_file)?
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect::<Vec<String>>();
            
            if empty_accounts.is_empty() {
                warn!("No empty account addresses found in file: {}", accounts_file);
                return Ok(());
            }
            
            match recovery_manager.estimate_recovery_fees(&empty_accounts).await {
                Ok(fees) => {
                    println!("Estimated recovery fees: {} lamports ({:.9} SOL)", fees, fees as f64 / 1_000_000_000.0);
                }
                Err(e) => {
                    error!("Fee estimation failed: {}", e);
                    return Err(e.into());
                }
            }
        }
    }
    
    Ok(())
}

fn init_logging(config: &solana_recover::config::LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    use std::str::FromStr;
    
    // For now, use a simple logging setup
    // In a real implementation, you would convert between config types
    let level = tracing::Level::from_str(&config.level)
        .unwrap_or(tracing::Level::INFO);
    
    tracing_subscriber::fmt()
        .with_max_level(level)
        .pretty()
        .init();
    
    Ok(())
}

fn print_scan_result(result: &solana_recover::ScanResult) {
    println!("============================================");
    println!(" Scan Results");
    println!("============================================");
    println!("Scan ID: {}", result.id);
    println!("Wallet:  {}", result.wallet_address);
    println!("Status:  {:?}", result.status);
    println!("Created: {}", result.created_at);
    
    if let Some(wallet_info) = &result.result {
        println!();
        println!("Total token accounts:   {}", wallet_info.total_accounts);
        println!("Empty accounts:         {}", wallet_info.empty_accounts);
        println!(
            "Recoverable SOL:        {:.9} SOL  ({} lamports)",
            wallet_info.recoverable_sol, wallet_info.recoverable_lamports
        );
        
        if !wallet_info.empty_account_addresses.is_empty() {
            println!("\nEmpty account addresses:");
            for (i, addr) in wallet_info.empty_account_addresses.iter().enumerate() {
                println!("  {}. {}", i + 1, addr);
            }
        }
    }
    
    if let Some(error) = &result.error {
        println!("\nError: {}", error);
    }
    
    println!("============================================");
}

fn print_batch_summary(result: &solana_recover::BatchScanResult) {
    println!("============================================");
    println!(" Batch Scan Summary");
    println!("============================================");
    println!("Batch ID:      {:?}", result.batch_id);
    println!("Total wallets: {}", result.total_wallets);
    println!("Successful:    {}", result.successful_scans);
    println!("Failed:        {}", result.failed_scans);
    
    let total_recoverable: f64 = result.results
        .iter()
        .filter_map(|r| r.result.as_ref())
        .map(|w| w.recoverable_sol)
        .sum();
    
    println!("Total recoverable SOL: {:.9}", total_recoverable);
    println!("Duration: {:?}ms", result.duration_ms);
    println!("============================================");
}

fn print_recovery_result(result: &solana_recover::RecoveryResult) {
    println!("============================================");
    println!(" Recovery Results");
    println!("============================================");
    println!("Recovery ID:  {}", result.id);
    println!("Wallet:       {}", result.wallet_address);
    println!("Status:       {:?}", result.status);
    println!("Created:      {}", result.created_at);
    
    if let Some(completed_at) = result.completed_at {
        println!("Completed:    {}", completed_at);
    }
    
    if let Some(duration) = result.duration_ms {
        println!("Duration:     {}ms", duration);
    }
    
    println!();
    println!("Accounts recovered: {}", result.total_accounts_recovered);
    println!("Total recovered:    {:.9} SOL  ({} lamports)", 
             result.net_sol, result.net_lamports);
    println!("Total fees paid:    {:.9} SOL  ({} lamports)", 
             result.total_fees_paid as f64 / 1_000_000_000.0, result.total_fees_paid);
    
    if !result.transactions.is_empty() {
        println!("\nTransactions:");
        for (i, tx) in result.transactions.iter().enumerate() {
            println!("  {}. {} - {} accounts, {:.9} SOL recovered", 
                     i + 1, 
                     tx.transaction_signature.chars().take(16).collect::<String>() + "...",
                     tx.accounts_recovered.len(),
                     tx.lamports_recovered as f64 / 1_000_000_000.0);
        }
    }
    
    if let Some(error) = &result.error {
        println!("\nError: {}", error);
    }
    
    println!("============================================");
}
