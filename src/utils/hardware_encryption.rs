use std::sync::Arc;
use std::arch::x86_64::*;
use aes_gcm::{Aes256Gcm, Key, Nonce, NewAead};
use aes_gcm::aead::{Aead, AeadCore, OsRng};
use ring::aead::{AES_256_GCM, LessSafeKey, UnboundKey, Aad, NONCE_LEN};
use ring::{pbkdf2, rand};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use serde::{Deserialize, Serialize};

/// Hardware-accelerated encryption engine with AES-NI support
#[derive(Clone)]
pub struct HardwareEncryptionEngine {
    /// Primary encryption algorithm
    cipher: Arc<RwLock<Aes256Gcm>>,
    /// Ring-based encryption for hardware acceleration
    ring_cipher: Arc<RwLock<LessSafeKey>>,
    /// Performance metrics
    metrics: Arc<RwLock<EncryptionMetrics>>,
    /// Configuration
    config: EncryptionConfig,
    /// Hardware capability flags
    hardware_caps: HardwareCapabilities,
}

/// Encryption configuration
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Key derivation iterations
    pub key_derivation_iterations: u32,
    /// Enable hardware acceleration
    pub enable_hardware_acceleration: bool,
    /// Batch operation size
    pub batch_size: usize,
    /// Cache keys in memory
    pub cache_keys: bool,
    /// Enable parallel processing
    pub enable_parallel: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            key_derivation_iterations: 100_000,
            enable_hardware_acceleration: true,
            batch_size: 64,
            cache_keys: true,
            enable_parallel: true,
        }
    }
}

/// Hardware capabilities detection
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// AES-NI instruction set available
    pub aes_ni: bool,
    /// AVX2 instruction set available
    pub avx2: bool,
    /// AVX512 instruction set available
    pub avx512: bool,
    /// BMI2 instruction set available
    pub bmi2: bool,
    /// FMA instruction set available
    pub fma: bool,
}

/// Encryption performance metrics
#[derive(Debug, Default, Clone)]
pub struct EncryptionMetrics {
    /// Total encryption operations
    pub total_encryptions: u64,
    /// Total decryption operations
    pub total_decryptions: u64,
    /// Average encryption time (microseconds)
    pub avg_encryption_time_us: f64,
    /// Average decryption time (microseconds)
    pub avg_decryption_time_us: f64,
    /// Throughput (MB/s)
    pub throughput_mbps: f64,
    /// Hardware acceleration usage rate
    pub hardware_acceleration_rate: f64,
    /// Cache hit rate
    pub cache_hit_rate: f64,
}

/// Encrypted data wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Ciphertext data
    pub ciphertext: Vec<u8>,
    /// Authentication tag
    pub tag: Vec<u8>,
    /// Nonce used for encryption
    pub nonce: Vec<u8>,
    /// Encryption algorithm used
    pub algorithm: String,
    /// Timestamp of encryption
    pub timestamp: u64,
    /// Hardware accelerated flag
    pub hardware_accelerated: bool,
}

/// Key cache entry
#[derive(Debug, Clone)]
struct KeyCacheEntry {
    key: Vec<u8>,
    created_at: Instant,
    last_used: Instant,
    usage_count: u64,
}

impl HardwareEncryptionEngine {
    /// Create new hardware-accelerated encryption engine
    pub fn new(config: EncryptionConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let hardware_caps = Self::detect_hardware_capabilities();
        
        info!("Hardware capabilities detected: {:?}", hardware_caps);
        
        // Generate master key
        let master_key = Self::generate_master_key(&config)?;
        
        // Initialize AES-GCM cipher
        let aes_key = Key::from_slice(&master_key);
        let cipher = Aes256Gcm::new(aes_key);
        
        // Initialize Ring cipher for hardware acceleration
        let unbound_key = UnboundKey::new(&AES_256_GCM, &master_key)
            .map_err(|e| format!("Failed to create unbound key: {}", e))?;
        let ring_cipher = LessSafeKey::new(unbound_key);

        Ok(Self {
            cipher: Arc::new(RwLock::new(cipher)),
            ring_cipher: Arc::new(RwLock::new(ring_cipher)),
            metrics: Arc::new(RwLock::new(EncryptionMetrics::default())),
            config,
            hardware_caps,
        })
    }

    /// Encrypt data with hardware acceleration
    pub async fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        
        // Generate nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        // Choose encryption method based on hardware capabilities
        let (ciphertext, tag, hardware_accelerated) = if self.config.enable_hardware_acceleration && self.hardware_caps.aes_ni {
            self.encrypt_hardware_accelerated(plaintext, &nonce).await?
        } else {
            self.encrypt_software(plaintext, &nonce).await?
        };

        let encryption_time = start_time.elapsed();
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_encryptions += 1;
            metrics.avg_encryption_time_us = (metrics.avg_encryption_time_us * (metrics.total_encryptions - 1) as f64 + encryption_time.as_micros() as f64) / metrics.total_encryptions as f64;
            
            if hardware_accelerated {
                metrics.hardware_acceleration_rate = (metrics.hardware_acceleration_rate * (metrics.total_encryptions - 1) as f64 + 1.0) / metrics.total_encryptions as f64;
            }
        }

        Ok(EncryptedData {
            ciphertext,
            tag,
            nonce: nonce.to_vec(),
            algorithm: "AES-256-GCM".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            hardware_accelerated,
        })
    }

    /// Decrypt data with hardware acceleration
    pub async fn decrypt(&self, encrypted_data: &EncryptedData) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        
        let nonce = Nonce::from_slice(&encrypted_data.nonce);
        
        // Choose decryption method based on how it was encrypted
        let plaintext = if encrypted_data.hardware_accelerated && self.config.enable_hardware_acceleration && self.hardware_caps.aes_ni {
            self.decrypt_hardware_accelerated(&encrypted_data.ciphertext, &encrypted_data.tag, nonce).await?
        } else {
            self.decrypt_software(&encrypted_data.ciphertext, &encrypted_data.tag, nonce).await?
        };

        let decryption_time = start_time.elapsed();
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_decryptions += 1;
            metrics.avg_decryption_time_us = (metrics.avg_decryption_time_us * (metrics.total_decryptions - 1) as f64 + decryption_time.as_micros() as f64) / metrics.total_decryptions as f64;
        }

        Ok(plaintext)
    }

    /// Batch encrypt multiple data items
    pub async fn encrypt_batch(&self, plaintexts: &[Vec<u8>]) -> Result<Vec<EncryptedData>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.config.enable_parallel {
            // Sequential processing
            let mut results = Vec::with_capacity(plaintexts.len());
            for plaintext in plaintexts {
                results.push(self.encrypt(plaintext).await?);
            }
            return Ok(results);
        }

        // Parallel processing
        let mut handles = Vec::new();
        for plaintext in plaintexts {
            let engine = self.clone();
            let plaintext = plaintext.clone();
            let handle = tokio::spawn(async move {
                engine.encrypt(&plaintext).await
            });
            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result?),
                Err(e) => return Err(Box::new(e)),
            }
        }

        Ok(results)
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> EncryptionMetrics {
        self.metrics.read().await.clone()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = EncryptionMetrics::default();
    }

    /// Detect hardware capabilities
    fn detect_hardware_capabilities() -> HardwareCapabilities {
        HardwareCapabilities {
            aes_ni: is_x86_feature_detected!("aes"),
            avx2: is_x86_feature_detected!("avx2"),
            avx512: is_x86_feature_detected!("avx512f"),
            bmi2: is_x86_feature_detected!("bmi2"),
            fma: is_x86_feature_detected!("fma"),
        }
    }

    /// Generate master key
    fn generate_master_key(config: &EncryptionConfig) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let salt = rand::generate(rand::SystemRandom::new())
            .map_err(|e| format!("Failed to generate salt: {}", e))?;
        
        let mut key = vec![0u8; 32]; // 256-bit key
        
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(config.key_derivation_iterations)
                .ok_or("Invalid iteration count")?,
            &salt.expose,
            b"solana-account-cleaner-master-key",
            &mut key,
        );

        Ok(key)
    }

    /// Hardware-accelerated encryption using Ring
    async fn encrypt_hardware_accelerated(&self, plaintext: &[u8], nonce: &Nonce) -> Result<(Vec<u8>, Vec<u8>, bool), Box<dyn std::error::Error + Send + Sync>> {
        let cipher = self.ring_cipher.read().await;
        
        // Prepare additional data
        let aad = Aad::from(b"solana-account-cleaner");
        
        // Prepare buffer for ciphertext + tag
        let mut buffer = plaintext.to_vec();
        buffer.resize(buffer.len() + 16, 0); // Space for tag
        
        // Encrypt in-place
        let nonce_bytes = ring::aead::Nonce::assume_unique_for_key(nonce.as_slice().try_into()
            .map_err(|_| "Invalid nonce length")?);
        
        cipher.seal_in_place_append_tag(nonce_bytes, aad, &mut buffer)
            .map_err(|e| format!("Hardware encryption failed: {}", e))?;
        
        // Split ciphertext and tag
        let tag_start = buffer.len() - 16;
        let ciphertext = buffer[..tag_start].to_vec();
        let tag = buffer[tag_start..].to_vec();
        
        Ok((ciphertext, tag, true))
    }

    /// Software-based encryption
    async fn encrypt_software(&self, plaintext: &[u8], nonce: &Nonce) -> Result<(Vec<u8>, Vec<u8>, bool), Box<dyn std::error::Error + Send + Sync>> {
        let cipher = self.cipher.read().await;
        
        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| format!("Software encryption failed: {}", e))?;
        
        // Split ciphertext and tag (AES-GCM appends tag)
        let tag_start = ciphertext.len() - 16;
        let ciphertext_bytes = ciphertext[..tag_start].to_vec();
        let tag = ciphertext[tag_start..].to_vec();
        
        Ok((ciphertext_bytes, tag, false))
    }

    /// Hardware-accelerated decryption using Ring
    async fn decrypt_hardware_accelerated(&self, ciphertext: &[u8], tag: &[u8], nonce: &Nonce) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let cipher = self.ring_cipher.read().await;
        
        // Prepare buffer with ciphertext + tag
        let mut buffer = Vec::with_capacity(ciphertext.len() + tag.len());
        buffer.extend_from_slice(ciphertext);
        buffer.extend_from_slice(tag);
        
        // Prepare additional data
        let aad = Aad::from(b"solana-account-cleaner");
        
        // Decrypt in-place
        let nonce_bytes = ring::aead::Nonce::assume_unique_for_key(nonce.as_slice().try_into()
            .map_err(|_| "Invalid nonce length")?);
        
        let plaintext = cipher.open_in_place(nonce_bytes, aad, &mut buffer)
            .map_err(|e| format!("Hardware decryption failed: {}", e))?;
        
        Ok(plaintext.to_vec())
    }

    /// Software-based decryption
    async fn decrypt_software(&self, ciphertext: &[u8], tag: &[u8], nonce: &Nonce) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let cipher = self.cipher.read().await;
        
        // Combine ciphertext and tag
        let mut encrypted_data = Vec::with_capacity(ciphertext.len() + tag.len());
        encrypted_data.extend_from_slice(ciphertext);
        encrypted_data.extend_from_slice(tag);
        
        let plaintext = cipher.decrypt(nonce, &encrypted_data)
            .map_err(|e| format!("Software decryption failed: {}", e))?;
        
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encryption_decryption() {
        let config = EncryptionConfig::default();
        let engine = HardwareEncryptionEngine::new(config).unwrap();
        
        let plaintext = b"Hello, Solana Account Cleaner!";
        
        let encrypted = engine.encrypt(plaintext).await.unwrap();
        let decrypted = engine.decrypt(&encrypted).await.unwrap();
        
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[tokio::test]
    async fn test_batch_encryption() {
        let config = EncryptionConfig::default();
        let engine = HardwareEncryptionEngine::new(config).unwrap();
        
        let plaintexts = vec![
            b"Data 1".to_vec(),
            b"Data 2".to_vec(),
            b"Data 3".to_vec(),
        ];
        
        let encrypted = engine.encrypt_batch(&plaintexts).await.unwrap();
        assert_eq!(encrypted.len(), plaintexts.len());
        
        // Verify decryption
        for (i, enc_data) in encrypted.iter().enumerate() {
            let decrypted = engine.decrypt(enc_data).await.unwrap();
            assert_eq!(decrypted, plaintexts[i]);
        }
    }

    #[test]
    fn test_hardware_capabilities_detection() {
        let caps = HardwareEncryptionEngine::detect_hardware_capabilities();
        // Should not panic and should return valid capabilities
        println!("Hardware capabilities: {:?}", caps);
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let config = EncryptionConfig::default();
        let engine = HardwareEncryptionEngine::new(config).unwrap();
        
        let initial_metrics = engine.get_metrics().await;
        assert_eq!(initial_metrics.total_encryptions, 0);
        assert_eq!(initial_metrics.total_decryptions, 0);
        
        // Perform encryption
        let plaintext = b"Test data";
        let _encrypted = engine.encrypt(plaintext).await.unwrap();
        
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.total_encryptions, 1);
        assert!(metrics.avg_encryption_time_us > 0.0);
    }
}
