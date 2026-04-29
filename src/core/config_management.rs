//! Configuration Management System
//! 
//! This module provides comprehensive configuration management with support for
//! environment-specific configs, validation, hot-reload, and external configuration sources.

use crate::core::{Result, SolanaRecoverError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use std::time::{Duration, Instant, SystemTime};
use std::collections::HashMap;

/// Environment types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

impl Default for Environment {
    fn default() -> Self {
        Environment::Development
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Development => write!(f, "development"),
            Environment::Testing => write!(f, "testing"),
            Environment::Staging => write!(f, "staging"),
            Environment::Production => write!(f, "production"),
        }
    }
}

/// RPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Default RPC endpoints
    pub endpoints: Vec<RpcEndpointConfig>,
    
    /// Connection pool settings
    pub pool_size: usize,
    
    /// Request timeout
    pub timeout_ms: u64,
    
    /// Rate limiting
    pub rate_limit_rps: u32,
    
    /// Retry settings
    pub max_retries: usize,
    pub retry_delay_ms: u64,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            endpoints: vec![
                RpcEndpointConfig {
                    url: "https://api.mainnet-beta.solana.com".to_string(),
                    priority: 1,
                    rate_limit_rps: 100,
                    timeout_ms: 30000,
                    enabled: true,
                }
            ],
            pool_size: 8,
            timeout_ms: 30000,
            rate_limit_rps: 100,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// Individual RPC endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEndpointConfig {
    pub url: String,
    pub priority: u8,
    pub rate_limit_rps: u32,
    pub timeout_ms: u64,
    pub enabled: bool,
}

/// Scanner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    /// Performance mode
    pub performance_mode: String,
    
    /// Maximum concurrent scans
    pub max_concurrent_scans: usize,
    
    /// Scan timeout
    pub scan_timeout_seconds: u64,
    
    /// Batch size
    pub batch_size: usize,
    
    /// Optimization settings
    pub enable_optimizations: bool,
    pub enable_caching: bool,
    pub enable_parallel_processing: bool,
    
    /// Ultra-fast specific settings
    pub ultra_fast: UltraFastConfig,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            performance_mode: "balanced".to_string(),
            max_concurrent_scans: 100,
            scan_timeout_seconds: 30,
            batch_size: 50,
            enable_optimizations: true,
            enable_caching: true,
            enable_parallel_processing: true,
            ultra_fast: UltraFastConfig::default(),
        }
    }
}

/// Ultra-fast scanner specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UltraFastConfig {
    pub prefetch_window_size: usize,
    pub batch_size_multiplier: f64,
    pub enable_predictive_prefetch: bool,
    pub enable_connection_multiplexing: bool,
    pub enable_smart_batching: bool,
    pub enable_fast_path: bool,
}

impl Default for UltraFastConfig {
    fn default() -> Self {
        Self {
            prefetch_window_size: 50,
            batch_size_multiplier: 2.0,
            enable_predictive_prefetch: true,
            enable_connection_multiplexing: true,
            enable_smart_batching: true,
            enable_fast_path: true,
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache type (memory, redis, etc.)
    pub cache_type: String,
    
    /// Memory cache settings
    pub memory: MemoryCacheConfig,
    
    /// Redis cache settings (if applicable)
    pub redis: Option<RedisCacheConfig>,
    
    /// Default TTL
    pub default_ttl_seconds: u64,
    
    /// Maximum cache size
    pub max_size_mb: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: "memory".to_string(),
            memory: MemoryCacheConfig::default(),
            redis: None,
            default_ttl_seconds: 300, // 5 minutes
            max_size_mb: 100,
        }
    }
}

/// Memory cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCacheConfig {
    pub max_items: usize,
    pub ttl_seconds: u64,
    pub enable_compression: bool,
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_items: 10000,
            ttl_seconds: 300,
            enable_compression: false,
        }
    }
}

/// Redis cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCacheConfig {
    pub url: String,
    pub pool_size: usize,
    pub connection_timeout_ms: u64,
    pub command_timeout_ms: u64,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Rate limiting
    pub rate_limiting: RateLimitConfig,
    
    /// Audit logging
    pub audit: AuditConfig,
    
    /// Access control
    pub access_control: AccessControlConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            rate_limiting: RateLimitConfig::default(),
            audit: AuditConfig::default(),
            access_control: AccessControlConfig::default(),
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub per_wallet_limits: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: 60,
            burst_size: 10,
            per_wallet_limits: true,
        }
    }
}

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enabled: bool,
    pub log_level: String,
    pub retention_days: u32,
    pub include_sensitive_data: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: "info".to_string(),
            retention_days: 30,
            include_sensitive_data: false,
        }
    }
}

/// Access control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    pub allowed_destinations: Vec<String>,
    pub require_whitelist: bool,
    pub max_transaction_amount_lamports: u64,
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            allowed_destinations: vec![],
            require_whitelist: false,
            max_transaction_amount_lamports: 1_000_000_000_000, // 1000 SOL
        }
    }
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Metrics collection
    pub metrics: MetricsConfig,
    
    /// Memory management
    pub memory: MemoryConfig,
    
    /// Parallel processing
    pub parallel: ParallelConfig,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            memory: MemoryConfig::default(),
            parallel: ParallelConfig::default(),
        }
    }
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub export_interval_seconds: u64,
    pub prometheus_port: u16,
    pub include_detailed_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            export_interval_seconds: 30,
            prometheus_port: 9090,
            include_detailed_metrics: false,
        }
    }
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub max_heap_size_mb: usize,
    pub gc_threshold_percentage: f64,
    pub enable_memory_pooling: bool,
    pub pool_size_mb: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_heap_size_mb: 1024,
            gc_threshold_percentage: 80.0,
            enable_memory_pooling: true,
            pool_size_mb: 100,
        }
    }
}

/// Parallel processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    pub max_workers: usize,
    pub work_stealing: bool,
    pub queue_size: usize,
    pub timeout_seconds: u64,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_workers: num_cpus::get(),
            work_stealing: true,
            queue_size: 1000,
            timeout_seconds: 60,
        }
    }
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Environment
    pub environment: Environment,
    
    /// Application metadata
    pub app_name: String,
    pub version: String,
    
    /// Component configurations
    pub rpc: RpcConfig,
    pub scanner: ScannerConfig,
    pub cache: CacheConfig,
    pub security: SecurityConfig,
    pub performance: PerformanceConfig,
    
    /// Feature flags
    pub features: HashMap<String, bool>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut features = HashMap::new();
        features.insert("scanner".to_string(), true);
        features.insert("api".to_string(), true);
        features.insert("cache".to_string(), true);
        features.insert("metrics".to_string(), true);
        
        Self {
            environment: Environment::Development,
            app_name: "solana-recover".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            rpc: RpcConfig::default(),
            scanner: ScannerConfig::default(),
            cache: CacheConfig::default(),
            security: SecurityConfig::default(),
            performance: PerformanceConfig::default(),
            features,
        }
    }
}

impl AppConfig {
    /// Load configuration from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| SolanaRecoverError::InternalError(
                format!("Failed to read config file: {}", e)
            ))?;
        
        let config: AppConfig = toml::from_str(&content)
            .map_err(|e| SolanaRecoverError::InternalError(
                format!("Failed to parse config file: {}", e)
            ))?;
        
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration from environment variables
    pub fn load_from_env() -> Result<Self> {
        let mut config = AppConfig::default();
        
        // Environment
        if let Ok(env_str) = env::var("ENVIRONMENT") {
            config.environment = match env_str.to_lowercase().as_str() {
                "development" => Environment::Development,
                "testing" => Environment::Testing,
                "staging" => Environment::Staging,
                "production" => Environment::Production,
                _ => Environment::Development,
            };
        }
        
        // RPC settings
        if let Ok(endpoints) = env::var("RPC_ENDPOINTS") {
            config.rpc.endpoints = endpoints
                .split(',')
                .map(|url| RpcEndpointConfig {
                    url: url.trim().to_string(),
                    priority: 1,
                    rate_limit_rps: 100,
                    timeout_ms: 30000,
                    enabled: true,
                })
                .collect();
        }
        
        if let Ok(pool_size) = env::var("RPC_POOL_SIZE") {
            config.rpc.pool_size = pool_size.parse().unwrap_or(8);
        }
        
        // Scanner settings
        if let Ok(performance_mode) = env::var("SCANNER_PERFORMANCE_MODE") {
            config.scanner.performance_mode = performance_mode;
        }
        
        if let Ok(max_concurrent) = env::var("SCANNER_MAX_CONCURRENT") {
            config.scanner.max_concurrent_scans = max_concurrent.parse().unwrap_or(100);
        }
        
        // Cache settings
        if let Ok(cache_type) = env::var("CACHE_TYPE") {
            config.cache.cache_type = cache_type;
        }
        
        // Security settings
        if let Ok(rate_limit_enabled) = env::var("RATE_LIMIT_ENABLED") {
            config.security.rate_limiting.enabled = rate_limit_enabled.parse().unwrap_or(true);
        }
        
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration with fallback chain
    pub fn load() -> Result<Self> {
        // Try environment-specific config file first
        let env = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        let env_config_path = format!("config-{}.toml", env);
        
        if Path::new(&env_config_path).exists() {
            info!("Loading environment-specific config from {}", env_config_path);
            return Self::load_from_file(env_config_path);
        }
        
        // Try default config file
        if Path::new("config.toml").exists() {
            info!("Loading default config from config.toml");
            return Self::load_from_file("config.toml");
        }
        
        // Fall back to environment variables
        info!("Loading config from environment variables");
        Self::load_from_env()
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate RPC endpoints
        if self.rpc.endpoints.is_empty() {
            return Err(SolanaRecoverError::InternalError(
                "At least one RPC endpoint must be configured".to_string()
            ));
        }
        
        for endpoint in &self.rpc.endpoints {
            if endpoint.url.is_empty() {
                return Err(SolanaRecoverError::InternalError(
                    "RPC endpoint URL cannot be empty".to_string()
                ));
            }
        }
        
        // Validate scanner settings
        if self.scanner.max_concurrent_scans == 0 {
            return Err(SolanaRecoverError::InternalError(
                "Max concurrent scans must be greater than 0".to_string()
            ));
        }
        
        if self.scanner.batch_size == 0 {
            return Err(SolanaRecoverError::InternalError(
                "Batch size must be greater than 0".to_string()
            ));
        }
        
        // Validate performance mode
        if !["ultra_fast", "balanced", "resource_efficient", "throughput", "latency"]
            .contains(&self.scanner.performance_mode.as_str()) {
            return Err(SolanaRecoverError::InternalError(
                format!("Invalid performance mode: {}", self.scanner.performance_mode)
            ));
        }
        
        // Validate cache settings
        if self.cache.cache_type.is_empty() {
            return Err(SolanaRecoverError::InternalError(
                "Cache type cannot be empty".to_string()
            ));
        }
        
        info!("Configuration validation passed");
        Ok(())
    }
    
    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| SolanaRecoverError::InternalError(
                format!("Failed to serialize config: {}", e)
            ))?;
        
        fs::write(path, content)
            .map_err(|e| SolanaRecoverError::InternalError(
                format!("Failed to write config file: {}", e)
            ))?;
        
        info!("Configuration saved successfully");
        Ok(())
    }
    
    /// Get configuration for specific environment
    pub fn for_environment(env: Environment) -> Self {
        let mut config = Self::default();
        config.environment = env.clone();
        
        match env {
            Environment::Production => {
                config.rpc.rate_limit_rps = 200;
                config.scanner.max_concurrent_scans = 500;
                config.security.audit.include_sensitive_data = false;
                config.performance.metrics.include_detailed_metrics = false;
            }
            Environment::Staging => {
                config.rpc.rate_limit_rps = 150;
                config.scanner.max_concurrent_scans = 200;
                config.security.audit.include_sensitive_data = true;
            }
            Environment::Testing => {
                config.rpc.rate_limit_rps = 50;
                config.scanner.max_concurrent_scans = 10;
                config.security.rate_limiting.enabled = false;
                config.performance.metrics.enabled = false;
            }
            Environment::Development => {
                config.rpc.rate_limit_rps = 100;
                config.scanner.max_concurrent_scans = 50;
                config.security.audit.include_sensitive_data = true;
                config.performance.metrics.include_detailed_metrics = true;
            }
        }
        
        config
    }
}

/// Configuration manager with hot-reload support
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    config_path: Option<PathBuf>,
    last_modified: Arc<RwLock<Option<Instant>>>,
    reload_interval: Duration,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            config_path: None,
            last_modified: Arc::new(RwLock::new(None)),
            reload_interval: Duration::from_secs(30),
        }
    }
    
    /// Create with file path for hot-reload
    pub async fn with_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = AppConfig::load_from_file(&path)?;
        let path_buf = path.as_ref().to_path_buf();
        
        let mut manager = Self::new(config);
        manager.config_path = Some(path_buf);
        
        // Set initial modification time
        if let Some(ref path) = manager.config_path {
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    let now = SystemTime::now();
                    let duration_since_modified = now.duration_since(modified).unwrap_or_default();
                    let modified_instant = Instant::now() - duration_since_modified;
                    let mut last_modified = manager.last_modified.write().await;
                    *last_modified = Some(modified_instant);
                }
            }
        }
        
        Ok(manager)
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }
    
    /// Update configuration
    pub async fn update_config(&self, new_config: AppConfig) -> Result<()> {
        new_config.validate()?;
        
        let mut config = self.config.write().await;
        *config = new_config;
        
        info!("Configuration updated successfully");
        Ok(())
    }
    
    /// Check for configuration changes and reload if needed
    pub async fn check_for_reload(&self) -> Result<bool> {
        if let Some(ref path) = self.config_path {
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    let now = SystemTime::now();
                    let duration_since_modified = now.duration_since(modified).unwrap_or_default();
                    let modified_instant = Instant::now() - duration_since_modified;
                    
                    let last_modified = *self.last_modified.read().await;
                    
                    if let Some(last) = last_modified {
                        if modified_instant > last {
                            info!("Configuration file changed, reloading...");
                            self.reload().await?;
                            return Ok(true);
                        }
                    }
                }
            }
        }
        
        Ok(false)
    }
    
    /// Force reload configuration from file
    pub async fn reload(&self) -> Result<()> {
        if let Some(ref path) = self.config_path {
            let new_config = AppConfig::load_from_file(path)?;
            self.update_config(new_config).await?;
            
            // Update last modified time
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    let now = SystemTime::now();
                    let duration_since_modified = now.duration_since(modified).unwrap_or_default();
                    let modified_instant = Instant::now() - duration_since_modified;
                    let mut last_modified = self.last_modified.write().await;
                    *last_modified = Some(modified_instant);
                }
            }
        }
        
        Ok(())
    }
    
    /// Start background hot-reload task
    pub async fn start_hot_reload(&self) -> tokio::task::JoinHandle<()> {
        let config_manager = self.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config_manager.reload_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = config_manager.check_for_reload().await {
                    error!("Failed to check for configuration reload: {:?}", e);
                }
            }
        })
    }
}

impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            config_path: self.config_path.clone(),
            last_modified: self.last_modified.clone(),
            reload_interval: self.reload_interval,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.environment, Environment::Development);
        assert!(!config.rpc.endpoints.is_empty());
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        
        // Invalid: no RPC endpoints
        config.rpc.endpoints.clear();
        assert!(config.validate().is_err());
        
        // Valid: add endpoint back
        config.rpc.endpoints.push(RpcEndpointConfig {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 100,
            timeout_ms: 30000,
            enabled: true,
        });
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_environment_configs() {
        let dev_config = AppConfig::for_environment(Environment::Development);
        let prod_config = AppConfig::for_environment(Environment::Production);
        
        assert_eq!(dev_config.environment, Environment::Development);
        assert_eq!(prod_config.environment, Environment::Production);
        
        // Production should have higher limits
        assert!(prod_config.rpc.rate_limit_rps > dev_config.rpc.rate_limit_rps);
        assert!(prod_config.scanner.max_concurrent_scans > dev_config.scanner.max_concurrent_scans);
    }
    
    #[test]
    fn test_config_save_and_load() -> Result<()> {
        let original_config = AppConfig::default();
        
        // Create temporary file
        let mut temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path();
        
        // Save config
        original_config.save_to_file(temp_path)?;
        
        // Load config
        let loaded_config = AppConfig::load_from_file(temp_path)?;
        
        // Compare
        assert_eq!(original_config.environment, loaded_config.environment);
        assert_eq!(original_config.rpc.endpoints.len(), loaded_config.rpc.endpoints.len());
        
        Ok(())
    }
    
    #[test]
    fn test_env_config_loading() {
        // Set environment variables
        env::set_var("ENVIRONMENT", "production");
        env::set_var("RPC_POOL_SIZE", "16");
        env::set_var("SCANNER_PERFORMANCE_MODE", "ultra_fast");
        
        let config = AppConfig::load_from_env().unwrap();
        
        assert_eq!(config.environment, Environment::Production);
        assert_eq!(config.rpc.pool_size, 16);
        assert_eq!(config.scanner.performance_mode, "ultra_fast");
        
        // Clean up
        env::remove_var("ENVIRONMENT");
        env::remove_var("RPC_POOL_SIZE");
        env::remove_var("SCANNER_PERFORMANCE_MODE");
    }
}
