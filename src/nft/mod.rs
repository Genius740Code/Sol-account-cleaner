//! # NFT Support Module
//!
//! A comprehensive, high-performance NFT analysis and management system.
//! 
//! This module provides:
//! - Ultra-fast NFT scanning and metadata fetching
//! - Portfolio analysis and valuation
//! - Security validation and risk assessment
//! - Batch processing with parallel optimization
//! - Customizable strategies and extensibility
//! - Integration with existing Solana recovery infrastructure
//!
//! ## Features
//!
//! - **Performance**: Sub-second NFT portfolio scanning
//! - **Scalability**: Linear scaling with available resources
//! - **Security**: Comprehensive validation and risk assessment
//! - **Customization**: Pluggable strategies and extensible architecture
//! - **Integration**: Seamless integration with existing wallet scanning
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use solana_recover::nft::{NftScanner, NftScanConfig};
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = NftScanConfig::default();
//!     let scanner = NftScanner::new(config)?;
//!     
//!     let result = scanner.scan_wallet_nfts("wallet_address_here").await?;
//!     println!("Found {} NFTs with total value {}", result.nfts.len(), result.total_value);
//!     
//!     Ok(())
//! }
//! ```

#[cfg(feature = "nft")]
pub mod types;
#[cfg(feature = "nft")]
pub mod scanner;
#[cfg(feature = "nft")]
pub mod metadata;
#[cfg(feature = "nft")]
pub mod valuation;
#[cfg(feature = "nft")]
pub mod security;
#[cfg(feature = "nft")]
pub mod cache;
#[cfg(feature = "nft")]
pub mod portfolio;
#[cfg(feature = "nft")]
pub mod batch;
#[cfg(feature = "nft")]
pub mod strategies;
#[cfg(feature = "nft")]
pub mod errors;

// Re-export commonly used types when nft feature is enabled
#[cfg(feature = "nft")]
pub use types::*;
#[cfg(feature = "nft")]
pub use scanner::NftScanner;
#[cfg(feature = "nft")]
pub use metadata::MetadataFetcher;
#[cfg(feature = "nft")]
pub use valuation::ValuationEngine;
#[cfg(feature = "nft")]
pub use security::SecurityValidator;
#[cfg(feature = "nft")]
pub use portfolio::PortfolioAnalyzer;
#[cfg(feature = "nft")]
pub use batch::BatchProcessor;
#[cfg(feature = "nft")]
pub use errors::NftError;

// Stub implementations when nft feature is disabled
#[cfg(not(feature = "nft"))]
pub use errors::NftError;

#[cfg(not(feature = "nft"))]
mod stubs {
    use super::errors::NftError;
    use crate::core::Result;
    
    pub struct NftScanner;
    pub struct MetadataFetcher;
    pub struct ValuationEngine;
    pub struct SecurityValidator;
    pub struct PortfolioAnalyzer;
    pub struct BatchProcessor;
    
    impl NftScanner {
        pub fn new(_config: ()) -> Result<Self> {
            Err(NftError::FeatureDisabled("nft".to_string()).into())
        }
    }
    
    impl MetadataFetcher {
        pub fn new(_config: ()) -> Result<Self> {
            Err(NftError::FeatureDisabled("nft".to_string()).into())
        }
    }
    
    impl ValuationEngine {
        pub fn new(_config: ()) -> Result<Self> {
            Err(NftError::FeatureDisabled("nft".to_string()).into())
        }
    }
    
    impl SecurityValidator {
        pub fn new(_config: ()) -> Result<Self> {
            Err(NftError::FeatureDisabled("nft".to_string()).into())
        }
    }
    
    impl PortfolioAnalyzer {
        pub fn new(_config: ()) -> Result<Self> {
            Err(NftError::FeatureDisabled("nft".to_string()).into())
        }
    }
    
    impl BatchProcessor {
        pub fn new(_config: ()) -> Result<Self> {
            Err(NftError::FeatureDisabled("nft".to_string()).into())
        }
    }
}

#[cfg(not(feature = "nft"))]
pub use stubs::*;
