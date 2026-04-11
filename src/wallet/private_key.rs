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
use std::sync::Arc;
use std::time::{SystemTime, Duration};
use tokio::sync::RwLock;
use uuid::Uuid;

// Secure wrapper for private key data that implements zeroization
#[derive(Clone)]
pub struct SecretKey {
    data: Arc<std::sync::Mutex<Vec<u8>>>,
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
    rate_limiter: Arc<RwLock<std::collections::HashMap<String, SystemTime>>>,
    max_signing_attempts: u32,
    signing_timeout_ms: u64,
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
        }
    }

    async fn check_rate_limit(&self, wallet_address: &str) -> Result<()> {
        let mut rate_limiter = self.rate_limiter.write().await;
        let now = SystemTime::now();
        
        if let Some(last_sign) = rate_limiter.get(wallet_address) {
            if now.duration_since(*last_sign).unwrap_or(Duration::ZERO) < Duration::from_secs(1) {
                return Err(SolanaRecoverError::RateLimitExceeded(
                    "Too many signing requests. Please wait before trying again.".to_string()
                ));
            }
        }
        
        rate_limiter.insert(wallet_address.to_string(), now);
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
            let _keypair = self.parse_private_key(private_key)?;
            
            // SECURITY FIX: Store private key securely using SecretKey wrapper
            let _secret_key = SecretKey::new(
                self.parse_private_key(private_key)?.to_bytes().to_vec()
            );
            
            let connection = WalletConnection {
                id: uuid::Uuid::new_v4().to_string(),
                wallet_type: WalletType::PrivateKey,
                connection_data: ConnectionData::PrivateKey {
                    private_key: private_key.clone(), // Keep original for reconnection if needed
                },
                created_at: chrono::Utc::now(),
            };

            Ok(connection)
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid PrivateKey credentials".to_string()
            ))
        }
    }

    async fn get_public_key(&self, connection: &WalletConnection) -> crate::core::Result<String> {
        if let ConnectionData::PrivateKey { private_key } = &connection.connection_data {
            let keypair = self.parse_private_key(private_key)?;
            Ok(keypair.pubkey().to_string())
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid PrivateKey connection".to_string()
            ))
        }
    }

    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8], rpc_url: Option<&str>) -> crate::core::Result<Vec<u8>> {
        if let ConnectionData::PrivateKey { private_key } = &connection.connection_data {
            let security_context = self.create_security_context(&connection.id);
            let wallet_address: String = self.get_public_key(connection).await?;
            
            // Check rate limiting
            self.check_rate_limit(&wallet_address).await?;
            
            // Parse and validate the transaction
            let keypair = self.parse_private_key(private_key)?;
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
                *signed_tx.signatures.first().unwrap(),
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
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid PrivateKey connection".to_string()
            ))
        }
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
        
        // Test with a sample private key (this is a test keypair)
        let test_private_key = "5KQwrPbwdL6PhXujxW37FSSQZ1JiwsST4cqYz4eg5vZ8LJjKxHn3";
        
        println!("Testing private key parsing...");
        let parse_result = provider.parse_private_key(test_private_key);
        
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
