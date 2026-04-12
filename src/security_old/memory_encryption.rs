//! Memory encryption for sensitive key data
//! Provides secure in-memory encryption for private keys and sensitive data

use crate::core::{Result, SolanaRecoverError};
use std::sync::Arc;
use tokio::sync::RwLock;
use aes_gcm::{Aes256Gcm, Key, Nonce, NewAead};
use aes_gcm::aead::{Aead, OsRng};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng as ArgonRng, SaltString};
use zeroize::{Zeroize, ZeroizeOnDrop};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use tracing::{debug, warn, error, info};
use std::time::{Duration, Instant};

/// Memory encryption manager for sensitive data
pub struct MemoryEncryptionManager {
    /// AES-GCM cipher for encryption
    cipher: Arc<RwLock<Option<Aes256Gcm>>>,
    /// Key derivation parameters
    key_derivation: KeyDerivationConfig,
    /// Encrypted memory regions
    encrypted_regions: Arc<RwLock<HashMap<String, EncryptedMemoryRegion>>>,
    /// Encryption statistics
    stats: Arc<RwLock<EncryptionStats>>,
    /// Security configuration
    config: SecurityConfig,
}

/// Configuration for key derivation
#[derive(Debug, Clone)]
pub struct KeyDerivationConfig {
    /// Master key password (from environment or secure storage)
    pub master_password: Option<String>,
    /// Key derivation algorithm
    pub algorithm: KeyDerivationAlgorithm,
    /// Memory hardening iterations
    pub memory_cost: u32,
    /// Time cost (iterations)
    pub time_cost: u32,
    /// Parallelism factor
    pub parallelism: u32,
    /// Salt for key derivation
    pub salt: Option<[u8; 32]>,
}

/// Key derivation algorithms
#[derive(Debug, Clone)]
pub enum KeyDerivationAlgorithm {
    Argon2id,
    Argon2i,
    Argon2d,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable memory encryption
    pub enabled: bool,
    /// Auto-rotate keys after duration
    pub key_rotation_interval: Duration,
    /// Secure memory cleanup on drop
    pub secure_cleanup: bool,
    /// Memory region timeout
    pub region_timeout: Duration,
    /// Enable integrity verification
    pub enable_integrity_check: bool,
    /// Maximum encrypted region size
    pub max_region_size: usize,
}

/// Encrypted memory region
#[derive(Debug, Clone)]
struct EncryptedMemoryRegion {
    /// Encrypted data
    ciphertext: Vec<u8>,
    /// Nonce for encryption
    nonce: Vec<u8>,
    /// Authentication tag
    tag: Vec<u8>,
    /// Creation timestamp
    created_at: std::time::SystemTime,
    /// Last access timestamp
    last_accessed: std::time::SystemTime,
    /// Data size
    size: usize,
    /// Region metadata
    metadata: HashMap<String, String>,
}

/// Token bucket state
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Current token count
    tokens: f64,
    /// Maximum tokens (burst size)
    max_tokens: f64,
    /// Last refill timestamp
    last_refill: std::time::SystemTime,
    /// Request count
    request_count: u64,
    /// Denied request count
    denied_count: u64,
    /// Penalty end time
    penalty_end: Option<std::time::SystemTime>,
    /// Client identifier
    client_id: String,
    /// Bucket creation time
    created_at: std::time::SystemTime,
}

/// Encryption statistics
#[derive(Debug, Default, Clone)]
pub struct EncryptionStats {
    /// Total encryptions performed
    pub total_encryptions: u64,
    /// Total decryptions performed
    pub total_decryptions: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Memory encrypted (bytes)
    pub memory_encrypted_bytes: u64,
    /// Memory decrypted (bytes)
    pub memory_decrypted_bytes: u64,
    /// Key rotations
    pub key_rotations: u64,
    /// Active regions
    pub active_regions: usize,
    /// Average encryption time (microseconds)
    pub avg_encryption_time_us: f64,
    /// Average decryption time (microseconds)
    pub avg_decryption_time_us: f64,
}

/// Secure memory buffer that auto-encrypts/decrypts
pub struct SecureBuffer {
    /// Buffer identifier
    id: String,
    /// Encryption manager
    encryption_manager: Arc<MemoryEncryptionManager>,
    /// Current data (unencrypted)
    data: Vec<u8>,
    /// Is data currently encrypted
    is_encrypted: bool,
    /// Last access time
    last_access: Instant,
}

impl Default for KeyDerivationConfig {
    fn default() -> Self {
        Self {
            master_password: std::env::var("MEMORY_ENCRYPTION_PASSWORD").ok(),
            algorithm: KeyDerivationAlgorithm::Argon2id,
            memory_cost: 65536, // 64MB
            time_cost: 3,
            parallelism: 4,
            salt: None,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("MEMORY_ENCRYPTION_ENABLED")
                .unwrap_or_else(|_| "true".to_string()) == "true",
            key_rotation_interval: Duration::from_secs(3600), // 1 hour
            secure_cleanup: true,
            region_timeout: Duration::from_secs(1800), // 30 minutes
            enable_integrity_check: true,
            max_region_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

impl MemoryEncryptionManager {
    /// Create new memory encryption manager
    pub fn new(config: SecurityConfig) -> Result<Self> {
        let key_derivation = KeyDerivationConfig::default();
        
        Ok(Self {
            cipher: Arc::new(RwLock::new(None)),
            key_derivation,
            encrypted_regions: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(EncryptionStats::default())),
            config,
        })
    }

    /// Initialize encryption manager
    pub async fn initialize(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Memory encryption is disabled");
            return Ok(());
        }

        // Derive encryption key
        let key = self.derive_encryption_key().await?;
        
        // Initialize cipher
        let cipher = Aes256Gcm::new(&key);
        *self.cipher.write().await = Some(cipher);
        
        info!("Memory encryption manager initialized successfully");
        
        // Start background tasks
        self.start_background_tasks().await;
        
        Ok(())
    }

    /// Derive encryption key from master password
    async fn derive_encryption_key(&self) -> Result<Key<Aes256Gcm>> {
        let password = self.key_derivation.master_password.as_ref()
            .ok_or_else(|| SolanaRecoverError::SecurityError(
                "No master password provided for memory encryption".to_string()
            ))?;

        let salt = match &self.key_derivation.salt {
            Some(salt) => *salt,
            None => {
                // Generate random salt
                let mut salt = [0u8; 32];
                use rand::RngCore;
                OsRng.fill_bytes(&mut salt);
                salt
            }
        };

        let params = match self.key_derivation.algorithm {
            KeyDerivationAlgorithm::Argon2id => argon2::Params::new(
                self.key_derivation.memory_cost,
                self.key_derivation.time_cost,
                self.key_derivation.parallelism,
                Some(32),
            )?,
            KeyDerivationAlgorithm::Argon2i => argon2::Params::new(
                self.key_derivation.memory_cost,
                self.key_derivation.time_cost,
                self.key_derivation.parallelism,
                Some(32),
            )?,
            KeyDerivationAlgorithm::Argon2d => argon2::Params::new(
                self.key_derivation.memory_cost,
                self.key_derivation.time_cost,
                self.key_derivation.parallelism,
                Some(32),
            )?,
        };

        let argon2 = Argon2::new(
            match self.key_derivation.algorithm {
                KeyDerivationAlgorithm::Argon2id => argon2::Algorithm::Argon2id,
                KeyDerivationAlgorithm::Argon2i => argon2::Algorithm::Argon2i,
                KeyDerivationAlgorithm::Argon2d => argon2::Algorithm::Argon2d,
            },
            argon2::Version::V0x13,
            params,
        );

        let salt_string = SaltString::encode_b64(&salt)
            .map_err(|e| SolanaRecoverError::SecurityError(
                format!("Failed to encode salt: {}", e)
            ))?;

        let password_hash = argon2.hash_password(password.as_bytes(), &salt_string)
            .map_err(|e| SolanaRecoverError::SecurityError(
                format!("Failed to hash password: {}", e)
            ))?;

        let hash = PasswordHash::new(&password_hash.to_string())
            .map_err(|e| SolanaRecoverError::SecurityError(
                format!("Failed to parse password hash: {}", e)
            ))?;

        // Extract key from hash (simplified approach)
        let hash_bytes = hash.hash.unwrap();
        let key_bytes = &hash_bytes.as_bytes()[..32];
        
        let key = Key::from_slice(key_bytes);
        Ok(*key)
    }

    /// Encrypt data in memory
    pub async fn encrypt(&self, data: &[u8], region_id: &str) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        if data.len() > self.config.max_region_size {
            return Err(SolanaRecoverError::SecurityError(
                format!("Data size {} exceeds maximum region size {}", 
                       data.len(), self.config.max_region_size)
            ));
        }

        let start_time = std::time::Instant::now();
        
        let cipher_guard = self.cipher.read().await;
        let cipher = cipher_guard.as_ref()
            .ok_or_else(|| SolanaRecoverError::SecurityError(
                "Encryption cipher not initialized".to_string()
            ))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt data
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| SolanaRecoverError::SecurityError(
                format!("Encryption failed: {}", e)
            ))?;

        // Split ciphertext and tag (for storage)
        let tag_offset = ciphertext.len() - 16; // GCM tag is 16 bytes
        let encrypted_data = ciphertext[..tag_offset].to_vec();
        let tag = ciphertext[tag_offset..].to_vec();

        // Store encrypted region
        let region = EncryptedMemoryRegion {
            ciphertext: encrypted_data,
            nonce: nonce_bytes.to_vec(),
            tag,
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            size: data.len(),
            metadata: HashMap::new(),
        };

        let mut regions = self.encrypted_regions.write().await;
        regions.insert(region_id.to_string(), region);

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_encryptions += 1;
        stats.memory_encrypted_bytes += data.len() as u64;
        stats.active_regions = regions.len();
        let encryption_time = start_time.elapsed().as_micros() as f64;
        stats.avg_encryption_time_us = 
            (stats.avg_encryption_time_us * (stats.total_encryptions - 1) as f64 + encryption_time)
            / stats.total_encryptions as f64;

        debug!("Encrypted {} bytes for region '{}'", data.len(), region_id);
        Ok(())
    }

    /// Decrypt data from memory
    pub async fn decrypt(&self, region_id: &str) -> Result<Vec<u8>> {
        if !self.config.enabled {
            return Err(SolanaRecoverError::SecurityError(
                "Memory encryption is disabled".to_string()
            ));
        }

        let start_time = std::time::Instant::now();
        
        let cipher_guard = self.cipher.read().await;
        let cipher = cipher_guard.as_ref()
            .ok_or_else(|| SolanaRecoverError::SecurityError(
                "Encryption cipher not initialized".to_string()
            ))?;

        let mut regions = self.encrypted_regions.write().await;
        let region = regions.get_mut(region_id)
            .ok_or_else(|| SolanaRecoverError::SecurityError(
                format!("Encrypted region '{}' not found", region_id)
            ))?;

        // Update last access time
        region.last_accessed = Instant::now();

        // Reconstruct ciphertext with tag
        let mut ciphertext = region.ciphertext.clone();
        ciphertext.extend_from_slice(&region.tag);

        let nonce = Nonce::from_slice(&region.nonce);

        // Decrypt data
        let plaintext = cipher.decrypt(nonce, ciphertext.as_slice())
            .map_err(|e| SolanaRecoverError::SecurityError(
                format!("Decryption failed: {}", e)
            ))?;

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_decryptions += 1;
        stats.memory_decrypted_bytes += plaintext.len() as u64;
        let decryption_time = start_time.elapsed().as_micros() as f64;
        stats.avg_decryption_time_us = 
            (stats.avg_decryption_time_us * (stats.total_decryptions - 1) as f64 + decryption_time)
            / stats.total_decryptions as f64;

        debug!("Decrypted {} bytes for region '{}'", plaintext.len(), region_id);
        Ok(plaintext)
    }

    /// Create secure buffer
    pub async fn create_secure_buffer(&self, data: Vec<u8>, id: String) -> Result<SecureBuffer> {
        let buffer = SecureBuffer {
            id: id.clone(),
            encryption_manager: Arc::new(self.clone()),
            data,
            is_encrypted: false,
            last_access: Instant::now(),
        };

        // Encrypt initial data
        self.encrypt(&buffer.data, &id).await?;

        Ok(buffer)
    }

    /// Delete encrypted region
    pub async fn delete_region(&self, region_id: &str) -> Result<()> {
        let mut regions = self.encrypted_regions.write().await;
        
        if let Some(mut region) = regions.remove(region_id) {
            // Securely zero out data
            region.ciphertext.zeroize();
            region.nonce.zeroize();
            region.tag.zeroize();
            
            debug!("Deleted encrypted region '{}'", region_id);
        }

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.active_regions = regions.len();

        Ok(())
    }

    /// Rotate encryption key
    pub async fn rotate_key(&self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        info!("Starting encryption key rotation");

        // Decrypt all regions with old key
        let regions_to_reencrypt = {
            let regions = self.encrypted_regions.read().await;
            regions.keys().cloned().collect::<Vec<_>>()
        };

        let mut decrypted_data = Vec::new();
        for region_id in &regions_to_reencrypt {
            match self.decrypt(region_id).await {
                Ok(data) => decrypted_data.push((region_id.clone(), data)),
                Err(e) => {
                    error!("Failed to decrypt region '{}' during key rotation: {}", region_id, e);
                    return Err(e);
                }
            }
        }

        // Derive new key
        let new_key = self.derive_encryption_key().await?;
        let new_cipher = Aes256Gcm::new(&new_key);
        *self.cipher.write().await = Some(new_cipher);

        // Re-encrypt all regions with new key
        for (region_id, data) in decrypted_data {
            self.encrypt(&data, &region_id).await?;
        }

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.key_rotations += 1;

        info!("Encryption key rotation completed successfully");
        Ok(())
    }

    /// Cleanup expired regions
    pub async fn cleanup_expired_regions(&self) -> Result<usize> {
        let mut regions = self.encrypted_regions.write().await;
        let initial_count = regions.len();

        regions.retain(|region_id, region| {
            let is_expired = region.last_accessed.elapsed() > self.config.region_timeout;
            if is_expired {
                debug!("Removing expired encrypted region '{}'", region_id);
                
                // Securely zero out data
                region.ciphertext.zeroize();
                region.nonce.zeroize();
                region.tag.zeroize();
            }
            !is_expired
        });

        let removed_count = initial_count - regions.len();

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.active_regions = regions.len();

        Ok(removed_count)
    }

    /// Get encryption statistics
    pub async fn get_stats(&self) -> EncryptionStats {
        self.stats.read().await.clone()
    }

    /// Get security status
    pub async fn get_security_status(&self) -> serde_json::Value {
        let stats = self.get_stats().await;
        let regions_count = self.encrypted_regions.read().await.len();

        serde_json::json!({
            "enabled": self.config.enabled,
            "active_regions": regions_count,
            "total_encryptions": stats.total_encryptions,
            "total_decryptions": stats.total_decryptions,
            "failed_operations": stats.failed_operations,
            "memory_encrypted_bytes": stats.memory_encrypted_bytes,
            "memory_decrypted_bytes": stats.memory_decrypted_bytes,
            "key_rotations": stats.key_rotations,
            "avg_encryption_time_us": stats.avg_encryption_time_us,
            "avg_decryption_time_us": stats.avg_decryption_time_us,
            "config": self.config,
        })
    }

    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Key rotation task
        let manager_clone = self.clone();
        let rotation_interval = self.config.key_rotation_interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(rotation_interval);
            loop {
                interval.tick().await;
                if let Err(e) = manager_clone.rotate_key().await {
                    error!("Key rotation failed: {}", e);
                }
            }
        });

        // Cleanup task
        let manager_clone = self.clone();
        let cleanup_interval = Duration::from_secs(300); // 5 minutes
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                if let Ok(removed) = manager_clone.cleanup_expired_regions().await {
                    if removed > 0 {
                        info!("Cleaned up {} expired encrypted regions", removed);
                    }
                }
            }
        });
    }
}

impl Clone for MemoryEncryptionManager {
    fn clone(&self) -> Self {
        Self {
            cipher: Arc::clone(&self.cipher),
            key_derivation: self.key_derivation.clone(),
            encrypted_regions: Arc::clone(&self.encrypted_regions),
            stats: Arc::clone(&self.stats),
            config: self.config.clone(),
        }
    }
}

impl SecureBuffer {
    /// Get data (decrypts if needed)
    pub async fn get_data(&mut self) -> Result<&[u8]> {
        if self.is_encrypted {
            let decrypted = self.encryption_manager.decrypt(&self.id).await?;
            self.data = decrypted;
            self.is_encrypted = false;
            self.last_access = Instant::now();
        }
        Ok(&self.data)
    }

    /// Update data (encrypts after update)
    pub async fn update_data(&mut self, new_data: Vec<u8>) -> Result<()> {
        self.data = new_data;
        self.last_access = Instant::now();
        
        // Encrypt new data
        self.encryption_manager.encrypt(&self.data, &self.id).await?;
        self.is_encrypted = true;
        
        Ok(())
    }

    /// Force encryption of current data
    pub async fn encrypt(&mut self) -> Result<()> {
        if !self.is_encrypted {
            self.encryption_manager.encrypt(&self.data, &self.id).await?;
            self.is_encrypted = true;
        }
        Ok(())
    }

    /// Get buffer size
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Check if buffer is currently encrypted
    pub fn is_encrypted(&self) -> bool {
        self.is_encrypted
    }
}

impl Drop for SecureBuffer {
    fn drop(&mut self) {
        if self.encryption_manager.config.secure_cleanup {
            self.data.zeroize();
        }
    }
}

impl Drop for EncryptedMemoryRegion {
    fn drop(&mut self) {
        self.ciphertext.zeroize();
        self.nonce.zeroize();
        self.tag.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_encryption_basic() {
        let config = SecurityConfig {
            enabled: true,
            ..Default::default()
        };
        
        let mut key_derivation = KeyDerivationConfig::default();
        key_derivation.master_password = Some("test_password".to_string());
        
        let mut manager = MemoryEncryptionManager::new(config);
        manager.key_derivation = key_derivation;
        
        manager.initialize().await.unwrap();

        let test_data = b"sensitive private key data";
        let region_id = "test_region";

        // Encrypt data
        manager.encrypt(test_data, region_id).await.unwrap();

        // Decrypt data
        let decrypted = manager.decrypt(region_id).await.unwrap();
        assert_eq!(decrypted, test_data);

        // Check statistics
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_encryptions, 1);
        assert_eq!(stats.total_decryptions, 1);
    }

    #[tokio::test]
    async fn test_secure_buffer() {
        let config = SecurityConfig {
            enabled: true,
            ..Default::default()
        };
        
        let mut key_derivation = KeyDerivationConfig::default();
        key_derivation.master_password = Some("test_password".to_string());
        
        let mut manager = MemoryEncryptionManager::new(config);
        manager.key_derivation = key_derivation;
        
        manager.initialize().await.unwrap();

        let initial_data = b"initial data".to_vec();
        let mut buffer = manager.create_secure_buffer(initial_data, "test_buffer".to_string()).await.unwrap();

        // Data should be encrypted initially
        assert!(buffer.is_encrypted());

        // Get data should decrypt
        let data = buffer.get_data().await.unwrap();
        assert_eq!(data, b"initial data");
        assert!(!buffer.is_encrypted());

        // Update data should re-encrypt
        let new_data = b"new data".to_vec();
        buffer.update_data(new_data.clone()).await.unwrap();
        assert!(buffer.is_encrypted());

        // Get updated data
        let data = buffer.get_data().await.unwrap();
        assert_eq!(data, new_data);
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let config = SecurityConfig {
            enabled: true,
            ..Default::default()
        };
        
        let mut key_derivation = KeyDerivationConfig::default();
        key_derivation.master_password = Some("test_password".to_string());
        
        let mut manager = MemoryEncryptionManager::new(config);
        manager.key_derivation = key_derivation;
        
        manager.initialize().await.unwrap();

        let test_data = b"test data for rotation";
        let region_id = "rotation_test";

        // Encrypt with original key
        manager.encrypt(test_data, region_id).await.unwrap();

        // Rotate key
        manager.rotate_key().await.unwrap();

        // Should still be able to decrypt with new key
        let decrypted = manager.decrypt(region_id).await.unwrap();
        assert_eq!(decrypted, test_data);

        let stats = manager.get_stats().await;
        assert_eq!(stats.key_rotations, 1);
    }

    #[tokio::test]
    async fn test_cleanup_expired_regions() {
        let config = SecurityConfig {
            enabled: true,
            region_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        
        let mut key_derivation = KeyDerivationConfig::default();
        key_derivation.master_password = Some("test_password".to_string());
        
        let mut manager = MemoryEncryptionManager::new(config);
        manager.key_derivation = key_derivation;
        
        manager.initialize().await.unwrap();

        let test_data = b"test data";
        let region_id = "cleanup_test";

        // Encrypt data
        manager.encrypt(test_data, region_id).await.unwrap();

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Cleanup expired regions
        let removed = manager.cleanup_expired_regions().await.unwrap();
        assert_eq!(removed, 1);

        // Should not be able to decrypt anymore
        let result = manager.decrypt(region_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_security_status() {
        let config = SecurityConfig {
            enabled: true,
            ..Default::default()
        };
        
        let mut key_derivation = KeyDerivationConfig::default();
        key_derivation.master_password = Some("test_password".to_string());
        
        let mut manager = MemoryEncryptionManager::new(config);
        manager.key_derivation = key_derivation;
        
        manager.initialize().await.unwrap();

        let status = manager.get_security_status().await;
        
        assert!(status["enabled"].as_bool().unwrap());
        assert_eq!(status["total_encryptions"].as_u64().unwrap(), 0);
        assert_eq!(status["total_decryptions"].as_u64().unwrap(), 0);
    }
}
