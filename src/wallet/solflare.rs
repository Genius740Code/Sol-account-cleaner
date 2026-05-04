use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct SolflareProvider {
    connections: Arc<RwLock<HashMap<String, SolflareSession>>>,
}

#[derive(Debug, Clone)]
pub struct SolflareSession {
    pub session_id: String,
    pub public_key: String,
    pub wallet_type: SolflareWalletType,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SolflareWalletType {
    Extension,
    Mobile,
    Web,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolflareConfig {
    pub timeout_ms: u64,
    pub retry_attempts: u32,
    pub enable_mobile_support: bool,
    pub enable_web_support: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolflareMessage {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolflareResponse {
    pub id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<SolflareError>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolflareError {
    pub code: i32,
    pub message: String,
}

impl Default for SolflareConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 15000,
            retry_attempts: 3,
            enable_mobile_support: true,
            enable_web_support: true,
        }
    }
}

impl SolflareProvider {
    pub fn new() -> Self {
        Self::with_config(SolflareConfig::default())
    }

    pub fn with_config(_config: SolflareConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl crate::wallet::manager::WalletProvider for SolflareProvider {
    async fn connect(&self, credentials: &crate::wallet::manager::WalletCredentials) -> crate::core::Result<crate::wallet::manager::WalletConnection> {
        if let crate::wallet::manager::WalletCredentialData::Solflare { public_key } = &credentials.credentials {
            let session_id = Uuid::new_v4().to_string();
            
            let session = SolflareSession {
                session_id: session_id.clone(),
                public_key: public_key.clone(),
                wallet_type: SolflareWalletType::Extension,
                connected_at: chrono::Utc::now(),
                last_activity: chrono::Utc::now(),
            };
            
            self.connections.write().await.insert(session_id.clone(), session);
            
            Ok(crate::wallet::manager::WalletConnection {
                id: session_id.clone(),
                wallet_type: crate::wallet::manager::WalletType::Solflare,
                connection_data: crate::wallet::manager::ConnectionData::Solflare { session_token: session_id.clone() },
                created_at: chrono::Utc::now(),
            })
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid credentials for Solflare provider".to_string()
            ))
        }
    }

    async fn get_public_key(&self, connection: &crate::wallet::manager::WalletConnection) -> crate::core::Result<String> {
        if let crate::wallet::manager::ConnectionData::Solflare { session_token } = &connection.connection_data {
            let sessions = self.connections.read().await;
            if let Some(session) = sessions.get(session_token) {
                Ok(session.public_key.clone())
            } else {
                Err(crate::core::SolanaRecoverError::AuthenticationError(
                    "Session not found".to_string()
                ))
            }
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid Solflare connection".to_string()
            ))
        }
    }

    async fn sign_transaction(&self, connection: &crate::wallet::manager::WalletConnection, transaction: &[u8], _rpc_url: Option<&str>) -> crate::core::Result<Vec<u8>> {
        if let crate::wallet::manager::ConnectionData::Solflare { session_token } = &connection.connection_data {
            // Update session activity
            {
                let mut sessions = self.connections.write().await;
                if let Some(session) = sessions.get_mut(session_token) {
                    session.last_activity = chrono::Utc::now();
                }
            }

            // For now, return a placeholder signature
            // In real implementation, this would interact with Solflare wallet
            let mut signature = vec![0u8; 64];
            signature[0] = 2; // Placeholder non-zero signature
            
            let mut signed_transaction = Vec::with_capacity(64 + transaction.len());
            signed_transaction.extend_from_slice(&signature);
            signed_transaction.extend_from_slice(transaction);

            Ok(signed_transaction)
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid Solflare connection".to_string()
            ))
        }
    }

    async fn disconnect(&self, connection: &crate::wallet::manager::WalletConnection) -> crate::core::Result<()> {
        if let crate::wallet::manager::ConnectionData::Solflare { session_token } = &connection.connection_data {
            self.connections.write().await.remove(session_token);
            Ok(())
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid Solflare connection".to_string()
            ))
        }
    }
}

impl Default for SolflareProvider {
    fn default() -> Self {
        Self::new()
    }
}
