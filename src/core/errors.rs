use thiserror::Error;

#[derive(Error, Debug)]
pub enum SolanaRecoverError {
    #[error("RPC client error: {0}")]
    RpcClientError(String),
    
    #[error("RPC error: {0}")]
    RpcError(String),
    
    #[error("Invalid wallet address: {0}")]
    InvalidWalletAddress(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    
    #[error("Connection pool exhausted")]
    ConnectionPoolExhausted,
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    
    #[error("Invalid fee structure: {0}")]
    InvalidFeeStructure(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Wallet not found: {0}")]
    WalletNotFound(String),
    
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },
    
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("No recoverable funds")]
    NoRecoverableFunds(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Rusqlite error: {0}")]
    RusqliteError(String),
    
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    #[error("Security error: {0}")]
    SecurityError(String),
    
    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),
    
    #[error("NFT error: {0}")]
    NftError(String),
}

impl Clone for SolanaRecoverError {
    fn clone(&self) -> Self {
        match self {
            SolanaRecoverError::RpcClientError(msg) => SolanaRecoverError::RpcClientError(msg.clone()),
            SolanaRecoverError::RpcError(msg) => SolanaRecoverError::RpcError(msg.clone()),
            SolanaRecoverError::InvalidWalletAddress(msg) => SolanaRecoverError::InvalidWalletAddress(msg.clone()),
            SolanaRecoverError::RateLimitExceeded(msg) => SolanaRecoverError::RateLimitExceeded(msg.clone()),
            SolanaRecoverError::ConnectionPoolExhausted => SolanaRecoverError::ConnectionPoolExhausted,
            SolanaRecoverError::ConfigError(msg) => SolanaRecoverError::ConfigError(msg.clone()),
            SolanaRecoverError::ConfigurationError(msg) => SolanaRecoverError::ConfigurationError(msg.clone()),
            SolanaRecoverError::StorageError(msg) => SolanaRecoverError::StorageError(msg.clone()),
            SolanaRecoverError::SerializationError(msg) => SolanaRecoverError::SerializationError(msg.clone()),
            SolanaRecoverError::IoError(msg) => SolanaRecoverError::IoError(msg.clone()),
            SolanaRecoverError::TimeoutError(msg) => SolanaRecoverError::TimeoutError(msg.clone()),
            SolanaRecoverError::NetworkError(msg) => SolanaRecoverError::NetworkError(msg.clone()),
            SolanaRecoverError::AuthenticationError(msg) => SolanaRecoverError::AuthenticationError(msg.clone()),
            SolanaRecoverError::InvalidFeeStructure(msg) => SolanaRecoverError::InvalidFeeStructure(msg.clone()),
            SolanaRecoverError::ValidationError(msg) => SolanaRecoverError::ValidationError(msg.clone()),
            SolanaRecoverError::WalletNotFound(msg) => SolanaRecoverError::WalletNotFound(msg.clone()),
            SolanaRecoverError::InsufficientBalance { required, available } => SolanaRecoverError::InsufficientBalance { required: *required, available: *available },
            SolanaRecoverError::TransactionFailed(msg) => SolanaRecoverError::TransactionFailed(msg.clone()),
            SolanaRecoverError::InternalError(msg) => SolanaRecoverError::InternalError(msg.clone()),
            SolanaRecoverError::InvalidInput(msg) => SolanaRecoverError::InvalidInput(msg.clone()),
            SolanaRecoverError::NoRecoverableFunds(msg) => SolanaRecoverError::NoRecoverableFunds(msg.clone()),
            SolanaRecoverError::DatabaseError(msg) => SolanaRecoverError::DatabaseError(msg.clone()),
            SolanaRecoverError::RusqliteError(msg) => SolanaRecoverError::RusqliteError(msg.clone()),
            SolanaRecoverError::TransactionError(msg) => SolanaRecoverError::TransactionError(msg.clone()),
            SolanaRecoverError::SecurityError(msg) => SolanaRecoverError::SecurityError(msg.clone()),
            SolanaRecoverError::CircuitBreakerOpen(msg) => SolanaRecoverError::CircuitBreakerOpen(msg.clone()),
            SolanaRecoverError::NftError(msg) => SolanaRecoverError::NftError(msg.clone()),
        }
    }
}

impl From<solana_client::client_error::ClientError> for SolanaRecoverError {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        SolanaRecoverError::RpcClientError(err.to_string())
    }
}

impl From<std::io::Error> for SolanaRecoverError {
    fn from(err: std::io::Error) -> Self {
        SolanaRecoverError::IoError(err.to_string())
    }
}

impl From<rusqlite::Error> for SolanaRecoverError {
    fn from(err: rusqlite::Error) -> Self {
        SolanaRecoverError::RusqliteError(err.to_string())
    }
}

#[cfg(feature = "nft")]
impl From<crate::nft::errors::NftError> for SolanaRecoverError {
    fn from(err: crate::nft::errors::NftError) -> Self {
        SolanaRecoverError::NftError(format!("{}", err))
    }
}

pub type Result<T> = std::result::Result<T, SolanaRecoverError>;
