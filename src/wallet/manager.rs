use crate::core::{Result, SolanaRecoverError};
use crate::wallet::transaction_validator::TransactionValidator;
use crate::wallet::nonce_manager::NonceManager;
use crate::wallet::audit_logger::{AuditLogger, SecurityContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

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


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalletConnection {
    pub id: String,
    pub wallet_type: WalletType,
    pub connection_data: ConnectionData,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait WalletProvider: Send + Sync {
    async fn connect(&self, credentials: &WalletCredentials) -> crate::core::Result<WalletConnection>;
    async fn get_public_key(&self, connection: &WalletConnection) -> crate::core::Result<String>;
    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8], rpc_url: Option<&str>) -> crate::core::Result<Vec<u8>>;
    async fn disconnect(&self, connection: &WalletConnection) -> crate::core::Result<()>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConnectionData {
    Turnkey { session_token: String },
    Phantom { session_id: String },
    Solflare { session_token: String },
    PrivateKey { private_key: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalletManagerConfig {
    pub enable_turnkey: bool,
    pub enable_phantom: bool,
    pub enable_solflare: bool,
    pub enable_private_key: bool,
    pub solflare_timeout_ms: u64,
    pub solflare_retry_attempts: u32,
    pub enable_solflare_mobile: bool,
    pub enable_solflare_web: bool,
    pub max_connections: usize,
    pub connection_timeout_seconds: u64,
}

impl Default for WalletManagerConfig {
    fn default() -> Self {
        Self {
            enable_turnkey: true,
            enable_phantom: true,
            enable_solflare: true,
            enable_private_key: true,
            solflare_timeout_ms: 15000,
            solflare_retry_attempts: 3,
            enable_solflare_mobile: true,
            enable_solflare_web: true,
            max_connections: 100,
            connection_timeout_seconds: 30,
        }
    }
}

pub struct WalletManager {
    providers: HashMap<WalletType, Box<dyn WalletProvider>>,
    active_connections: dashmap::DashMap<String, WalletConnection>,
    config: WalletManagerConfig,
    validator: Arc<TransactionValidator>,
    nonce_manager: Arc<NonceManager>,
    audit_logger: Arc<AuditLogger>,
}

impl WalletManager {
    pub fn new() -> Self {
        Self::with_config(WalletManagerConfig::default())
    }

    pub fn with_config(config: WalletManagerConfig) -> Self {
        let mut providers: HashMap<WalletType, Box<dyn WalletProvider>> = HashMap::new();
        
        // Initialize shared components
        let validator = Arc::new(TransactionValidator::new()
            .with_limits(5, 20, 1_000_000_000_000) // 5 signers, 20 instructions, 1000 SOL max
            .require_simulation(true));
        
        let nonce_manager = Arc::new(NonceManager::default());
        let audit_logger = Arc::new(AuditLogger::default());
        
        // Initialize Turnkey provider
        if config.enable_turnkey {
            providers.insert(WalletType::Turnkey, Box::new(crate::wallet::turnkey::TurnkeyProvider::new()));
        }
        
        // Initialize Phantom provider (temporarily disabled for testing)
        if config.enable_phantom {
            // providers.insert(WalletType::Phantom, Box::new(crate::wallet::phantom::PhantomProvider::new()));
        }
        
        // Initialize Solflare provider with custom config (temporarily disabled for testing)
        if config.enable_solflare {
            // let solflare_config = crate::wallet::solflare::SolflareConfig {
            //     timeout_ms: config.solflare_timeout_ms,
            //     retry_attempts: config.solflare_retry_attempts,
            //     enable_mobile_support: config.enable_solflare_mobile,
            //     enable_web_support: config.enable_solflare_web,
            // };
            // providers.insert(WalletType::Solflare, Box::new(crate::wallet::solflare::SolflareProvider::with_config(solflare_config)));
        }
        
        // Initialize PrivateKey provider with enhanced components
        if config.enable_private_key {
            providers.insert(WalletType::PrivateKey, Box::new(
                crate::wallet::private_key::PrivateKeyProvider::with_components(
                    validator.clone(),
                    nonce_manager.clone(),
                    audit_logger.clone(),
                )
            ));
        }
        
        Self {
            providers,
            active_connections: dashmap::DashMap::new(),
            config,
            validator,
            nonce_manager,
            audit_logger,
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

    pub async fn sign_with_wallet(&self, connection_id: &str, transaction: &[u8], rpc_url: Option<&str>) -> Result<Vec<u8>> {
        let connection = self.active_connections.get(connection_id)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("No active connection found for ID: {}", connection_id)
            ))?;

        let provider = self.providers.get(&connection.wallet_type)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("Unsupported wallet type: {:?}", connection.wallet_type)
            ))?;

        provider.sign_transaction(&connection, transaction, rpc_url).await
    }

    pub async fn get_supported_wallets(&self) -> Vec<WalletType> {
        self.providers.keys().cloned().collect()
    }

    pub async fn get_wallet_info(&self, connection_id: &str) -> Option<WalletInfo> {
        if let Some(connection) = self.get_connection(connection_id) {
            let public_key = match self.providers.get(&connection.wallet_type) {
                Some(provider) => provider.get_public_key(&connection).await.ok(),
                None => None,
            };

            public_key.map(|pk| WalletInfo {
                id: connection.id.clone(),
                wallet_type: connection.wallet_type,
                public_key: pk,
                label: None,
                created_at: connection.created_at,
                last_used: Some(chrono::Utc::now()),
            })
        } else {
            None
        }
    }

    pub async fn cleanup_expired_connections(&self) -> Result<usize> {
        let mut expired_connections = Vec::new();
        let timeout_duration = chrono::Duration::seconds(self.config.connection_timeout_seconds as i64);
        let now = chrono::Utc::now();

        for entry in self.active_connections.iter() {
            let connection = entry.value();
            if now.signed_duration_since(connection.created_at) > timeout_duration {
                expired_connections.push(connection.id.clone());
            }
        }

        let count = expired_connections.len();
        for connection_id in expired_connections {
            let _ = self.disconnect_wallet(&connection_id).await;
        }

        Ok(count)
    }

    pub async fn get_connection_metrics(&self) -> serde_json::Value {
        let mut wallet_type_counts = std::collections::HashMap::new();
        let mut total_connections = 0;

        for entry in self.active_connections.iter() {
            let connection = entry.value();
            *wallet_type_counts.entry(format!("{:?}", connection.wallet_type)).or_insert(0) += 1;
            total_connections += 1;
        }

        serde_json::json!({
            "total_connections": total_connections,
            "max_connections": self.config.max_connections,
            "connections_by_type": wallet_type_counts,
            "supported_wallets": self.providers.keys().map(|t| format!("{:?}", t)).collect::<Vec<_>>(),
            "config": {
                "enable_turnkey": self.config.enable_turnkey,
                "enable_phantom": self.config.enable_phantom,
                "enable_solflare": self.config.enable_solflare,
                "enable_private_key": self.config.enable_private_key,
            }
        })
    }

    pub async fn validate_connection(&self, connection_id: &str) -> Result<bool> {
        if let Some(connection) = self.get_connection(connection_id) {
            if let Some(provider) = self.providers.get(&connection.wallet_type) {
                // Try to get public key as a connection health check
                match provider.get_public_key(&connection).await {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    pub async fn reconnect_wallet(&self, connection_id: &str, credentials: &WalletCredentials) -> Result<WalletConnection> {
        // Disconnect existing connection
        let _ = self.disconnect_wallet(connection_id).await;
        
        // Connect with new credentials
        self.connect_wallet(credentials.clone()).await
    }

    pub async fn batch_sign_transactions(
        &self, 
        connection_id: &str, 
        transactions: &[Vec<u8>]
    ) -> Result<Vec<Result<Vec<u8>>>> {
        let connection = self.active_connections.get(connection_id)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("No active connection found for ID: {}", connection_id)
            ))?;

        let provider = self.providers.get(&connection.wallet_type)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("Unsupported wallet type: {:?}", connection.wallet_type)
            ))?;

        let mut results = Vec::with_capacity(transactions.len());
        
        for (_i, transaction) in transactions.iter().enumerate() {
            // Validate each transaction before signing
            if let Ok(tx) = bincode::deserialize::<solana_sdk::transaction::Transaction>(transaction) {
                // Check for replay attacks
                if let Err(e) = self.nonce_manager.validate_transaction(&tx).await {
                    results.push(Err(e));
                    continue;
                }
                
                // Log batch signing attempt
                let security_context = self.create_security_context(connection_id);
                self.audit_logger.log_transaction_signing(
                    None,
                    format!("{:?}", connection.wallet_type),
                    None,
                    &tx,
                    solana_sdk::signature::Signature::default(),
                    security_context,
                    crate::wallet::audit_logger::RiskLevel::Medium,
                ).await?;
            }
            
            let result = provider.sign_transaction(&connection, transaction, None).await;
            results.push(result);
        }

        Ok(results)
    }

    pub async fn sign_transaction_enhanced(
        &self,
        connection_id: &str,
        transaction: &[u8],
        user_id: Option<String>,
        rpc_url: Option<&str>,
    ) -> Result<Vec<u8>> {
        let connection = self.active_connections.get(connection_id)
            .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                format!("No active connection found for ID: {}", connection_id)
            ))?;

        // Validate transaction structure
        let tx: solana_sdk::transaction::Transaction = bincode::deserialize(transaction)
            .map_err(|e| SolanaRecoverError::SerializationError(format!("Failed to deserialize transaction: {}", e)))?;

        // Check for replay attacks
        self.nonce_manager.validate_transaction(&tx).await?;

        // Validate with RPC simulation
        let url = rpc_url.unwrap_or("https://api.mainnet-beta.solana.com");
        let rpc_client = solana_client::rpc_client::RpcClient::new(url);
        let validation_result = self.validator.validate_transaction(transaction, &rpc_client).await?;

        if !validation_result.is_valid {
            return Err(SolanaRecoverError::ValidationError(
                format!("Transaction validation failed: {:?}", validation_result.errors)
            ));
        }

        // Log validation warnings
        if !validation_result.warnings.is_empty() {
            let security_context = self.create_security_context(connection_id);
            self.audit_logger.log_security_violation(
                user_id,
                format!("{:?}", connection.wallet_type),
                "Transaction validation warnings".to_string(),
                serde_json::json!({
                    "warnings": validation_result.warnings,
                    "transaction_hash": validation_result.simulation_result.as_ref()
                        .and_then(|s| s.account_changes.first())
                        .map(|c| c.pubkey.to_string()),
                }),
                security_context,
            ).await?;
        }

        // Proceed with signing
        self.sign_with_wallet(connection_id, transaction, rpc_url).await
    }

    fn create_security_context(&self, connection_id: &str) -> SecurityContext {
        SecurityContext {
            ip_address: None,
            user_agent: Some("solana-recover-manager".to_string()),
            session_id: Some(connection_id.to_string()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            geo_location: None,
        }
    }

    pub async fn get_security_metrics(&self) -> Result<crate::wallet::audit_logger::SecurityMetrics> {
        self.audit_logger.get_security_metrics().await
    }

    pub async fn get_nonce_metrics(&self) -> Result<crate::wallet::nonce_manager::NonceMetrics> {
        self.nonce_manager.get_metrics().await
    }
}

impl Default for WalletManager {
    fn default() -> Self {
        Self::new()
    }
}
