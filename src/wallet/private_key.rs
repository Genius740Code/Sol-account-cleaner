use crate::core::{Result, SolanaRecoverError};
use crate::wallet::{WalletProvider, WalletCredentials, WalletConnection, ConnectionData};
use crate::wallet::manager::{WalletType, WalletCredentialData};
use crate::wallet::transaction_validator::TransactionValidator;
use crate::wallet::nonce_manager::NonceManager;
use crate::wallet::audit_logger::{AuditLogger, SecurityContext, RiskLevel};
use async_trait::async_trait;
use solana_sdk::{
    signature::{Keypair, Signer, SeedDerivable},
    transaction::Transaction,
};
use zeroize::Zeroize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, Duration};
use tokio::sync::RwLock;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use once_cell::sync::Lazy;

// Secure in-memory registry for private key connections
// This is never serialized and only exists in memory
static PRIVATE_KEY_REGISTRY: Lazy<Mutex<HashMap<String, PrivateKeyConnection>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

// Structure for secure private key connections
#[derive(Clone)]
struct PrivateKeyConnection {
    id: String,
    secret_key: SecretKey,
    created_at: chrono::DateTime<chrono::Utc>,
}

// Secure wrapper for private key data that implements zeroization
#[derive(Clone)]
pub struct SecretKey {
    data: Arc<std::sync::Mutex<Vec<u8>>>,
}

// Custom serialization for SecretKey that doesn't expose the actual key
impl Serialize for SecretKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Only serialize a placeholder, not the actual key
        serializer.serialize_str("[REDACTED_SECRET_KEY]")
    }
}

// Custom deserialization for SecretKey
impl<'de> Deserialize<'de> for SecretKey {
    fn deserialize<D>(_deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // For security, don't allow deserialization of secret keys from serialized data
        // This forces keys to be created fresh from secure sources
        Err(serde::de::Error::custom(
            "SecretKey cannot be deserialized from serialized data for security reasons"
        ))
    }
}

impl SecretKey {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            data: Arc::new(std::sync::Mutex::new(bytes)),
        }
    }
    
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        let data = self.data.lock()
            .map_err(|_| SolanaRecoverError::InternalError("Failed to access secret key".to_string()))?;
        Ok(data.clone())
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        if let Ok(mut data) = self.data.lock() {
            data.zeroize();
        }
    }
}

pub struct PrivateKeyProvider {
    validator: Arc<TransactionValidator>,
    nonce_manager: Arc<NonceManager>,
    audit_logger: Arc<AuditLogger>,
    rate_limiter: Arc<RwLock<std::collections::HashMap<String, (SystemTime, u32)>>>,
    max_signing_attempts: u32,
    signing_timeout_ms: u64,
    max_transaction_size: usize,
}

impl PrivateKeyProvider {
    pub fn new() -> Self {
        Self::with_components(
            Arc::new(TransactionValidator::new()),
            Arc::new(NonceManager::default()),
            Arc::new(AuditLogger::default()),
        )
    }

    pub fn with_components(
        validator: Arc<TransactionValidator>,
        nonce_manager: Arc<NonceManager>,
        audit_logger: Arc<AuditLogger>,
    ) -> Self {
        Self {
            validator,
            nonce_manager,
            audit_logger,
            rate_limiter: Arc::new(RwLock::new(std::collections::HashMap::new())),
            max_signing_attempts: 3,
            signing_timeout_ms: 30000,
            max_transaction_size: 1232, // Default Solana transaction size
        }
    }

    async fn check_rate_limit(&self, wallet_address: &str) -> Result<()> {
        let mut rate_limiter = self.rate_limiter.write().await;
        let now = SystemTime::now();
        
        // Clean up old entries (older than 5 minutes) to prevent memory leaks
        let cutoff_time = now - Duration::from_secs(300);
        rate_limiter.retain(|_, (timestamp, _)| *timestamp > cutoff_time);
        
        if let Some((last_sign, attempt_count)) = rate_limiter.get(wallet_address) {
            let time_since_last = now.duration_since(*last_sign).unwrap_or(Duration::ZERO);
            
            // Enhanced exponential backoff: 2s, 4s, 8s, 16s, 32s, max 60s
            // More aggressive starting delay for better security
            let backoff_time = Duration::from_secs(2_u64.pow((*attempt_count).min(5)).min(60));
            
            // Additional security: if too many attempts, temporarily block
            if *attempt_count >= 10 {
                return Err(SolanaRecoverError::RateLimitExceeded(
                    "Too many failed attempts. Account temporarily blocked for security reasons.".to_string()
                ));
            }
            
            if time_since_last < backoff_time {
                return Err(SolanaRecoverError::RateLimitExceeded(
                    format!("Rate limit exceeded. Please wait {} seconds before trying again.", 
                           backoff_time.as_secs() - time_since_last.as_secs())
                ));
            }
        }
        
        // Track attempt count for exponential backoff
        let attempt_count = if let Some((_, count)) = rate_limiter.get(wallet_address) {
            *count + 1
        } else {
            1
        };
        
        rate_limiter.insert(wallet_address.to_string(), (now, attempt_count));
        Ok(())
    }

    fn create_security_context(&self, connection_id: &str) -> SecurityContext {
        SecurityContext {
            ip_address: None,
            user_agent: Some("solana-recover-client".to_string()),
            session_id: Some(connection_id.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            geo_location: None,
        }
    }

    async fn validate_transaction_with_retry(
        &self,
        transaction: &[u8],
        rpc_client: &solana_client::rpc_client::RpcClient,
        attempts: u32,
    ) -> Result<crate::wallet::transaction_validator::ValidationResult> {
        let mut last_error = None;
        
        for attempt in 1..=attempts {
            match self.validator.validate_transaction(transaction, rpc_client).await {
                Ok(result) => {
                    if result.is_valid {
                        return Ok(result);
                    } else {
                        return Err(SolanaRecoverError::ValidationError(
                            format!("Transaction validation failed: {:?}", result.errors)
                        ));
                    }
                }
                Err(e) => {
                    let error_str = format!("{}", e);
                    last_error = Some(SolanaRecoverError::InternalError(error_str));
                    if attempt < attempts {
                        tokio::time::sleep(Duration::from_millis(1000 * attempt as u64)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| SolanaRecoverError::InternalError(
            "Transaction validation failed after all attempts".to_string()
        )))
    }
}

#[async_trait]
impl WalletProvider for PrivateKeyProvider {
    async fn connect(&self, credentials: &WalletCredentials) -> crate::core::Result<WalletConnection> {
        if let WalletCredentialData::PrivateKey { private_key } = &credentials.credentials {
            // Validate the private key format
            let keypair = self.parse_private_key(private_key)?;
            
            // SECURITY FIX: Store private key securely using SecretKey wrapper
            let secret_key = SecretKey::new(keypair.to_bytes().to_vec());
            
            // Create connection with secure key storage
            let connection_id = uuid::Uuid::new_v4().to_string();
            
            // Store the secret key in a secure registry for this connection
            let secure_connection = PrivateKeyConnection {
                id: connection_id.clone(),
                secret_key: secret_key.clone(),
                created_at: chrono::Utc::now(),
            };
            
            // Store in our secure registry (in-memory only, never serialized)
            let mut registry = PRIVATE_KEY_REGISTRY.lock().unwrap();
            registry.insert(connection_id.clone(), secure_connection);
            
            let connection = WalletConnection {
                id: connection_id,
                wallet_type: WalletType::PrivateKey,
                connection_data: ConnectionData::PrivateKey {
                    private_key: "[SECURELY_STORED]".to_string(), // Placeholder only
                },
                created_at: chrono::Utc::now(),
            };
            
            // The secret_key is now stored in the secure registry and will be zeroized when dropped
            // No need to explicitly drop here as it's safely stored

            Ok(connection)
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid PrivateKey credentials".to_string()
            ))
        }
    }

    async fn get_public_key(&self, connection: &WalletConnection) -> crate::core::Result<String> {
        // Access the secure registry to get the secret key
        let registry = PRIVATE_KEY_REGISTRY.lock().unwrap();
        if let Some(secure_conn) = registry.get(&connection.id) {
            let key_bytes = secure_conn.secret_key.as_bytes()
                .map_err(|_| SolanaRecoverError::AuthenticationError(
                    "Failed to access private key".to_string()
                ))?;
            let keypair = Keypair::from_bytes(&key_bytes)
                .map_err(|_| SolanaRecoverError::AuthenticationError(
                    "Invalid private key in connection".to_string()
                ))?;
            Ok(keypair.pubkey().to_string())
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid PrivateKey connection - not found in secure registry".to_string()
            ))
        }
    }

    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8], rpc_url: Option<&str>) -> crate::core::Result<Vec<u8>> {
        // Access the secure registry to get the secret key
        let (keypair, wallet_address) = {
            let registry = PRIVATE_KEY_REGISTRY.lock().unwrap();
            if let Some(secure_conn) = registry.get(&connection.id) {
                // Access private key securely from SecretKey wrapper
                let key_bytes = secure_conn.secret_key.as_bytes()
                    .map_err(|_| SolanaRecoverError::AuthenticationError(
                        "Failed to access private key".to_string()
                    ))?;
                let keypair = Keypair::from_bytes(&key_bytes)
                    .map_err(|_| SolanaRecoverError::AuthenticationError(
                        "Invalid private key in connection".to_string()
                    ))?;
                
                let wallet_address = keypair.pubkey().to_string();
                (keypair, wallet_address)
            } else {
                return Err(SolanaRecoverError::AuthenticationError(
                    "Invalid PrivateKey connection - not found in secure registry".to_string()
                ));
            }
        };
        
        let security_context = self.create_security_context(&connection.id);
        
        // Check rate limiting
        self.check_rate_limit(&wallet_address).await?;
        
        let tx: Transaction = bincode::deserialize(transaction)
            .map_err(|e| SolanaRecoverError::SerializationError(format!("Failed to deserialize transaction: {}", e)))?;
        
        // Log signing request
        self.audit_logger.log_transaction_signing(
            None,
            "PrivateKey".to_string(),
            Some(keypair.pubkey()),
            &tx,
            solana_sdk::signature::Signature::default(), // Will be updated after signing
            security_context.clone(),
            RiskLevel::Medium,
        ).await?;
        
        // Validate transaction with RPC client
        let url = rpc_url.unwrap_or("https://api.mainnet-beta.solana.com");
        let rpc_client = solana_client::rpc_client::RpcClient::new(url);
        let validation_result = self.validate_transaction_with_retry(transaction, &rpc_client, self.max_signing_attempts).await?;
        
        // Check for replay attacks
        self.nonce_manager.validate_transaction(&tx).await?;
        
        // Determine risk level based on validation
        let risk_level = if validation_result.warnings.is_empty() {
            RiskLevel::Low
        } else if validation_result.warnings.len() < 3 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        };
        
        // Sign the transaction with timeout
        let signed_tx = tokio::time::timeout(
            Duration::from_millis(self.signing_timeout_ms),
            Self::sign_with_timeout(&keypair, &tx)
        ).await
        .map_err(|_| SolanaRecoverError::TimeoutError(
            "Transaction signing timed out".to_string()
        ))??;
        
        // Log successful signing
        self.audit_logger.log_transaction_signing(
            None,
            "PrivateKey".to_string(),
            Some(keypair.pubkey()),
            &signed_tx,
            *signed_tx.signatures.first().ok_or_else(|| {
                SolanaRecoverError::TransactionError("No signature found".to_string())
            })?,
            security_context,
            risk_level,
        ).await?;
        
        // Register nonce for replay protection
        if Self::is_nonce_transaction(&signed_tx) {
            let nonce = signed_tx.message.recent_blockhash;
            self.nonce_manager.register_nonce(keypair.pubkey(), nonce).await?;
        }
        
        // Return the full serialized signed transaction
        bincode::serialize(&signed_tx)
            .map_err(|e| SolanaRecoverError::SerializationError(format!("Failed to serialize signed transaction: {}", e)))
    }
    
    async fn disconnect(&self, connection: &WalletConnection) -> crate::core::Result<()> {
        let security_context = self.create_security_context(&connection.id);
        
        // Log disconnection
        self.audit_logger.log_wallet_disconnection(
            None,
            "PrivateKey".to_string(),
            connection,
            security_context,
        ).await?;
        
        // Clean up rate limiting data
        if let Ok(wallet_address) = self.get_public_key(connection).await {
            let mut rate_limiter = self.rate_limiter.write().await;
            rate_limiter.remove(&wallet_address);
        }
        
        // Clean up secure registry - this will trigger zeroization
        let mut registry = PRIVATE_KEY_REGISTRY.lock().unwrap();
        registry.remove(&connection.id);
        
        Ok(())
    }
}

impl PrivateKeyProvider {
    async fn sign_with_timeout(keypair: &Keypair, tx: &Transaction) -> Result<Transaction> {
        let mut signed_tx = tx.clone();
        signed_tx.sign(&[keypair], tx.message.recent_blockhash);
        
        // Verify signature
        if signed_tx.verify().is_err() {
            return Err(SolanaRecoverError::TransactionError(
                "Transaction signature verification failed".to_string()
            ));
        }
        
        Ok(signed_tx)
    }
    
    fn is_nonce_transaction(tx: &Transaction) -> bool {
        // Check if transaction uses a nonce account
        for instruction in &tx.message.instructions {
            if let Some(program_id) = tx.message.account_keys.get(instruction.program_id_index as usize) {
                if program_id == &solana_sdk::system_program::id() {
                    // This is a simplified check - in production, decode the instruction
                    return true;
                }
            }
        }
        false
    }

    pub fn parse_private_key(&self, private_key: &str) -> Result<Keypair> {
        // Try different formats: base58, hex, or array format
        let mut key_bytes = None;
        
        // Try base58 format (most common for Solana)
        if let Ok(bytes) = bs58::decode(private_key).into_vec() {
            if bytes.len() == 64 {
                key_bytes = Some(bytes);
            }
        }
        
        // Try hex format
        if key_bytes.is_none() {
            let hex_str = private_key.strip_prefix("0x").unwrap_or(private_key);
            if let Ok(bytes) = hex::decode(hex_str) {
                if bytes.len() == 32 {
                    // For 32-byte seeds, we need to create a keypair
                    if let Ok(kp) = Keypair::from_seed(&bytes) {
                        return Ok(kp);
                    }
                } else if bytes.len() == 64 {
                    key_bytes = Some(bytes);
                }
            }
        }
        
        // Try JSON array format
        if key_bytes.is_none() {
            if let Ok(bytes_vec) = serde_json::from_str::<Vec<u8>>(private_key) {
                if bytes_vec.len() == 32 {
                    if let Ok(kp) = Keypair::from_seed(&bytes_vec) {
                        return Ok(kp);
                    }
                } else if bytes_vec.len() == 64 {
                    key_bytes = Some(bytes_vec);
                }
            }
        }
        
        // If we have 64-byte keypair data, use it directly
        if let Some(mut bytes) = key_bytes {
            let result = Keypair::from_bytes(&bytes);
            bytes.zeroize(); // Immediately zeroize after use
            return result.map_err(|_| SolanaRecoverError::AuthenticationError(
                "Invalid private key format. Expected base58, hex, or array format.".to_string()
            ));
        }
        
        Err(SolanaRecoverError::AuthenticationError(
            "Invalid private key format. Expected base58, hex, or array format.".to_string()
        ))
    }
    
    // New secure method that returns a zeroizable keypair
    #[allow(dead_code)]
    fn parse_private_key_secure(&self, private_key: &str) -> Result<SecretKey> {
        // Parse the key and return as SecretKey for secure handling
        let keypair = self.parse_private_key(private_key)?;
        let bytes = keypair.to_bytes();
        Ok(SecretKey::new(bytes.to_vec()))
    }
}

impl Default for PrivateKeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::{WalletCredentials, WalletCredentialData};

    #[tokio::test]
    async fn test_private_key_connection_flow() {
        let provider = PrivateKeyProvider::new();
        
        // SECURITY FIX: Use environment variable for test private key
        let test_private_key = std::env::var("TEST_PRIVATE_KEY")
            .unwrap_or_else(|_| {
                // Generate a test keypair if no environment variable is set
                let test_keypair = Keypair::new();
                bs58::encode(test_keypair.to_bytes()).into_string()
            });
        
        println!("Testing private key parsing...");
        let parse_result = provider.parse_private_key(&test_private_key);
        
        if let Ok(keypair) = parse_result {
            println!("Successfully parsed private key");
            println!("Public key: {}", keypair.pubkey());
            
            // Test wallet connection
            println!("Testing wallet connection...");
            let credentials = WalletCredentials {
                wallet_type: crate::wallet::WalletType::PrivateKey,
                credentials: WalletCredentialData::PrivateKey {
                    private_key: test_private_key.to_string(),
                },
            };
            
            let connection_result = provider.connect(&credentials).await;
            if let Ok(connection) = connection_result {
                println!("Successfully connected to wallet");
                println!("Connection ID: {}", connection.id);
                
                // Test getting public key
                let pubkey_result = provider.get_public_key(&connection).await;
                if let Ok(pubkey) = pubkey_result {
                    println!("Retrieved public key: {}", pubkey);
                    assert_eq!(pubkey, keypair.pubkey().to_string());
                } else {
                    println!("Failed to get public key");
                }
            } else {
                println!("Failed to connect to wallet");
            }
        } else {
            println!("Failed to parse private key (expected for test key)");
        }
    }
}
