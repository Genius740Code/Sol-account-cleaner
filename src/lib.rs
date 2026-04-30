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

// Re-export NFT types when feature is enabled
#[cfg(feature = "nft")]
pub use nft::scanner::NftScanResult;
#[cfg(feature = "nft")]
pub use nft::types::NftInfo;

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

/// Ultra-fast wallet scanning with all optimizations enabled
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
