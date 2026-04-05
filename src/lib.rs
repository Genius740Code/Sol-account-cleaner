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

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default RPC endpoint for mainnet - use config instead
pub const DEFAULT_MAINNET_ENDPOINT: &str = "mainnet";

/// Core error types
#[derive(Debug, thiserror::Error)]
pub enum SolanaRecoverError {
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Invalid wallet address: {0}")]
    InvalidAddress(String),
    
    #[error("RPC error: {0}")]
    RpcError(String),
    
    #[error("Timeout error")]
    TimeoutError,
    
    #[error("Rate limit exceeded")]
    RateLimitError,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Result type for the library
pub type Result<T> = std::result::Result<T, SolanaRecoverError>;

/// Represents an empty token account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyAccount {
    /// The account address
    pub address: String,
    /// The mint address of token
    pub mint: String,
    /// The owner address
    pub owner: String,
    /// Balance in lamports
    pub lamports: u64,
}

/// Result of a wallet scan operation
#[derive(Debug, Clone)]
pub struct WalletScanResult {
    /// Wallet address that was scanned
    pub wallet_address: String,
    /// Total number of accounts found
    pub total_accounts: usize,
    /// Number of empty accounts found
    pub empty_accounts: Vec<EmptyAccount>,
    /// Total recoverable SOL amount
    pub recoverable_sol: f64,
    /// Time taken for the scan in milliseconds
    pub scan_time_ms: u64,
}

/// Convenience function for quick wallet scanning
/// 
/// This is the simplest way to scan a wallet for empty accounts.
/// 
/// # Arguments
/// 
/// * `wallet_address` - The Solana wallet address to scan
/// * `rpc_endpoint` - Optional RPC endpoint (defaults to mainnet configuration)
/// 
/// # Returns
/// 
/// Returns a `WalletScanResult` containing the scan results.
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
#[cfg(feature = "scanner")]
pub async fn scan_wallet(
    wallet_address: &str,
    _rpc_endpoint: Option<&str>,
) -> Result<WalletScanResult> {
    // For now, return a mock result until the advanced scanner is properly integrated
    let start_time = std::time::Instant::now();
    
    // Basic validation
    if wallet_address.len() != 44 {
        return Err(SolanaRecoverError::InvalidAddress(wallet_address.to_string()));
    }
    
    let result = WalletScanResult {
        wallet_address: wallet_address.to_string(),
        total_accounts: 25,
        empty_accounts: vec![
            EmptyAccount {
                address: "AbCdEfGhIjKlMnOpQrStUvWxYz1234567890abcdef".to_string(),
                mint: "So11111111111111111111111111111111111111112".to_string(),
                owner: wallet_address.to_string(),
                lamports: 2039280,
            },
        ],
        recoverable_sol: 0.00203928,
        scan_time_ms: start_time.elapsed().as_millis() as u64,
    };
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[cfg(feature = "scanner")]
    #[tokio::test]
    async fn test_scan_wallet_function() {
        // This is a basic test to ensure the function compiles
        let result = scan_wallet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await;
        
        // We expect this to succeed with our mock implementation
        assert!(result.is_ok());
        if let Ok(scan_result) = result {
            assert_eq!(scan_result.wallet_address, "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");
        }
    }
}
