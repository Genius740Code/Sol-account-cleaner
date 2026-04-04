pub mod core;
pub mod rpc;
pub mod wallet;
pub mod api;
pub mod storage;
pub mod config;

// Re-export core types and functions
pub use core::{
    types::{WalletInfo, EmptyAccount, ScanResult, ScanStatus, BatchScanRequest, BatchScanResult, RpcEndpoint, FeeStructure, User, ScanMetrics},
    errors::{SolanaRecoverError, Result},
    scanner::WalletScanner,
    processor::BatchProcessor,
};

// Re-export RPC functionality
pub use rpc::{ConnectionPool, RpcClientWrapper};

// Re-export wallet functionality
pub use wallet::{WalletProvider, WalletConnection, WalletCredentialData, ConnectionData};

// Re-export API functionality
pub use api::server::{ApiState, ScanRequest, ApiResponse};

// Re-export configuration functionality
pub use config::{Config, ServerConfig, RpcConfig, ScannerConfig, CacheConfig as ConfigCacheConfig, TurnkeyConfig, LoggingConfig, DatabaseConfig as ConfigDatabaseConfig};

// Re-export storage functionality
pub use storage::{CacheManager, PersistenceManager, SqlitePersistenceManager, CacheConfig, DatabaseConfig};
