use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub rpc: RpcConfig,
    pub scanner: ScannerConfig,
    pub cache: CacheConfig,
    pub turnkey: TurnkeyConfig,
    pub logging: LoggingConfig,
    pub database: DatabaseConfig,
}

impl From<crate::config::CacheConfig> for crate::storage::CacheConfig {
    fn from(config: crate::config::CacheConfig) -> Self {
        crate::storage::CacheConfig {
            ttl_seconds: config.ttl_seconds,
            max_size: config.max_size,
            cleanup_interval_seconds: config.cleanup_interval_seconds,
        }
    }
}

impl From<crate::config::DatabaseConfig> for crate::storage::DatabaseConfig {
    fn from(config: crate::config::DatabaseConfig) -> Self {
        crate::storage::DatabaseConfig {
            database_url: config.database_url,
            max_connections: config.max_connections,
        }
    }
}

impl From<crate::config::ScannerConfig> for crate::core::processor::ProcessorConfig {
    fn from(config: crate::config::ScannerConfig) -> Self {
        crate::core::processor::ProcessorConfig {
            batch_size: config.batch_size,
            max_concurrent_wallets: config.max_concurrent_wallets,
            retry_attempts: config.retry_attempts,
            retry_delay_ms: config.retry_delay_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub endpoints: Vec<String>,
    pub pool_size: usize,
    pub timeout_ms: u64,
    pub rate_limit_rps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub batch_size: usize,
    pub max_concurrent_wallets: usize,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub ttl_seconds: u64,
    pub max_size: usize,
    pub cleanup_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyConfig {
    pub api_url: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            rpc: RpcConfig::default(),
            scanner: ScannerConfig::default(),
            cache: CacheConfig::default(),
            turnkey: TurnkeyConfig::default(),
            logging: LoggingConfig::default(),
            database: DatabaseConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            workers: 4,
        }
    }
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            endpoints: vec!["https://api.mainnet-beta.solana.com".to_string()],
            pool_size: 10,
            timeout_ms: 5000,
            rate_limit_rps: 100,
        }
    }
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_concurrent_wallets: 1000,
            retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 300,
            max_size: 10000,
            cleanup_interval_seconds: 60,
        }
    }
}

impl Default for TurnkeyConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.turnkey.com".to_string(),
            timeout_ms: 10000,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            file_path: None,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: "./solana_recover.db".to_string(),
            max_connections: 10,
        }
    }
}

impl Config {
    pub fn load() -> crate::Result<Self> {
        // Try to load from environment variables first
        if let Ok(config) = Self::from_env() {
            return Ok(config);
        }

        // Try to load from config files
        let config_dirs = [
            "config/production.toml",
            "config/development.toml",
            "config/default.toml",
        ];

        for config_file in &config_dirs {
            if Path::new(config_file).exists() {
                return Self::from_file(config_file);
            }
        }

        // Return default configuration
        Ok(Self::default())
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::SolanaRecoverError::ConfigurationError(
                format!("Failed to read config file: {}", e)
            ))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| crate::SolanaRecoverError::ConfigurationError(
                format!("Failed to parse config file: {}", e)
            ))?;

        Ok(config)
    }

    pub fn from_env() -> crate::Result<Self> {
        let mut config = Config::default();

        // Server configuration
        if let Ok(host) = std::env::var("SERVER_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("SERVER_PORT") {
            config.server.port = port.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid SERVER_PORT value".to_string()
                ))?;
        }
        if let Ok(workers) = std::env::var("SERVER_WORKERS") {
            config.server.workers = workers.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid SERVER_WORKERS value".to_string()
                ))?;
        }

        // RPC configuration
        if let Ok(endpoints) = std::env::var("RPC_ENDPOINTS") {
            config.rpc.endpoints = endpoints.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Ok(pool_size) = std::env::var("RPC_POOL_SIZE") {
            config.rpc.pool_size = pool_size.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid RPC_POOL_SIZE value".to_string()
                ))?;
        }
        if let Ok(timeout) = std::env::var("RPC_TIMEOUT_MS") {
            config.rpc.timeout_ms = timeout.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid RPC_TIMEOUT_MS value".to_string()
                ))?;
        }

        // Scanner configuration
        if let Ok(batch_size) = std::env::var("SCANNER_BATCH_SIZE") {
            config.scanner.batch_size = batch_size.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid SCANNER_BATCH_SIZE value".to_string()
                ))?;
        }
        if let Ok(max_concurrent) = std::env::var("SCANNER_MAX_CONCURRENT_WALLETS") {
            config.scanner.max_concurrent_wallets = max_concurrent.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid SCANNER_MAX_CONCURRENT_WALLETS value".to_string()
                ))?;
        }

        // Cache configuration
        if let Ok(ttl) = std::env::var("CACHE_TTL_SECONDS") {
            config.cache.ttl_seconds = ttl.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid CACHE_TTL_SECONDS value".to_string()
                ))?;
        }
        if let Ok(max_size) = std::env::var("CACHE_MAX_SIZE") {
            config.cache.max_size = max_size.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid CACHE_MAX_SIZE value".to_string()
                ))?;
        }

        // Database configuration
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            config.database.database_url = database_url;
        }
        if let Ok(max_connections) = std::env::var("DATABASE_MAX_CONNECTIONS") {
            config.database.max_connections = max_connections.parse()
                .map_err(|_| crate::SolanaRecoverError::ConfigurationError(
                    "Invalid DATABASE_MAX_CONNECTIONS value".to_string()
                ))?;
        }

        // Logging configuration
        if let Ok(level) = std::env::var("LOG_LEVEL") {
            config.logging.level = level;
        }
        if let Ok(format) = std::env::var("LOG_FORMAT") {
            config.logging.format = format;
        }
        if let Ok(file_path) = std::env::var("LOG_FILE") {
            config.logging.file_path = Some(file_path);
        }

        Ok(config)
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.server.port == 0 {
            return Err(crate::SolanaRecoverError::ConfigurationError(
                "Server port cannot be 0".to_string()
            ));
        }

        if self.rpc.endpoints.is_empty() {
            return Err(crate::SolanaRecoverError::ConfigurationError(
                "At least one RPC endpoint must be configured".to_string()
            ));
        }

        if self.rpc.pool_size == 0 {
            return Err(crate::SolanaRecoverError::ConfigurationError(
                "RPC pool size must be greater than 0".to_string()
            ));
        }

        if self.scanner.batch_size == 0 {
            return Err(crate::SolanaRecoverError::ConfigurationError(
                "Scanner batch size must be greater than 0".to_string()
            ));
        }

        if self.cache.max_size == 0 {
            return Err(crate::SolanaRecoverError::ConfigurationError(
                "Cache max size must be greater than 0".to_string()
            ));
        }

        Ok(())
    }
}
