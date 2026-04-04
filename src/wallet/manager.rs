use crate::core::{Result, SolanaRecoverError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletCredentials {
    pub wallet_type: WalletType,
    pub credentials: WalletCredentialData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WalletType {
    Turnkey,
    Phantom,
    Solflare,
    PrivateKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletCredentialData {
    Turnkey { api_key: String, organization_id: String, private_key_id: String },
    Phantom { encrypted_private_key: String },
    Solflare { public_key: String },
    PrivateKey { private_key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub id: String,
    pub wallet_type: WalletType,
    pub public_key: String,
    pub label: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

#[async_trait]
pub trait WalletProvider: Send + Sync {
    async fn connect(&self, credentials: &WalletCredentials) -> Result<WalletConnection>;
    async fn get_public_key(&self, connection: &WalletConnection) -> Result<String>;
    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8]) -> Result<Vec<u8>>;
    async fn disconnect(&self, connection: &WalletConnection) -> Result<()>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalletConnection {
    pub id: String,
    pub wallet_type: WalletType,
    pub connection_data: ConnectionData,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConnectionData {
    Turnkey { session_token: String },
    Phantom { session_id: String },
    Solflare { session_token: String },
    PrivateKey { private_key: String },
}

pub struct WalletManager {
    providers: HashMap<WalletType, Box<dyn WalletProvider>>,
    active_connections: dashmap::DashMap<String, WalletConnection>,
}

impl WalletManager {
    pub fn new() -> Self {
        let mut providers: HashMap<WalletType, Box<dyn WalletProvider>> = HashMap::new();
        
        providers.insert(WalletType::Turnkey, Box::new(crate::wallet::turnkey::TurnkeyProvider::new()));
        providers.insert(WalletType::Phantom, Box::new(crate::wallet::phantom::PhantomProvider::new()));
        providers.insert(WalletType::Solflare, Box::new(crate::wallet::solflare::SolflareProvider::new()));
        
        Self {
            providers,
            active_connections: dashmap::DashMap::new(),
        }
    }

    pub async fn connect_wallet(&self, credentials: WalletCredentials) -> Result<WalletConnection> {
        let provider = self.providers.get(&credentials.wallet_type)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("Unsupported wallet type: {:?}", credentials.wallet_type)
            ))?;

        let connection = provider.connect(&credentials).await?;
        
        self.active_connections.insert(connection.id.clone(), connection.clone());
        Ok(connection)
    }

    pub async fn disconnect_wallet(&self, connection_id: &str) -> Result<()> {
        if let Some((_, connection)) = self.active_connections.remove(connection_id) {
            let provider = self.providers.get(&connection.wallet_type)
                .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                    format!("Unsupported wallet type: {:?}", connection.wallet_type)
                ))?;
            
            provider.disconnect(&connection).await?;
        }
        
        Ok(())
    }

    pub fn get_connection(&self, connection_id: &str) -> Option<WalletConnection> {
        self.active_connections.get(connection_id).map(|entry| entry.clone())
    }

    pub fn list_active_connections(&self) -> Vec<WalletConnection> {
        self.active_connections.iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub async fn sign_with_wallet(&self, connection_id: &str, transaction: &[u8]) -> Result<Vec<u8>> {
        let connection = self.active_connections.get(connection_id)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("No active connection found for ID: {}", connection_id)
            ))?;

        let provider = self.providers.get(&connection.wallet_type)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("Unsupported wallet type: {:?}", connection.wallet_type)
            ))?;

        provider.sign_transaction(&connection, transaction).await
    }
}

impl Default for WalletManager {
    fn default() -> Self {
        Self::new()
    }
}
