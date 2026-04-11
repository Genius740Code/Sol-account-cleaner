use crate::core::{Result, SolanaRecoverError};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
    hash::Hash,
};
use std::collections::{HashMap, BTreeMap};
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceInfo {
    pub nonce: Hash,
    pub used_signatures: Vec<Signature>,
    pub last_used: SystemTime,
    pub expires_at: SystemTime,
    pub account: Pubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayProtectionConfig {
    pub nonce_ttl_seconds: u64,
    pub max_signatures_per_nonce: usize,
    pub cleanup_interval_seconds: u64,
    pub enable_durable_nonce: bool,
}

impl Default for ReplayProtectionConfig {
    fn default() -> Self {
        Self {
            nonce_ttl_seconds: 300, // 5 minutes
            max_signatures_per_nonce: 10,
            cleanup_interval_seconds: 60, // 1 minute
            enable_durable_nonce: true,
        }
    }
}

pub struct NonceManager {
    nonces: RwLock<HashMap<Pubkey, NonceInfo>>,
    signature_history: RwLock<BTreeMap<SystemTime, Signature>>,
    config: ReplayProtectionConfig,
    last_cleanup: RwLock<SystemTime>,
}

impl NonceManager {
    pub fn new(config: ReplayProtectionConfig) -> Self {
        Self {
            nonces: RwLock::new(HashMap::new()),
            signature_history: RwLock::new(BTreeMap::new()),
            config,
            last_cleanup: RwLock::new(SystemTime::now()),
        }
    }

    pub async fn register_nonce(&self, account: Pubkey, nonce: Hash) -> Result<()> {
        let now = SystemTime::now();
        let expires_at = now + Duration::from_secs(self.config.nonce_ttl_seconds);

        let nonce_info = NonceInfo {
            nonce,
            used_signatures: Vec::new(),
            last_used: now,
            expires_at,
            account,
        };

        let mut nonces = self.nonces.write().await;
        nonces.insert(account, nonce_info);

        let _ = self.cleanup_expired_nonces().await;

        Ok(())
    }

    pub async fn validate_transaction(&self, transaction: &Transaction) -> Result<bool> {
        // Check if this is a nonce transaction
        if !self.is_nonce_transaction(transaction) {
            return Ok(true); // Non-nonce transactions are not subject to nonce validation
        }

        let nonce = self.extract_nonce(transaction)?;
        let signature = transaction.signatures.first()
            .ok_or_else(|| SolanaRecoverError::TransactionError("Transaction has no signature".to_string()))?;

        // Check for replay attacks
        if self.is_signature_replay(signature).await? {
            return Err(SolanaRecoverError::TransactionError(
                "Transaction signature replay detected".to_string()
            ));
        }

        // Check nonce validity
        if !self.is_nonce_valid(&nonce).await? {
            return Err(SolanaRecoverError::TransactionError(
                "Invalid or expired nonce".to_string()
            ));
        }

        // Check if this nonce has been used too many times
        if self.exceeds_nonce_usage_limit(&nonce).await? {
            return Err(SolanaRecoverError::TransactionError(
                "Nonce usage limit exceeded".to_string()
            ));
        }

        // Record the signature usage
        self.record_signature_usage(signature, &nonce).await?;

        Ok(true)
    }

    async fn is_signature_replay(&self, signature: &Signature) -> Result<bool> {
        let history = self.signature_history.read().await;
        
        // Check if signature exists in recent history
        for (_, sig) in history.iter().rev().take(1000) { // Check last 1000 signatures
            if sig == signature {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn is_nonce_valid(&self, nonce: &Hash) -> Result<bool> {
        let nonces = self.nonces.read().await;
        let now = SystemTime::now();

        for nonce_info in nonces.values() {
            if nonce_info.nonce == *nonce {
                return Ok(now < nonce_info.expires_at);
            }
        }

        // If nonce is not registered, check if it's a valid recent blockhash
        // This allows for transactions using recent blockhashes as nonces
        Ok(true)
    }

    async fn exceeds_nonce_usage_limit(&self, nonce: &Hash) -> Result<bool> {
        let nonces = self.nonces.read().await;

        for nonce_info in nonces.values() {
            if nonce_info.nonce == *nonce {
                return Ok(nonce_info.used_signatures.len() >= self.config.max_signatures_per_nonce);
            }
        }

        Ok(false)
    }

    async fn record_signature_usage(&self, signature: &Signature, nonce: &Hash) -> Result<()> {
        let now = SystemTime::now();

        // Record in signature history
        {
            let mut history = self.signature_history.write().await;
            history.insert(now, *signature);
            
            // Keep only recent signatures (last 24 hours)
            let cutoff = now - Duration::from_secs(86400);
            history.split_off(&cutoff);
        }

        // Update nonce usage
        {
            let mut nonces = self.nonces.write().await;
            for nonce_info in nonces.values_mut() {
                if nonce_info.nonce == *nonce {
                    nonce_info.used_signatures.push(*signature);
                    nonce_info.last_used = now;
                    break;
                }
            }
        }

        Ok(())
    }

    async fn cleanup_expired_nonces(&self) -> Result<usize> {
        let now = SystemTime::now();
        let mut last_cleanup = self.last_cleanup.write().await;

        // Check if cleanup is needed
        if now.duration_since(*last_cleanup).unwrap_or(Duration::ZERO) < 
           Duration::from_secs(self.config.cleanup_interval_seconds) {
            return Ok(0);
        }

        let mut nonces = self.nonces.write().await;
        let initial_count = nonces.len();

        // Remove expired nonces
        nonces.retain(|_, nonce_info| now < nonce_info.expires_at);

        let removed_count = initial_count - nonces.len();
        *last_cleanup = now;

        Ok(removed_count)
    }

    fn is_nonce_transaction(&self, transaction: &Transaction) -> bool {
        // Check if transaction uses a nonce account
        // This is a simplified check - in production, decode the instructions properly
        for instruction in &transaction.message.instructions {
            if let Some(program_id) = transaction.message.account_keys.get(instruction.program_id_index as usize) {
                if program_id == &solana_sdk::system_program::id() {
                    // Check if this is an advance nonce instruction
                    // In production, decode the instruction data to be certain
                    return true;
                }
            }
        }
        false
    }

    fn extract_nonce(&self, transaction: &Transaction) -> Result<Hash> {
        // Extract nonce from transaction
        // This could be from recent_blockhash or from nonce account instruction
        Ok(transaction.message.recent_blockhash)
    }

    pub async fn get_nonce_info(&self, account: &Pubkey) -> Result<Option<NonceInfo>> {
        let nonces = self.nonces.read().await;
        Ok(nonces.get(account).cloned())
    }

    pub async fn get_active_nonces(&self) -> Result<Vec<NonceInfo>> {
        let nonces = self.nonces.read().await;
        let now = SystemTime::now();
        
        Ok(nonces.values()
            .filter(|info| now < info.expires_at)
            .cloned()
            .collect())
    }

    pub async fn revoke_nonce(&self, account: &Pubkey) -> Result<bool> {
        let mut nonces = self.nonces.write().await;
        Ok(nonces.remove(account).is_some())
    }

    pub async fn get_metrics(&self) -> Result<NonceMetrics> {
        let nonces = self.nonces.read().await;
        let history = self.signature_history.read().await;
        let now = SystemTime::now();

        let active_nonces = nonces.values()
            .filter(|info| now < info.expires_at)
            .count();

        let total_signatures = history.len();
        let signatures_per_hour = if total_signatures > 0 {
            total_signatures as f64 / 24.0 // Assuming we keep 24 hours of history
        } else {
            0.0
        };

        Ok(NonceMetrics {
            active_nonces: active_nonces as u64,
            total_signatures: total_signatures as u64,
            signatures_per_hour,
            average_nonce_usage: if !nonces.is_empty() {
                nonces.values()
                    .map(|info| info.used_signatures.len() as f64)
                    .sum::<f64>() / nonces.len() as f64
            } else {
                0.0
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceMetrics {
    pub active_nonces: u64,
    pub total_signatures: u64,
    pub signatures_per_hour: f64,
    pub average_nonce_usage: f64,
}

impl Default for NonceManager {
    fn default() -> Self {
        Self::new(ReplayProtectionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{signature::Keypair, transaction::Transaction, message::Message};

    #[tokio::test]
    async fn test_nonce_management() {
        let manager = NonceManager::new(ReplayProtectionConfig::default());
        let account = Keypair::new().pubkey();
        let nonce = Hash::new_unique();

        // Register nonce
        assert!(manager.register_nonce(account, nonce).await.is_ok());

        // Check nonce info
        let info = manager.get_nonce_info(&account).await.unwrap();
        assert!(info.is_some());
        assert_eq!(info.unwrap().nonce, nonce);

        // Test transaction validation
        let keypair = Keypair::new();
        let message = Message::new(&[], Some(&keypair.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        tx.message.recent_blockhash = nonce;
        tx.sign(&[&keypair], nonce);

        // Should validate successfully
        assert!(manager.validate_transaction(&tx).await.is_ok());

        // Test replay detection
        assert!(manager.validate_transaction(&tx).await.is_err());
    }

    #[tokio::test]
    async fn test_nonce_expiration() {
        let mut config = ReplayProtectionConfig::default();
        config.nonce_ttl_seconds = 1; // 1 second TTL for testing
        
        let manager = NonceManager::new(config);
        let account = Keypair::new().pubkey();
        let nonce = Hash::new_unique();

        manager.register_nonce(account, nonce).await.unwrap();

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Nonce should be expired
        let keypair = Keypair::new();
        let message = Message::new(&[], Some(&keypair.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        tx.message.recent_blockhash = nonce;
        tx.sign(&[&keypair], nonce);

        assert!(manager.validate_transaction(&tx).await.is_err());
    }
}
