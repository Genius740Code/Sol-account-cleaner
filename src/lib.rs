pub mod core;
pub mod rpc;
pub mod wallet;
pub mod api;

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
