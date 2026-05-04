use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct PhantomProvider {
    connections: Arc<RwLock<HashMap<String, PhantomSession>>>,
}

#[derive(Debug, Clone)]
pub struct PhantomSession {
    pub session_id: String,
    pub public_key: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhantomMessage {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhantomResponse {
    pub id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<PhantomError>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhantomError {
    pub code: i32,
    pub message: String,
}

impl PhantomProvider {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

}

#[async_trait::async_trait]
impl crate::wallet::manager::WalletProvider for PhantomProvider {
    async fn connect(&self, credentials: &crate::wallet::manager::WalletCredentials) -> crate::core::Result<crate::wallet::manager::WalletConnection> {
        if let crate::wallet::manager::WalletCredentialData::Phantom { encrypted_private_key: _ } = &credentials.credentials {
            let session_id = Uuid::new_v4().to_string();
            let public_key = "phantom_public_key_placeholder".to_string(); // Would be derived from actual connection
            
            let session = PhantomSession {
                session_id: session_id.clone(),
                public_key: public_key.clone(),
                connected_at: chrono::Utc::now(),
                last_activity: chrono::Utc::now(),
            };
            
            self.connections.write().await.insert(session_id.clone(), session);
            
            Ok(crate::wallet::manager::WalletConnection {
                id: session_id.clone(),
                wallet_type: crate::wallet::manager::WalletType::Phantom,
                connection_data: crate::wallet::manager::ConnectionData::Phantom { session_id: session_id.clone() },
                created_at: chrono::Utc::now(),
            })
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid credentials for Phantom provider".to_string()
            ))
        }
    }

    async fn get_public_key(&self, connection: &crate::wallet::manager::WalletConnection) -> crate::core::Result<String> {
        if let crate::wallet::manager::ConnectionData::Phantom { session_id } = &connection.connection_data {
            let sessions = self.connections.read().await;
            if let Some(session) = sessions.get(session_id) {
                Ok(session.public_key.clone())
            } else {
                Err(crate::core::SolanaRecoverError::AuthenticationError(
                    "Session not found".to_string()
                ))
            }
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid Phantom connection".to_string()
            ))
        }
    }

    async fn sign_transaction(&self, connection: &crate::wallet::manager::WalletConnection, transaction: &[u8], _rpc_url: Option<&str>) -> crate::core::Result<Vec<u8>> {
        if let crate::wallet::manager::ConnectionData::Phantom { session_id } = &connection.connection_data {
            // Update session activity
            {
                let mut sessions = self.connections.write().await;
                if let Some(session) = sessions.get_mut(session_id) {
                    session.last_activity = chrono::Utc::now();
                }
            }

            // For now, return a placeholder signature
            // In real implementation, this would interact with Phantom wallet
            let mut signature = vec![0u8; 64];
            signature[0] = 1; // Placeholder non-zero signature
            
            let mut signed_transaction = Vec::with_capacity(64 + transaction.len());
            signed_transaction.extend_from_slice(&signature);
            signed_transaction.extend_from_slice(transaction);

            Ok(signed_transaction)
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid Phantom connection".to_string()
            ))
        }
    }

    async fn disconnect(&self, connection: &crate::wallet::manager::WalletConnection) -> crate::core::Result<()> {
        if let crate::wallet::manager::ConnectionData::Phantom { session_id } = &connection.connection_data {
            self.connections.write().await.remove(session_id);
            Ok(())
        } else {
            Err(crate::core::SolanaRecoverError::AuthenticationError(
                "Invalid Phantom connection".to_string()
            ))
        }
    }
}

impl Default for PhantomProvider {
    fn default() -> Self {
        Self::new()
    }
}
