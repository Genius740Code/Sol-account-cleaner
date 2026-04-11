use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub rpc: RpcConfig,
    pub scanner: ScannerConfig,
    pub cache: CacheConfig,
    pub memory: MemoryConfig,
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
            enable_hierarchical_cache: config.enable_hierarchical_cache,
            l1_cache_size: config.l1_max_size,
            l2_cache_size: config.l2_max_size,
            compression_threshold: config.compression_threshold,
            enable_metrics: config.enable_metrics,
        }
    }
}

impl From<crate::config::CacheConfig> for crate::storage::HierarchicalCacheConfig {
    fn from(config: crate::config::CacheConfig) -> Self {
        crate::storage::HierarchicalCacheConfig {
            l1_ttl_seconds: config.l1_ttl_seconds,
            l1_max_size: config.l1_max_size,
            l2_ttl_seconds: config.l2_ttl_seconds,
            l2_max_size: config.l2_max_size,
            l3_ttl_seconds: config.l3_ttl_seconds,
            enable_compression: config.enable_compression,
            compression_threshold: config.compression_threshold,
            enable_cache_warming: config.enable_cache_warming,
            enable_metrics: config.enable_metrics,
            redis_url: std::env::var("REDIS_URL").ok(),
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

impl From<crate::config::MemoryConfig> for crate::utils::enhanced_memory_manager::MemoryManagerConfig {
    fn from(config: crate::config::MemoryConfig) -> Self {
        crate::utils::enhanced_memory_manager::MemoryManagerConfig {
            max_pool_sizes: crate::utils::enhanced_memory_manager::PoolSizes {
                wallet_info_pool: config.pool_sizes.wallet_info_pool,
                empty_account_pool: config.pool_sizes.empty_account_pool,
                scan_result_pool: config.pool_sizes.scan_result_pool,
                batch_scan_result_pool: config.pool_sizes.batch_scan_result_pool,
                recovery_transaction_pool: config.pool_sizes.recovery_transaction_pool,
                string_pool: config.pool_sizes.string_pool,
                vec_string_pool: config.pool_sizes.vec_string_pool,
                vec_u8_pool: config.pool_sizes.vec_u8_pool,
            },
            gc_config: crate::utils::enhanced_memory_manager::GcConfig {
                interval_seconds: config.gc_config.interval_seconds,
                memory_threshold_percent: config.gc_config.memory_threshold_percent,
                force_gc_interval_seconds: config.gc_config.force_gc_interval_seconds,
                enable_adaptive_gc: config.gc_config.enable_adaptive_gc,
            },
            monitoring_config: crate::utils::enhanced_memory_manager::MonitoringConfig {
                collection_interval_seconds: config.monitoring_config.collection_interval_seconds,
                enable_leak_detection: config.monitoring_config.enable_leak_detection,
                leak_detection_threshold_seconds: config.monitoring_config.leak_detection_threshold_seconds,
                enable_memory_profiling: config.monitoring_config.enable_memory_profiling,
            },
            enable_object_pooling: config.enable_object_pooling,
            enable_memory_monitoring: config.enable_memory_monitoring,
            enable_auto_optimization: config.enable_auto_optimization,
        }
    }
}

impl From<super::MemoryConfig> for crate::utils::advanced_buffer_pools::BufferPoolConfig {
    fn from(config: crate::config::MemoryConfig) -> Self {
        crate::utils::advanced_buffer_pools::BufferPoolConfig {
            pool_sizes: crate::utils::advanced_buffer_pools::BufferPoolSizes {
                tiny_pool_size: config.buffer_config.tiny_pool_size,
                small_pool_size: config.buffer_config.small_pool_size,
                medium_pool_size: config.buffer_config.medium_pool_size,
                large_pool_size: config.buffer_config.large_pool_size,
                xlarge_pool_size: config.buffer_config.xlarge_pool_size,
                xxlarge_pool_size: config.buffer_config.xxlarge_pool_size,
                jumbo_pool_size: config.buffer_config.jumbo_pool_size,
                rpc_request_pool_size: config.pool_sizes.rpc_request_pool,
                rpc_response_pool_size: config.pool_sizes.rpc_response_pool,
                account_data_pool_size: config.pool_sizes.account_data_pool,
                transaction_pool_size: config.pool_sizes.recovery_transaction_pool,
            },
            enable_compression: config.buffer_config.enable_compression,
            enable_zero_copy: config.buffer_config.enable_zero_copy,
            max_buffer_age_seconds: config.buffer_config.max_buffer_age_seconds,
            cleanup_interval_seconds: config.buffer_config.cleanup_interval_seconds,
            enable_stats_collection: config.enable_memory_monitoring,
        }
    }
}

impl From<crate::config::MemoryConfig> for crate::utils::gc_scheduler::GcSchedulerConfig {
    fn from(config: crate::config::MemoryConfig) -> Self {
        crate::utils::gc_scheduler::GcSchedulerConfig {
            base_interval_seconds: config.gc_config.interval_seconds,
            max_interval_seconds: config.gc_config.force_gc_interval_seconds,
            min_interval_seconds: 10,
            memory_pressure_threshold: config.gc_config.memory_threshold_percent,
            enable_adaptive_scheduling: config.gc_config.enable_adaptive_gc,
            max_concurrent_gc: config.gc_config.max_concurrent_gc,
            gc_timeout_seconds: config.gc_config.gc_timeout_seconds,
            enable_incremental_gc: config.gc_config.enable_incremental_gc,
            incremental_batch_size: 100,
            priority_config: crate::utils::gc_scheduler::GcPriorityConfig::default(),
            performance_targets: crate::utils::gc_scheduler::GcPerformanceTargets::default(),
        }
    }
}

impl From<crate::config::MemoryConfig> for crate::utils::memory_monitor::MemoryMonitorConfig {
    fn from(config: crate::config::MemoryConfig) -> Self {
        crate::utils::memory_monitor::MemoryMonitorConfig {
            monitoring_interval_seconds: config.monitoring_config.collection_interval_seconds,
            history_retention_seconds: config.monitoring_config.history_retention_seconds,
            enable_profiling: config.monitoring_config.enable_memory_profiling,
            enable_leak_detection: config.monitoring_config.enable_leak_detection,
            enable_performance_monitoring: config.enable_memory_monitoring,
            alert_thresholds: crate::utils::memory_monitor::AlertThresholds::default(),
            performance_targets: crate::utils::memory_monitor::PerformanceTargets::default(),
            enable_real_time_events: config.monitoring_config.enable_real_time_events,
            max_history_size: 1000,
        }
    }
}

impl From<crate::config::MemoryConfig> for crate::utils::memory_integration::MemoryIntegrationConfig {
    fn from(config: crate::config::MemoryConfig) -> Self {
        crate::utils::memory_integration::MemoryIntegrationConfig {
            enable_scanner_pooling: config.enable_object_pooling,
            enable_rpc_pooling: config.enable_object_pooling,
            enable_buffer_pooling: config.enable_object_pooling,
            enable_auto_gc: config.enable_auto_optimization,
            enable_monitoring: config.enable_memory_monitoring,
            scanner_config: crate::utils::memory_integration::ScannerMemoryConfig {
                wallet_info_pool_size: config.pool_sizes.wallet_info_pool,
                empty_account_pool_size: config.pool_sizes.empty_account_pool,
                scan_result_pool_size: config.pool_sizes.scan_result_pool,
                batch_scan_result_pool_size: config.pool_sizes.batch_scan_result_pool,
                enable_scan_tracking: config.enable_memory_monitoring,
            },
            rpc_config: crate::utils::memory_integration::RpcMemoryConfig {
                request_buffer_pool_size: config.pool_sizes.rpc_request_pool,
                response_buffer_pool_size: config.pool_sizes.rpc_response_pool,
                account_data_buffer_pool_size: config.pool_sizes.account_data_pool,
                enable_rpc_tracking: config.enable_memory_monitoring,
                max_request_size: 64 * 1024,  // 64KB
                max_response_size: 1024 * 1024, // 1MB
            },
            buffer_config: crate::utils::memory_integration::BufferIntegrationConfig {
                enable_size_tiered_pools: true,
                enable_rpc_specialized_buffers: true,
                cleanup_interval_seconds: config.buffer_config.cleanup_interval_seconds,
                max_buffer_age_seconds: config.buffer_config.max_buffer_age_seconds,
            },
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
            enable_intelligent_processing: true,
            num_workers: None,
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
    pub enable_hierarchical_cache: bool,
    pub l1_ttl_seconds: u64,
    pub l1_max_size: usize,
    pub l2_ttl_seconds: u64,
    pub l2_max_size: usize,
    pub l3_ttl_seconds: u64,
    pub enable_compression: bool,
    pub compression_threshold: usize,
    pub enable_cache_warming: bool,
    pub enable_metrics: bool,
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
pub struct MemoryConfig {
    pub enable_object_pooling: bool,
    pub enable_memory_monitoring: bool,
    pub enable_auto_optimization: bool,
    pub pool_sizes: MemoryPoolSizes,
    pub gc_config: MemoryGcConfig,
    pub monitoring_config: MemoryMonitoringConfig,
    pub buffer_config: MemoryBufferConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPoolSizes {
    pub wallet_info_pool: usize,
    pub empty_account_pool: usize,
    pub scan_result_pool: usize,
    pub batch_scan_result_pool: usize,
    pub recovery_transaction_pool: usize,
    pub string_pool: usize,
    pub vec_string_pool: usize,
    pub vec_u8_pool: usize,
    pub rpc_request_pool: usize,
    pub rpc_response_pool: usize,
    pub account_data_pool: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryGcConfig {
    pub interval_seconds: u64,
    pub memory_threshold_percent: f64,
    pub force_gc_interval_seconds: u64,
    pub enable_adaptive_gc: bool,
    pub max_concurrent_gc: usize,
    pub gc_timeout_seconds: u64,
    pub enable_incremental_gc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMonitoringConfig {
    pub collection_interval_seconds: u64,
    pub enable_leak_detection: bool,
    pub leak_detection_threshold_seconds: u64,
    pub enable_memory_profiling: bool,
    pub enable_real_time_events: bool,
    pub history_retention_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBufferConfig {
    pub tiny_pool_size: usize,
    pub small_pool_size: usize,
    pub medium_pool_size: usize,
    pub large_pool_size: usize,
    pub xlarge_pool_size: usize,
    pub xxlarge_pool_size: usize,
    pub jumbo_pool_size: usize,
    pub enable_compression: bool,
    pub enable_zero_copy: bool,
    pub max_buffer_age_seconds: u64,
    pub cleanup_interval_seconds: u64,
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
            memory: MemoryConfig::default(),
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
            endpoints: vec![
                "https://api.mainnet-beta.solana.com".to_string(),
                "https://solana-api.projectserum.com".to_string(),
                "https://rpc.ankr.com/solana".to_string(),
            ],
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
            enable_hierarchical_cache: true,
            l1_ttl_seconds: 60,
            l1_max_size: 100000,
            l2_ttl_seconds: 900,
            l2_max_size: 1000000,
            l3_ttl_seconds: 3600,
            enable_compression: true,
            compression_threshold: 1024,
            enable_cache_warming: true,
            enable_metrics: true,
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

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enable_object_pooling: true,
            enable_memory_monitoring: true,
            enable_auto_optimization: true,
            pool_sizes: MemoryPoolSizes::default(),
            gc_config: MemoryGcConfig::default(),
            monitoring_config: MemoryMonitoringConfig::default(),
            buffer_config: MemoryBufferConfig::default(),
        }
    }
}

impl Default for MemoryPoolSizes {
    fn default() -> Self {
        Self {
            wallet_info_pool: 10000,
            empty_account_pool: 50000,
            scan_result_pool: 10000,
            batch_scan_result_pool: 1000,
            recovery_transaction_pool: 5000,
            string_pool: 100000,
            vec_string_pool: 20000,
            vec_u8_pool: 50000,
            rpc_request_pool: 1000,
            rpc_response_pool: 1000,
            account_data_pool: 2000,
        }
    }
}

impl Default for MemoryGcConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 60,
            memory_threshold_percent: 80.0,
            force_gc_interval_seconds: 300,
            enable_adaptive_gc: true,
            max_concurrent_gc: 1,
            gc_timeout_seconds: 30,
            enable_incremental_gc: true,
        }
    }
}

impl Default for MemoryMonitoringConfig {
    fn default() -> Self {
        Self {
            collection_interval_seconds: 30,
            enable_leak_detection: true,
            leak_detection_threshold_seconds: 300,
            enable_memory_profiling: true,
            enable_real_time_events: true,
            history_retention_seconds: 3600,
        }
    }
}

impl Default for MemoryBufferConfig {
    fn default() -> Self {
        Self {
            tiny_pool_size: 10000,      // 64B-256B buffers
            small_pool_size: 5000,      // 256B-1KB buffers
            medium_pool_size: 2000,     // 1KB-4KB buffers
            large_pool_size: 1000,      // 4KB-16KB buffers
            xlarge_pool_size: 500,      // 16KB-64KB buffers
            xxlarge_pool_size: 100,     // 64KB-256KB buffers
            jumbo_pool_size: 50,         // 256KB-1MB buffers
            enable_compression: false,
            enable_zero_copy: true,
            max_buffer_age_seconds: 300,
            cleanup_interval_seconds: 60,
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
