use crate::core::{Result, SolanaRecoverError};
use crate::wallet::{WalletProvider, WalletCredentials, WalletConnection, ConnectionData};
use async_trait::async_trait;
use solana_sdk::{
    signature::{Keypair, Signer, SeedDerivable},
    transaction::Transaction,
};
use zeroize::Zeroize;
use std::sync::Arc;

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
    // In a real implementation, this would handle secure key storage
}

impl PrivateKeyProvider {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl WalletProvider for PrivateKeyProvider {
    async fn connect(&self, credentials: &WalletCredentials) -> Result<WalletConnection> {
        if let crate::wallet::WalletCredentialData::PrivateKey { private_key } = &credentials.credentials {
            // Validate the private key format
            let _keypair = self.parse_private_key(private_key)?;
            
            let connection = WalletConnection {
                id: uuid::Uuid::new_v4().to_string(),
                wallet_type: crate::wallet::WalletType::PrivateKey,
                connection_data: ConnectionData::PrivateKey {
                    private_key: private_key.clone(), // TODO: Convert to SecretKey
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

    async fn get_public_key(&self, connection: &WalletConnection) -> Result<String> {
        if let ConnectionData::PrivateKey { private_key } = &connection.connection_data {
            let keypair = self.parse_private_key(private_key)?;
            Ok(keypair.pubkey().to_string())
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid PrivateKey connection".to_string()
            ))
        }
    }

    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8]) -> Result<Vec<u8>> {
        if let ConnectionData::PrivateKey { private_key } = &connection.connection_data {
            let keypair = self.parse_private_key(private_key)?;
            
            // Deserialize the transaction
            let mut tx: Transaction = bincode::deserialize(transaction)
                .map_err(|e| SolanaRecoverError::SerializationError(format!("Failed to deserialize transaction: {}", e)))?;
            
            // Sign the transaction
            tx.sign(&[&keypair], tx.message.recent_blockhash);
            
            // Return the signature of the first signer (which should be our keypair)
            if let Some(signature) = tx.signatures.get(0) {
                Ok(signature.as_ref().to_vec())
            } else {
                Err(SolanaRecoverError::InternalError("No signature found in signed transaction".to_string()))
            }
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid PrivateKey connection".to_string()
            ))
        }
    }

    async fn disconnect(&self, _connection: &WalletConnection) -> Result<()> {
        // For PrivateKey provider, disconnection is mainly about cleaning up the connection data
        // The private key itself should be zeroized if stored in memory
        Ok(())
    }
}

impl PrivateKeyProvider {
    fn parse_private_key(&self, private_key: &str) -> Result<Keypair> {
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
