pub mod core;
pub mod rpc;
pub mod wallet;
pub mod api;
pub mod storage;
pub mod config;
pub mod utils;

// Re-export core types and functions
pub use core::{
    types::{WalletInfo, EmptyAccount, ScanResult, ScanStatus, BatchScanRequest, BatchScanResult, RpcEndpoint, FeeStructure, User, ScanMetrics, RecoveryRequest, RecoveryResult, RecoveryTransaction, RecoveryStatus, TransactionStatus, RecoveryConfig},
    errors::{SolanaRecoverError, Result},
    scanner::WalletScanner,
    processor::BatchProcessor,
    fee_calculator::{FeeCalculator, FeeCalculation, BatchFeeCalculation},
    recovery::RecoveryManager,
};

// Re-export RPC functionality
pub use rpc::{ConnectionPool, RpcClientWrapper};

// Re-export wallet functionality
pub use wallet::{WalletProvider, WalletConnection, WalletCredentialData, ConnectionData};

// Re-export API functionality
pub use api::server::{ApiState, ScanRequest, ApiResponse};

// Re-export configuration functionality
pub use config::{Config, ServerConfig, RpcConfig, ScannerConfig, CacheConfig as ConfigCacheConfig, TurnkeyConfig, DatabaseConfig as ConfigDatabaseConfig};

// Re-export storage functionality
pub use storage::{CacheManager, PersistenceManager, SqlitePersistenceManager, CacheConfig, DatabaseConfig};

// Re-export utils functionality
pub use utils::{MetricsCollector, MetricsConfig, Logger, LoggingConfig, StructuredLogger};
