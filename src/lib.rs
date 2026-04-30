//! # Solana Recover
//!
//! A high-performance Solana wallet scanner and SOL recovery library.
//! 
//! This library provides functionality to:
//! - Scan Solana wallets for empty token accounts
//! - Calculate recoverable SOL from empty accounts
//! - Perform automated SOL recovery operations
//! - Handle batch processing of multiple wallets
//! - Provide connection pooling and caching for optimal performance
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use solana_recover::{scan_wallet};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let result = scan_wallet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await?;
//!     
//!     println!("Found {} recoverable SOL", result.recoverable_sol);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Feature Flags
//!
//! - `default`: Enables `scanner` and `client` features
//! - `scanner`: Core wallet scanning functionality
//! - `client`: HTTP client for external APIs
//! - `api`: REST API server functionality
//! - `full`: Enables all features
//! - `database`: Database persistence support
//! - `cache`: Advanced caching capabilities
//! - `metrics`: Prometheus metrics collection
//! - `security`: Enhanced security features
//! - `config`: Configuration file support

#![cfg_attr(docsrs, feature(doc_cfg))]

// Core dependencies
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Export core modules
pub mod core;
pub mod rpc;
pub mod storage;

pub mod wallet;
pub mod utils;
pub mod config;
pub mod api;

// NFT module (when feature is enabled)
#[cfg(feature = "nft")]
pub mod nft;

// Re-export commonly used types
pub use core::*;

/// Re-export NFT types when feature is enabled
#[cfg(feature = "nft")]
pub use nft::scanner::NftScanResult;
#[cfg(feature = "nft")]
pub use nft::types::NftInfo;

/// Unified scan result containing both SOL and NFT information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedScanResult {
    /// SOL wallet information
    pub sol_info: Option<WalletInfo>,
    /// NFT scan information
    #[cfg(feature = "nft")]
    pub nft_info: Option<NftScanResult>,
    /// Scan mode used
    pub scan_mode: ScanMode,
    /// Total scan duration in milliseconds
    pub total_scan_time_ms: u64,
    /// Wallet address scanned
    pub wallet_address: String,
}

/// Scan modes for different types of scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanMode {
    /// Scan SOL accounts only
    SolOnly,
    /// Scan NFT accounts only
    NftOnly,
    /// Scan both SOL and NFT accounts
    Both,
}

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default RPC endpoint for mainnet
pub const DEFAULT_MAINNET_ENDPOINT: &str = "https://api.mainnet-beta.solana.com";

/// Default RPC endpoint for devnet
pub const DEFAULT_DEVNET_ENDPOINT: &str = "https://api.devnet.solana.com";

/// Represents an empty token account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyAccount {
    /// The account address
    pub address: String,
    /// The mint address of token
    pub mint: String,
    /// The owner address
    pub owner: String,
    /// Balance in lamports (recoverable amount after rent exemption)
    pub lamports: u64,
}

/// Ultra-fast unified wallet scanning with SOL and NFT support
/// 
/// This function provides the fastest possible wallet scanning using:
/// - Predictive prefetching
/// - Connection multiplexing
/// - Smart batching
/// - Fast path scanning for common patterns
/// - Maximum parallelization
/// - Unified SOL and NFT scanning
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// * `scan_mode` - Scan mode: sol, nft, or both
/// 
/// # Returns
/// 
/// Returns a `UnifiedScanResult` containing both SOL and NFT scan results.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::{scan_wallet_unified, ScanMode};
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = scan_wallet_unified("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None, ScanMode::Both).await?;
///     if let Some(sol_info) = &result.sol_info {
///         println!("Found {} recoverable SOL", sol_info.recoverable_sol);
///     }
///     if let Some(nft_info) = &result.nft_info {
///         println!("Found {} NFTs", nft_info.nfts.len());
///     }
///     Ok(())
/// }
/// ```
#[cfg(feature = "nft")]
pub async fn scan_wallet_unified(
    wallet_address: &str,
    rpc_endpoint: Option<&str>,
    scan_mode: ScanMode,
) -> core::Result<UnifiedScanResult> {
    use core::ScannerFactory;
    use core::unified_scanner::UnifiedScannerConfig;
    use core::unified_scanner::PerformanceMode;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use nft::scanner::{NftScanner, NftScannerConfig};
    use std::sync::Arc;
    
    let start_time = std::time::Instant::now();
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 200, // Higher rate limit for ultra-fast
        timeout_ms: 5000,     // Shorter timeout for ultra-fast
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 16));
    
    // Initialize results
    let mut sol_info = None;
    let mut nft_info = None;
    
    // Scan SOL accounts if requested
    if matches!(scan_mode, ScanMode::SolOnly | ScanMode::Both) {
        let config = UnifiedScannerConfig {
            performance_mode: PerformanceMode::UltraFast,
            max_concurrent_scans: 500,
            scan_timeout: std::time::Duration::from_secs(2),
            batch_size: 100,
            enable_optimizations: true,
            enable_caching: true,
            enable_parallel_processing: true,
        };
        
        let scanner = ScannerFactory::create_with_config(connection_pool.clone(), config)?;
        let scan_result = scanner.scan_wallet(wallet_address).await?;
        sol_info = scan_result.result;
    }
    
    // Scan NFT accounts if requested
    if matches!(scan_mode, ScanMode::NftOnly | ScanMode::Both) {
        let nft_config = NftScannerConfig {
            performance_mode: nft::types::PerformanceMode::UltraFast,
            max_concurrent_scans: 50,
            scan_timeout_seconds: 60,
            enable_batch_processing: true,
            ..Default::default()
        };
        
        let nft_scanner = NftScanner::new(connection_pool.clone(), nft_config)?;
        nft_info = Some(nft_scanner.scan_wallet_nfts(wallet_address).await?);
    }
    
    let total_scan_time_ms = start_time.elapsed().as_millis() as u64;
    
    #[cfg(feature = "nft")]
    {
        Ok(UnifiedScanResult {
            sol_info,
            nft_info,
            scan_mode,
            total_scan_time_ms,
            wallet_address: wallet_address.to_string(),
        })
    }
    #[cfg(not(feature = "nft"))]
    {
        Ok(UnifiedScanResult {
            sol_info,
            scan_mode,
            total_scan_time_ms,
            wallet_address: wallet_address.to_string(),
        })
    }
}

/// Calculate total claimable assets for multiple wallets with unified scanning
#[cfg(feature = "nft")]
pub async fn calculate_total_claimable_unified(
    targets: &str,
    rpc_endpoint: Option<&str>,
    dev: bool,
    scan_mode: ScanMode,
) -> core::Result<UnifiedTotalClaimResult> {
    let (wallets, _is_private_key) = parse_targets_wrapper(targets)?;
    
    let mut total_recoverable_sol = 0.0;
    let mut total_nfts = 0usize;
    let mut total_nft_value = 0u64;
    let mut wallet_results = Vec::new();
    
    for wallet_address in wallets {
        let scan_result = scan_wallet_unified(&wallet_address, rpc_endpoint, scan_mode.clone()).await?;
        
        if let Some(sol_info) = &scan_result.sol_info {
            total_recoverable_sol += sol_info.recoverable_sol;
        }
        
        if let Some(nft_info) = &scan_result.nft_info {
            total_nfts += nft_info.nfts.len();
            total_nft_value += nft_info.total_estimated_value_lamports;
        }
        
        if dev {
            wallet_results.push((wallet_address, scan_result));
        }
    }
    
    Ok(UnifiedTotalClaimResult {
        total_wallets: wallet_results.len(),
        total_recoverable_sol,
        total_nfts,
        total_nft_value_lamports: total_nft_value,
        wallet_results,
        scan_mode,
    })
}

/// Parse targets string into wallet addresses
/// Wrapper function to parse targets in the same format as the CLI
pub fn parse_targets_wrapper(targets: &str) -> core::Result<(Vec<String>, bool)> {
    if targets.starts_with("wallet:") {
        let addresses = targets.strip_prefix("wallet:")
            .unwrap()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok((addresses, false))
    } else if targets.starts_with("key:") {
        // For private keys, we would need to derive addresses
        // This is a placeholder - in a real implementation, you'd parse the private keys
        // and derive the corresponding wallet addresses
        Err(core::SolanaRecoverError::InternalError("Private key parsing not implemented in unified scanner".to_string()))
    } else {
        // Assume it's a single wallet address
        Ok((vec![targets.trim().to_string()], false))
    }
}

/// Unified total claim result containing both SOL and NFT information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTotalClaimResult {
    /// Total wallets scanned
    pub total_wallets: usize,
    /// Total recoverable SOL
    pub total_recoverable_sol: f64,
    /// Total NFTs found
    pub total_nfts: usize,
    /// Total NFT value in lamports
    pub total_nft_value_lamports: u64,
    /// Individual wallet results (only included if dev mode is enabled)
    pub wallet_results: Vec<(String, UnifiedScanResult)>,
    /// Scan mode used
    pub scan_mode: ScanMode,
}
/// 
/// This function provides the fastest possible wallet scanning using:
/// - Predictive prefetching
/// - Connection multiplexing
/// - Smart batching
/// - Fast path scanning for common patterns
/// - Maximum parallelization
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// 
/// # Returns
/// 
/// Returns a `WalletInfo` containing scan results in sub-second time.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::scan_wallet_ultra_fast;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = scan_wallet_ultra_fast("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await?;
///     println!("Found {} recoverable SOL in {}ms", result.recoverable_sol, result.scan_time_ms);
///     Ok(())
/// }
/// ```
pub async fn scan_wallet_ultra_fast(
    wallet_address: &str,
    rpc_endpoint: Option<&str>,
) -> core::Result<WalletInfo> {
    use core::ScannerFactory;
    use core::unified_scanner::UnifiedScannerConfig;
    use core::unified_scanner::PerformanceMode;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use std::sync::Arc;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 200, // Higher rate limit for ultra-fast
        timeout_ms: 5000,     // Shorter timeout for ultra-fast
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 16));
    
    // Ultra-fast configuration using new unified architecture
    let config = UnifiedScannerConfig {
        performance_mode: PerformanceMode::UltraFast,
        max_concurrent_scans: 500,
        scan_timeout: std::time::Duration::from_secs(2),
        batch_size: 100,
        enable_optimizations: true,
        enable_caching: true,
        enable_parallel_processing: true,
    };
    
    let scanner = ScannerFactory::create_with_config(connection_pool, config)?;
    let scan_result = scanner.scan_wallet(wallet_address).await?;
    
    scan_result.result.ok_or_else(|| 
        SolanaRecoverError::InternalError("Scan result is empty".to_string())
    )
}

/// Convenience function for quick wallet scanning using the unified scanner
/// 
/// This is the simplest way to scan a wallet for empty accounts using the new
/// unified architecture with strategy pattern.
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// 
/// # Returns
/// 
/// Returns a `WalletInfo` containing scan results.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::scan_wallet;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = scan_wallet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await?;
///     println!("Found {} recoverable SOL", result.recoverable_sol);
///     Ok(())
/// }
/// ```
pub async fn scan_wallet(
    wallet_address: &str,
    rpc_endpoint: Option<&str>,
) -> core::Result<WalletInfo> {
    use core::ScannerFactory;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use std::sync::Arc;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 100,
        timeout_ms: 30000,
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 8));
    
    // Use balanced mode for general scanning
    let scanner = ScannerFactory::create_balanced(connection_pool)?;
    let scan_result = scanner.scan_wallet(wallet_address).await?;
    
    scan_result.result.ok_or_else(|| 
        SolanaRecoverError::InternalError("Scan result is empty".to_string())
    )
}

/// Convenience function for SOL recovery using the core recovery manager
/// 
/// This is the simplest way to recover SOL from empty accounts.
/// 
/// # Arguments
/// 
/// * `request` - The recovery request containing wallet and destination info
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// * `wallet_manager` - Optional shared wallet manager instance
/// 
/// # Returns
/// 
/// Returns a `RecoveryResult` containing recovery operation results.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::{recover_sol, RecoveryRequest, wallet::WalletManager};
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let wallet_manager = std::sync::Arc::new(WalletManager::new());
///     let request = RecoveryRequest {
///         // ... populate request fields
///         id: uuid::Uuid::new_v4(),
///         wallet_address: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
///         destination_address: "destination_address_here".to_string(),
///         empty_accounts: vec![],
///         max_fee_lamports: Some(10_000_000),
///         priority_fee_lamports: None,
///         wallet_connection_id: None,
///         user_id: None,
///         created_at: chrono::Utc::now(),
///     };
///     
///     let result = recover_sol(&request, None, Some(wallet_manager)).await?;
///     println!("Recovered {} SOL", result.net_sol);
///     Ok(())
/// }
/// ```
pub async fn recover_sol(
    request: &RecoveryRequest,
    rpc_endpoint: Option<&str>,
    wallet_manager: Option<std::sync::Arc<wallet::WalletManager>>,
) -> core::Result<RecoveryResult> {
    use rpc::ConnectionPool;
    use core::{RpcEndpoint, RecoveryManager, RecoveryConfig};
    use wallet::WalletManager;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 100,
        timeout_ms: 30000,
        healthy: true,
    };
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 8));
    
    let config = RecoveryConfig::default();
    let fee_structure = core::FeeStructure::default(); // Can be customized
    let wallet_manager = wallet_manager.unwrap_or_else(|| Arc::new(WalletManager::new()));
    let recovery_manager = RecoveryManager::new(connection_pool, wallet_manager, config, fee_structure);
    
    recovery_manager.recover_sol(request).await
}

/// Convenience function for ultra-fast NFT scanning
/// 
/// This function provides the fastest possible NFT scanning using:
/// - Ultra-fast performance mode
/// - Maximum parallelization
/// - Optimized caching
/// - Minimal validation
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan for NFTs
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// 
/// # Returns
/// 
/// Returns an `NftScanResult` containing comprehensive NFT analysis.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::scan_wallet_nfts_ultra_fast;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = scan_wallet_nfts_ultra_fast("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await?;
///     println!("Found {} NFTs with total value {} SOL", 
///         result.nfts.len(), result.total_estimated_value_lamports as f64 / 1_000_000_000.0);
///     Ok(())
/// }
/// ```
#[cfg(feature = "nft")]
pub async fn scan_wallet_nfts_ultra_fast(
    wallet_address: &str,
    rpc_endpoint: Option<&str>,
) -> core::Result<NftScanResult> {
    use nft::scanner::create_ultra_fast_nft_scanner;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use std::sync::Arc;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 200, // Higher rate limit for ultra-fast
        timeout_ms: 5000,     // Shorter timeout for ultra-fast
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 16));
    let scanner = create_ultra_fast_nft_scanner(connection_pool)?;
    
    let scan_result = scanner.scan_wallet_nfts(wallet_address).await?;
    
    Ok(scan_result)
}

/// Convenience function for balanced NFT scanning
/// 
/// This function provides balanced NFT scanning with comprehensive analysis
/// including metadata, valuation, security validation, and portfolio analysis.
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan for NFTs
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// 
/// # Returns
/// 
/// Returns an `NftScanResult` containing comprehensive NFT analysis.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::scan_wallet_nfts;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = scan_wallet_nfts("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await?;
///     println!("Found {} NFTs with {} security issues", 
///         result.nfts.len(), result.security_issues.len());
///     Ok(())
/// }
/// ```
#[cfg(feature = "nft")]
pub async fn scan_wallet_nfts(
    wallet_address: &str,
    rpc_endpoint: Option<&str>,
) -> core::Result<NftScanResult> {
    use nft::scanner::create_nft_scanner;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use std::sync::Arc;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 100,
        timeout_ms: 30000,
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 8));
    let scanner = create_nft_scanner(connection_pool)?;
    
    let scan_result = scanner.scan_wallet_nfts(wallet_address).await?;
    
    Ok(scan_result)
}

/// Convenience function for thorough NFT scanning
/// 
/// This function provides thorough NFT scanning with maximum validation
/// and comprehensive security analysis.
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan for NFTs
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// 
/// # Returns
/// 
/// Returns an `NftScanResult` containing thorough NFT analysis.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::scan_wallet_nfts_thorough;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = scan_wallet_nfts_thorough("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await?;
///     println!("Thorough scan completed with {} insights", 
///         result.portfolio.as_ref().map(|p| p.quality_metrics.diversity_score).unwrap_or(0.0));
///     Ok(())
/// }
/// ```
#[cfg(feature = "nft")]
pub async fn scan_wallet_nfts_thorough(
    wallet_address: &str,
    rpc_endpoint: Option<&str>,
) -> core::Result<NftScanResult> {
    use nft::scanner::create_thorough_nft_scanner;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use std::sync::Arc;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 50, // Lower rate limit for thorough scanning
        timeout_ms: 60000,   // Longer timeout for thorough scanning
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 4));
    let scanner = create_thorough_nft_scanner(connection_pool)?;
    
    let scan_result = scanner.scan_wallet_nfts(wallet_address).await?;
    
    Ok(scan_result)
}

/// Convenience function for batch NFT scanning
/// 
/// This function scans multiple wallets in parallel with optimized performance.
/// 
/// # Arguments
/// 
/// * `wallet_addresses` - Vector of Solana wallet addresses to scan
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// 
/// # Returns
/// 
/// Returns a vector of `NftScanResult` for each wallet.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::scan_wallets_nfts_batch;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let wallets = vec![
///         "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
///         "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
///     ];
///     let results = scan_wallets_nfts_batch(&wallets, None).await?;
///     for result in results {
///         println!("Wallet {}: {} NFTs", result.wallet_address, result.nfts.len());
///     }
///     Ok(())
/// }
/// ```
#[cfg(feature = "nft")]
pub async fn scan_wallets_nfts_batch(
    wallet_addresses: &[String],
    rpc_endpoint: Option<&str>,
) -> core::Result<Vec<NftScanResult>> {
    use nft::scanner::create_nft_scanner;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use std::sync::Arc;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 100,
        timeout_ms: 30000,
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 8));
    let scanner = create_nft_scanner(connection_pool)?;
    
    let scan_results = scanner.scan_wallets_batch(wallet_addresses).await?;
    
    Ok(scan_results)
}

/// Convenience function for NFT metadata fetching only
/// 
/// This function fetches NFT metadata without valuation or security analysis.
/// 
/// # Arguments
/// 
/// * `mint_address` - The NFT mint address
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet)
/// 
/// # Returns
/// 
/// Returns an `NftInfo` containing the NFT metadata.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use solana_recover::fetch_nft_metadata;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let nft_info = fetch_nft_metadata("mint_address_here", None).await?;
///     println!("NFT Name: {:?}", nft_info.name);
///     println!("NFT Symbol: {:?}", nft_info.symbol);
///     Ok(())
/// }
/// ```
#[cfg(feature = "nft")]
pub async fn fetch_nft_metadata(
    mint_address: &str,
    rpc_endpoint: Option<&str>,
) -> core::Result<NftInfo> {
    use nft::scanner::create_nft_scanner;
    use nft::cache::CacheManager;
    use nft::metadata::MetadataFetcher;
    use nft::metadata::MetadataConfig;
    use core::types::RpcEndpoint;
    use rpc::ConnectionPool;
    use std::sync::Arc;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 100,
        timeout_ms: 30000,
        healthy: true,
    };
    
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 8));
    let cache_manager = Arc::new(CacheManager::new(Default::default()));
    let metadata_config = MetadataConfig::default();
    
    let metadata_fetcher = Arc::new(MetadataFetcher::new(
        connection_pool,
        metadata_config,
        cache_manager,
    )?);
    
    let nft_info = metadata_fetcher.fetch_nft_metadata(mint_address).await?;
    
    Ok(nft_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_default_endpoints() {
        assert_eq!(DEFAULT_MAINNET_ENDPOINT, "https://api.mainnet-beta.solana.com");
        assert_eq!(DEFAULT_DEVNET_ENDPOINT, "https://api.devnet.solana.com");
    }
}
