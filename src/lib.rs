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

// Re-export commonly used types
pub use core::*;

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

/// Convenience function for quick wallet scanning using the core scanner
/// 
/// This is the simplest way to scan a wallet for empty accounts.
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
    use rpc::ConnectionPool;
    use core::RpcEndpoint;
    
    let endpoint = rpc_endpoint.unwrap_or(DEFAULT_MAINNET_ENDPOINT);
    let rpc_endpoint = RpcEndpoint {
        url: endpoint.to_string(),
        priority: 0,
        rate_limit_rps: 100,
        timeout_ms: 30000,
        healthy: true,
    };
    let connection_pool = Arc::new(ConnectionPool::new(vec![rpc_endpoint], 8));
    let scanner = Arc::new(core::scanner::WalletScanner::new(connection_pool));
    
    scanner.scan_wallet(wallet_address).await.map(|scan_result| scan_result.result.unwrap())
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
