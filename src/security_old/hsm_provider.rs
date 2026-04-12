//! Hardware Security Module (HSM) provider for enhanced key security
//! Supports various HSM implementations for secure private key operations

use crate::core::{Result, SolanaRecoverError};
use solana_sdk::signature::{Keypair, Signer, Signature};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::Transaction;
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use tracing::{info, warn, debug, error};
use tokio::sync::RwLock;
use uuid::Uuid;

/// HSM provider trait for different HSM implementations
#[async_trait]
pub trait HsmProvider: Send + Sync {
    /// Initialize HSM connection
    async fn initialize(&mut self) -> Result<()>;
    
    /// Check if HSM is available
    async fn is_available(&self) -> bool;
    
    /// Generate new key in HSM
    async fn generate_key(&self, key_id: &str) -> Result<String>;
    
    /// Import key into HSM
    async fn import_key(&self, key_id: &str, private_key: &[u8]) -> Result<()>;
    
    /// Sign data with HSM key
    async fn sign(&self, key_id: &str, message: &[u8]) -> Result<Signature>;
    
    /// Get public key for HSM key
    async fn get_public_key(&self, key_id: &str) -> Result<Pubkey>;
    
    /// Delete key from HSM
    async fn delete_key(&self, key_id: &str) -> Result<()>;
    
    /// List all keys in HSM
    async fn list_keys(&self) -> Result<Vec<String>>;
    
    /// Get HSM provider info
    fn get_provider_info(&self) -> HsmProviderInfo;
}

/// HSM provider information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsmProviderInfo {
    /// Provider name
    pub name: String,
    /// Provider version
    pub version: String,
    /// Supported operations
    pub supported_operations: Vec<String>,
    /// Security level
    pub security_level: SecurityLevel,
    /// Provider type
    pub provider_type: HsmProviderType,
}

/// Security level for HSM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// Software-based (for testing)
    Software,
    /// Basic hardware security
    Basic,
    /// High security hardware
    High,
    /// Military grade security
    Military,
}

/// HSM provider types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HsmProviderType {
    /// Software simulation (for testing)
    Software,
    /// Network HSM
    Network,
    /// USB/PCIe HSM
    Hardware,
    /// Cloud HSM service
    Cloud,
}

/// HSM manager for managing multiple providers
pub struct HsmManager {
    /// Active HSM providers
    providers: Arc<RwLock<HashMap<String, Arc<dyn HsmProvider>>>>,
    /// Default provider
    default_provider: Arc<RwLock<Option<String>>>,
    /// Key to provider mapping
    key_mapping: Arc<RwLock<HashMap<String, String>>>,
    /// Configuration
    config: HsmConfig,
}

/// HSM configuration
#[derive(Debug, Clone)]
pub struct HsmConfig {
    /// Enable HSM for key operations
    pub enabled: bool,
    /// Default provider name
    pub default_provider: String,
    /// Fallback to software if HSM unavailable
    pub fallback_to_software: bool,
    /// Cache HSM operations
    pub cache_operations: bool,
    /// Operation timeout in seconds
    pub operation_timeout_secs: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Enable audit logging
    pub enable_audit_logging: bool,
}

/// Software HSM provider for testing and development
pub struct SoftwareHsmProvider {
    /// Provider info
    info: HsmProviderInfo,
    /// In-memory key store
    keys: Arc<RwLock<HashMap<String, Keypair>>>,
    /// Initialization status
    initialized: Arc<RwLock<bool>>,
}

/// Network HSM provider (placeholder for real implementation)
pub struct NetworkHsmProvider {
    /// Provider info
    info: HsmProviderInfo,
    /// HSM endpoint
    endpoint: String,
    /// Authentication token
    auth_token: String,
    /// Connection status
    connected: Arc<RwLock<bool>>,
}

impl Default for HsmConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("HSM_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true",
            default_provider: std::env::var("HSM_DEFAULT_PROVIDER").unwrap_or_else(|_| "software".to_string()),
            fallback_to_software: true,
            cache_operations: true,
            operation_timeout_secs: 30,
            max_retries: 3,
            enable_audit_logging: true,
        }
    }
}

impl HsmManager {
    /// Create new HSM manager
    pub fn new(config: HsmConfig) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            default_provider: Arc::new(RwLock::new(Some(config.default_provider.clone()))),
            key_mapping: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Initialize HSM manager with default providers
    pub async fn initialize(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("HSM is disabled in configuration");
            return Ok(());
        }

        // Add software provider for testing/fallback
        let software_provider = Arc::new(SoftwareHsmProvider::new());
        self.add_provider("software".to_string(), software_provider).await?;

        // Add network provider if configured
        if let Ok(endpoint) = std::env::var("HSM_NETWORK_ENDPOINT") {
            if let Ok(auth_token) = std::env::var("HSM_AUTH_TOKEN") {
                let network_provider = Arc::new(NetworkHsmProvider::new(
                    endpoint,
                    auth_token,
                ));
                self.add_provider("network".to_string(), network_provider).await?;
            }
        }

        // Initialize default provider
        if let Some(default_name) = self.default_provider.read().await.clone() {
            if let Some(provider) = self.providers.read().await.get(&default_name) {
                let mut provider_mut = provider.clone();
                provider_mut.initialize().await?;
                info!("HSM provider '{}' initialized successfully", default_name);
            }
        }

        info!("HSM manager initialized with {} providers", 
              self.providers.read().await.len());
        Ok(())
    }

    /// Add HSM provider
    pub async fn add_provider(&self, name: String, provider: Arc<dyn HsmProvider>) -> Result<()> {
        let mut providers = self.providers.write().await;
        providers.insert(name.clone(), provider);
        info!("Added HSM provider: {}", name);
        Ok(())
    }

    /// Get HSM provider by name
    async fn get_provider(&self, name: Option<&str>) -> Result<Arc<dyn HsmProvider>> {
        let providers = self.providers.read().await;
        
        let provider_name = name.or_else(|| {
            self.default_provider.read().await.as_ref().map(|s| s.as_str())
        }).ok_or_else(|| {
            SolanaRecoverError::ConfigError("No HSM provider specified".to_string())
        })?;

        providers.get(provider_name)
            .cloned()
            .ok_or_else(|| {
                SolanaRecoverError::ConfigError(
                    format!("HSM provider '{}' not found", provider_name)
                )
            })
    }

    /// Generate new key in HSM
    pub async fn generate_key(&self, key_id: &str, provider_name: Option<&str>) -> Result<String> {
        let provider = self.get_provider(provider_name).await?;
        
        if !provider.is_available().await {
            if self.config.fallback_to_software && provider_name != Some("software") {
                warn!("HSM provider unavailable, falling back to software");
                return self.generate_key(key_id, Some("software")).await;
            }
            return Err(SolanaRecoverError::NetworkError("HSM provider unavailable".to_string()));
        }

        let result = provider.generate_key(key_id).await?;
        
        // Map key to provider
        let mut key_mapping = self.key_mapping.write().await;
        key_mapping.insert(key_id.to_string(), provider.get_provider_info().name);

        self.log_audit_event("key_generated", key_id, &provider.get_provider_info().name).await;
        
        Ok(result)
    }

    /// Import private key into HSM
    pub async fn import_key(&self, key_id: &str, private_key: &[u8], provider_name: Option<&str>) -> Result<()> {
        let provider = self.get_provider(provider_name).await?;
        
        if !provider.is_available().await {
            if self.config.fallback_to_software && provider_name != Some("software") {
                warn!("HSM provider unavailable, falling back to software");
                return self.import_key(key_id, private_key, Some("software")).await;
            }
            return Err(SolanaRecoverError::NetworkError("HSM provider unavailable".to_string()));
        }

        provider.import_key(key_id, private_key).await?;
        
        // Map key to provider
        let mut key_mapping = self.key_mapping.write().await;
        key_mapping.insert(key_id.to_string(), provider.get_provider_info().name);

        self.log_audit_event("key_imported", key_id, &provider.get_provider_info().name).await;
        
        Ok(())
    }

    /// Sign data with HSM key
    pub async fn sign(&self, key_id: &str, message: &[u8]) -> Result<Signature> {
        let key_mapping = self.key_mapping.read().await;
        let provider_name = key_mapping.get(key_id)
            .ok_or_else(|| SolanaRecoverError::InvalidKey(
                format!("Key '{}' not found in HSM", key_id)
            ))?;
        
        let provider = self.get_provider(Some(provider_name)).await?;
        
        if !provider.is_available().await {
            if self.config.fallback_to_software && provider_name != "software" {
                warn!("HSM provider unavailable, falling back to software");
                return self.sign(key_id, message);
            }
            return Err(SolanaRecoverError::NetworkError("HSM provider unavailable".to_string()));
        }

        let signature = provider.sign(key_id, message).await?;
        
        self.log_audit_event("data_signed", key_id, provider_name).await;
        
        Ok(signature)
    }

    /// Get public key for HSM key
    pub async fn get_public_key(&self, key_id: &str) -> Result<Pubkey> {
        let key_mapping = self.key_mapping.read().await;
        let provider_name = key_mapping.get(key_id)
            .ok_or_else(|| SolanaRecoverError::InvalidKey(
                format!("Key '{}' not found in HSM", key_id)
            ))?;
        
        let provider = self.get_provider(Some(provider_name)).await?;
        provider.get_public_key(key_id).await
    }

    /// Delete key from HSM
    pub async fn delete_key(&self, key_id: &str) -> Result<()> {
        let key_mapping = self.key_mapping.read().await;
        let provider_name = key_mapping.get(key_id)
            .ok_or_else(|| SolanaRecoverError::InvalidKey(
                format!("Key '{}' not found in HSM", key_id)
            ))?;
        
        let provider = self.get_provider(Some(provider_name)).await?;
        provider.delete_key(key_id).await?;
        
        // Remove from mapping
        drop(key_mapping);
        let mut key_mapping = self.key_mapping.write().await;
        key_mapping.remove(key_id);

        self.log_audit_event("key_deleted", key_id, provider_name).await;
        
        Ok(())
    }

    /// List all keys in HSM
    pub async fn list_keys(&self) -> Result<Vec<String>> {
        let mut all_keys = Vec::new();
        let providers = self.providers.read().await;
        
        for (name, provider) in providers.iter() {
            if provider.is_available().await {
                let keys = provider.list_keys().await?;
                all_keys.extend(keys);
            }
        }
        
        Ok(all_keys)
    }

    /// Get HSM status
    pub async fn get_status(&self) -> serde_json::Value {
        let providers = self.providers.read().await;
        let mut provider_status = Vec::new();
        
        for (name, provider) in providers.iter() {
            let info = provider.get_provider_info();
            provider_status.push(serde_json::json!({
                "name": name,
                "info": info,
                "available": provider.is_available().await,
            }));
        }

        serde_json::json!({
            "enabled": self.config.enabled,
            "default_provider": *self.default_provider.read().await,
            "providers": provider_status,
            "total_keys": self.key_mapping.read().await.len(),
            "config": self.config,
        })
    }

    /// Log audit event
    async fn log_audit_event(&self, event_type: &str, key_id: &str, provider_name: &str) {
        if !self.config.enable_audit_logging {
            return;
        }

        info!("HSM Audit: {} - Key: {} - Provider: {} - Timestamp: {}", 
              event_type, key_id, provider_name, chrono::Utc::now());
    }
}

impl SoftwareHsmProvider {
    /// Create new software HSM provider
    pub fn new() -> Self {
        Self {
            info: HsmProviderInfo {
                name: "Software HSM".to_string(),
                version: "1.0.0".to_string(),
                supported_operations: vec![
                    "generate_key".to_string(),
                    "import_key".to_string(),
                    "sign".to_string(),
                    "get_public_key".to_string(),
                    "delete_key".to_string(),
                    "list_keys".to_string(),
                ],
                security_level: SecurityLevel::Software,
                provider_type: HsmProviderType::Software,
            },
            keys: Arc::new(RwLock::new(HashMap::new())),
            initialized: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl HsmProvider for SoftwareHsmProvider {
    async fn initialize(&mut self) -> Result<()> {
        *self.initialized.write().await = true;
        info!("Software HSM provider initialized");
        Ok(())
    }

    async fn is_available(&self) -> bool {
        *self.initialized.read().await
    }

    async fn generate_key(&self, key_id: &str) -> Result<String> {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey().to_string();
        
        let mut keys = self.keys.write().await;
        keys.insert(key_id.to_string(), keypair);
        
        debug!("Generated key '{}' in software HSM", key_id);
        Ok(pubkey)
    }

    async fn import_key(&self, key_id: &str, private_key: &[u8]) -> Result<()> {
        let keypair = Keypair::from_bytes(private_key)
            .map_err(|e| SolanaRecoverError::InvalidKey(
                format!("Failed to import key: {}", e)
            ))?;
        
        let mut keys = self.keys.write().await;
        keys.insert(key_id.to_string(), keypair);
        
        debug!("Imported key '{}' to software HSM", key_id);
        Ok(())
    }

    async fn sign(&self, key_id: &str, message: &[u8]) -> Result<Signature> {
        let keys = self.keys.read().await;
        let keypair = keys.get(key_id)
            .ok_or_else(|| SolanaRecoverError::InvalidKey(
                format!("Key '{}' not found", key_id)
            ))?;
        
        Ok(keypair.sign_message(message))
    }

    async fn get_public_key(&self, key_id: &str) -> Result<Pubkey> {
        let keys = self.keys.read().await;
        let keypair = keys.get(key_id)
            .ok_or_else(|| SolanaRecoverError::InvalidKey(
                format!("Key '{}' not found", key_id)
            ))?;
        
        Ok(keypair.pubkey())
    }

    async fn delete_key(&self, key_id: &str) -> Result<()> {
        let mut keys = self.keys.write().await;
        keys.remove(key_id)
            .ok_or_else(|| SolanaRecoverError::InvalidKey(
                format!("Key '{}' not found", key_id)
            ))?;
        
        debug!("Deleted key '{}' from software HSM", key_id);
        Ok(())
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        let keys = self.keys.read().await;
        Ok(keys.keys().cloned().collect())
    }

    fn get_provider_info(&self) -> HsmProviderInfo {
        self.info.clone()
    }
}

impl NetworkHsmProvider {
    /// Create new network HSM provider
    pub fn new(endpoint: String, auth_token: String) -> Self {
        Self {
            info: HsmProviderInfo {
                name: "Network HSM".to_string(),
                version: "1.0.0".to_string(),
                supported_operations: vec![
                    "generate_key".to_string(),
                    "import_key".to_string(),
                    "sign".to_string(),
                    "get_public_key".to_string(),
                    "delete_key".to_string(),
                    "list_keys".to_string(),
                ],
                security_level: SecurityLevel::High,
                provider_type: HsmProviderType::Network,
            },
            endpoint,
            auth_token,
            connected: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl HsmProvider for NetworkHsmProvider {
    async fn initialize(&mut self) -> Result<()> {
        // In a real implementation, this would establish connection to HSM
        *self.connected.write().await = true;
        info!("Network HSM provider initialized");
        Ok(())
    }

    async fn is_available(&self) -> bool {
        *self.connected.read().await
    }

    async fn generate_key(&self, _key_id: &str) -> Result<String> {
        // Placeholder implementation
        Err(SolanaRecoverError::NotImplemented(
            "Network HSM not fully implemented".to_string()
        ))
    }

    async fn import_key(&self, _key_id: &str, _private_key: &[u8]) -> Result<()> {
        // Placeholder implementation
        Err(SolanaRecoverError::NotImplemented(
            "Network HSM not fully implemented".to_string()
        ))
    }

    async fn sign(&self, _key_id: &str, _message: &[u8]) -> Result<Signature> {
        // Placeholder implementation
        Err(SolanaRecoverError::NotImplemented(
            "Network HSM not fully implemented".to_string()
        ))
    }

    async fn get_public_key(&self, _key_id: &str) -> Result<Pubkey> {
        // Placeholder implementation
        Err(SolanaRecoverError::NotImplemented(
            "Network HSM not fully implemented".to_string()
        ))
    }

    async fn delete_key(&self, _key_id: &str) -> Result<()> {
        // Placeholder implementation
        Err(SolanaRecoverError::NotImplemented(
            "Network HSM not fully implemented".to_string()
        ))
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        // Placeholder implementation
        Err(SolanaRecoverError::NotImplemented(
            "Network HSM not fully implemented".to_string()
        ))
    }

    fn get_provider_info(&self) -> HsmProviderInfo {
        self.info.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_software_hsm_provider() {
        let mut provider = SoftwareHsmProvider::new();
        provider.initialize().await.unwrap();
        
        assert!(provider.is_available().await);
        
        let key_id = "test_key";
        let pubkey = provider.generate_key(key_id).await.unwrap();
        assert!(!pubkey.is_empty());
        
        let retrieved_pubkey = provider.get_public_key(key_id).await.unwrap();
        assert_eq!(pubkey, retrieved_pubkey.to_string());
        
        let keys = provider.list_keys().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], key_id);
        
        provider.delete_key(key_id).await.unwrap();
        
        let keys = provider.list_keys().await.unwrap();
        assert_eq!(keys.len(), 0);
    }

    #[tokio::test]
    async fn test_hsm_manager() {
        let mut config = HsmConfig::default();
        config.enabled = true;
        config.default_provider = "software".to_string();
        
        let mut manager = HsmManager::new(config);
        manager.initialize().await.unwrap();
        
        let key_id = "manager_test_key";
        let pubkey = manager.generate_key(key_id, None).await.unwrap();
        assert!(!pubkey.is_empty());
        
        let retrieved_pubkey = manager.get_public_key(key_id).await.unwrap();
        assert_eq!(pubkey, retrieved_pubkey.to_string());
        
        let keys = manager.list_keys().await.unwrap();
        assert!(keys.contains(&key_id.to_string()));
        
        manager.delete_key(key_id).await.unwrap();
        
        let keys = manager.list_keys().await.unwrap();
        assert!(!keys.contains(&key_id.to_string()));
    }

    #[tokio::test]
    async fn test_hsm_signing() {
        let mut config = HsmConfig::default();
        config.enabled = true;
        
        let mut manager = HsmManager::new(config);
        manager.initialize().await.unwrap();
        
        let key_id = "sign_test_key";
        let _pubkey = manager.generate_key(key_id, None).await.unwrap();
        
        let message = b"test message to sign";
        let signature = manager.sign(key_id, message).await.unwrap();
        
        // Verify signature
        let pubkey = manager.get_public_key(key_id).await.unwrap();
        assert!(signature.verify(pubkey.as_ref(), message));
    }

    #[tokio::test]
    async fn test_hsm_status() {
        let mut config = HsmConfig::default();
        config.enabled = true;
        
        let mut manager = HsmManager::new(config);
        manager.initialize().await.unwrap();
        
        let status = manager.get_status().await;
        assert!(status["enabled"].as_bool().unwrap());
        assert!(status["providers"].as_array().unwrap().len() > 0);
    }
}
