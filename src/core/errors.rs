use thiserror::Error;

#[derive(Error, Debug)]
pub enum SolanaRecoverError {
    #[error("RPC client error: {0}")]
    RpcClientError(#[from] solana_client::client_error::ClientError),
    
    #[error("Invalid wallet address: {0}")]
    InvalidWalletAddress(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("No recoverable funds: {0}")]
    NoRecoverableFunds(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
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
    IoError(#[from] std::io::Error),
    
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
    
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Database error: {0}")]
    RusqliteError(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, SolanaRecoverError>;
