use crate::core::{Result, SolanaRecoverError};
use crate::wallet::{WalletProvider, WalletCredentials, WalletConnection, ConnectionData};
use crate::wallet::manager::{WalletType, WalletCredentialData};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TurnkeyConfig {
    pub api_url: String,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub enable_session_caching: bool,
}

impl Default for TurnkeyConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.turnkey.com".to_string(),
            timeout_seconds: 30,
            retry_attempts: 3,
            enable_session_caching: true,
        }
    }
}

pub struct TurnkeyProvider {
    client: Client,
    config: TurnkeyConfig,
    session_cache: dashmap::DashMap<String, TurnkeySession>,
}

#[derive(Debug, Clone)]
struct TurnkeySession {
    session_token: String,
    public_key: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl TurnkeyProvider {
    pub fn new() -> Self {
        Self::with_config(TurnkeyConfig::default())
    }

    pub fn with_config(config: TurnkeyConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            config,
            session_cache: dashmap::DashMap::new(),
        }
    }

    pub fn with_api_url(api_url: String) -> Self {
        let mut config = TurnkeyConfig::default();
        config.api_url = api_url;
        Self::with_config(config)
    }

    /// Check if a session is still valid
    fn is_session_valid(&self, session: &TurnkeySession) -> bool {
        chrono::Utc::now() < session.expires_at
    }

    /// Get cached session or return None
    fn get_cached_session(&self, credentials: &WalletCredentials) -> Option<TurnkeySession> {
        if !self.config.enable_session_caching {
            return None;
        }

        if let WalletCredentialData::Turnkey { organization_id, private_key_id, .. } = &credentials.credentials {
            let cache_key = format!("{}:{}", organization_id, private_key_id);
            if let Some(session) = self.session_cache.get(&cache_key) {
                if self.is_session_valid(&session) {
                    return Some(session.clone());
                } else {
                    // Remove expired session
                    self.session_cache.remove(&cache_key);
                }
            }
        }
        None
    }

    /// Cache a valid session
    fn cache_session(&self, credentials: &WalletCredentials, session: TurnkeySession) {
        if self.config.enable_session_caching {
            if let WalletCredentialData::Turnkey { organization_id, private_key_id, .. } = &credentials.credentials {
                let cache_key = format!("{}:{}", organization_id, private_key_id);
                self.session_cache.insert(cache_key, session);
            }
        }
    }

    /// Validate Turnkey credentials format
    fn validate_credentials(&self, credentials: &WalletCredentials) -> Result<()> {
        if let WalletCredentialData::Turnkey { api_key, organization_id, private_key_id } = &credentials.credentials {
            if api_key.is_empty() {
                return Err(SolanaRecoverError::AuthenticationError(
                    "Turnkey API key cannot be empty".to_string()
                ));
            }
            if organization_id.is_empty() {
                return Err(SolanaRecoverError::AuthenticationError(
                    "Turnkey organization ID cannot be empty".to_string()
                ));
            }
            if private_key_id.is_empty() {
                return Err(SolanaRecoverError::AuthenticationError(
                    "Turnkey private key ID cannot be empty".to_string()
                ));
            }
            Ok(())
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid Turnkey credentials format".to_string()
            ))
        }
    }

    /// Retry an operation with exponential backoff
    async fn retry_operation<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>,
    {
        let mut last_error = None;
        
        for attempt in 1..=self.config.retry_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e.clone());
                    
                    if attempt < self.config.retry_attempts {
                        let delay_ms = 1000 * (1 << (attempt - 1)); // Exponential backoff
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| SolanaRecoverError::InternalError(
            "All retry attempts failed".to_string()
        )))
    }

    /// Get wallet info without creating a connection
    pub async fn get_wallet_info(&self, credentials: &WalletCredentials) -> Result<crate::wallet::WalletInfo> {
        let connection = self.connect(credentials).await?;
        let public_key = self.get_public_key(&connection).await?;
        
        Ok(crate::wallet::WalletInfo {
            id: connection.id.clone(),
            wallet_type: WalletType::Turnkey,
            public_key,
            label: None,
            created_at: connection.created_at,
            last_used: Some(chrono::Utc::now()),
        })
    }

    /// Check if the provider is healthy
    pub async fn health_check(&self) -> Result<bool> {
        let response = self.client
            .get(&format!("{}/v1/health", self.config.api_url))
            .send()
            .await;

        match response {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Clear session cache
    pub fn clear_session_cache(&self) {
        self.session_cache.clear();
    }

    /// Get session cache statistics
    pub fn get_cache_stats(&self) -> (usize, usize) {
        let total = self.session_cache.len();
        let valid = self.session_cache.iter()
            .filter(|entry| self.is_session_valid(entry.value()))
            .count();
        (total, valid)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TurnkeyAuthRequest {
    api_key: String,
    organization_id: String,
    private_key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TurnkeyAuthResponse {
    session_token: String,
    public_key: String,
    expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TurnkeySignRequest {
    session_token: String,
    transaction: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TurnkeySignResponse {
    signature: String,
}

#[async_trait]
impl WalletProvider for TurnkeyProvider {
    async fn connect(&self, credentials: &WalletCredentials) -> Result<WalletConnection> {
        // Validate credentials first
        self.validate_credentials(credentials)?;

        // Check for cached session
        if let Some(cached_session) = self.get_cached_session(credentials) {
            return Ok(WalletConnection {
                id: uuid::Uuid::new_v4().to_string(),
                wallet_type: WalletType::Turnkey,
                connection_data: ConnectionData::Turnkey {
                    session_token: cached_session.session_token,
                },
                created_at: chrono::Utc::now(),
            });
        }

        // Perform authentication with retry logic
        let (api_key, organization_id, private_key_id) = match &credentials.credentials {
            WalletCredentialData::Turnkey { api_key, organization_id, private_key_id } => {
                (api_key.clone(), organization_id.clone(), private_key_id.clone())
            }
            _ => return Err(SolanaRecoverError::AuthenticationError(
                "Invalid credential type for Turnkey".to_string()
            )),
        };
        
        let client = self.client.clone();
        let api_url = self.config.api_url.clone();
        let auth_response = self.retry_operation(|| {
            let api_key = api_key.clone();
            let organization_id = organization_id.clone();
            let private_key_id = private_key_id.clone();
            let client = client.clone();
            let api_url = api_url.clone();
            Box::pin(async move {
                    let auth_request = TurnkeyAuthRequest {
                        api_key: api_key.clone(),
                        organization_id: organization_id.clone(),
                        private_key_id: private_key_id.clone(),
                    };

                    let response = client
                        .post(&format!("{}/v1/auth", api_url))
                        .json(&auth_request)
                        .send()
                        .await
                        .map_err(|e| SolanaRecoverError::AuthenticationError(
                            format!("Turnkey auth request failed: {}", e)
                        ))?;

                    let auth_response: TurnkeyAuthResponse = response
                        .json()
                        .await
                        .map_err(|e| SolanaRecoverError::AuthenticationError(
                            format!("Failed to parse Turnkey auth response: {}", e)
                        ))?;

                    Ok(auth_response)
            })
        }).await?;

        // Cache the session
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(1); // Sessions expire in 1 hour
        let session = TurnkeySession {
            session_token: auth_response.session_token.clone(),
            public_key: auth_response.public_key.clone(),
            expires_at,
        };
        self.cache_session(credentials, session);

        let connection = WalletConnection {
            id: uuid::Uuid::new_v4().to_string(),
            wallet_type: WalletType::Turnkey,
            connection_data: ConnectionData::Turnkey {
                session_token: auth_response.session_token,
            },
            created_at: chrono::Utc::now(),
        };

        Ok(connection)
    }

    async fn get_public_key(&self, connection: &WalletConnection) -> Result<String> {
        if let ConnectionData::Turnkey { session_token } = &connection.connection_data {
            let session_token = session_token.clone();
            let api_url = self.config.api_url.clone();
            
            self.retry_operation(move || {
                let session_token = session_token.clone();
                let api_url = api_url.clone();
                
                Box::pin(async move {
                    let response = reqwest::Client::new()
                        .get(&format!("{}/v1/public-key?session_token={}", api_url, session_token))
                        .send()
                        .await
                        .map_err(|e| SolanaRecoverError::AuthenticationError(
                            format!("Turnkey public key request failed: {}", e)
                        ))?;

                    let auth_response: TurnkeyAuthResponse = response
                        .json()
                        .await
                        .map_err(|e| SolanaRecoverError::AuthenticationError(
                            format!("Failed to parse Turnkey public key response: {}", e)
                        ))?;

                    Ok(auth_response.public_key)
                })
            }).await
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid Turnkey connection".to_string()
            ))
        }
    }

    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8], _rpc_url: Option<&str>) -> Result<Vec<u8>> {
        if let ConnectionData::Turnkey { session_token } = &connection.connection_data {
            let session_token = session_token.clone();
            let transaction_hex = hex::encode(transaction);
            let api_url = self.config.api_url.clone();
            let transaction_data = transaction.to_vec();
            
            self.retry_operation(move || {
                let session_token = session_token.clone();
                let transaction_hex = transaction_hex.clone();
                let api_url = api_url.clone();
                let transaction_data = transaction_data.clone();
                
                Box::pin(async move {
                    let sign_request = TurnkeySignRequest {
                        session_token: session_token.clone(),
                        transaction: transaction_hex.clone(),
                    };

                    let response = reqwest::Client::new()
                        .post(&format!("{}/v1/sign", api_url))
                        .json(&sign_request)
                        .send()
                        .await
                        .map_err(|e| SolanaRecoverError::TransactionFailed(
                            format!("Turnkey sign request failed: {}", e)
                        ))?;

                    let sign_response: TurnkeySignResponse = response
                        .json()
                        .await
                        .map_err(|e| SolanaRecoverError::TransactionFailed(
                            format!("Failed to parse Turnkey sign response: {}", e)
                        ))?;

                    // FIXED: Properly reconstruct the signed transaction
                    // Turnkey returns the signature, we need to create the full signed transaction
                    
                    // Decode the signature from hex
                    let signature_bytes = hex::decode(&sign_response.signature)
                        .map_err(|e| SolanaRecoverError::TransactionFailed(
                            format!("Failed to decode signature: {}", e)
                        ))?;

                    // Verify signature length (64 bytes for ed25519)
                    if signature_bytes.len() != 64 {
                        return Err(SolanaRecoverError::TransactionFailed(
                            format!("Invalid signature length: expected 64, got {}", signature_bytes.len())
                        ));
                    }

                    // Create a new signed transaction by combining the original transaction with the signature
                    // Solana transaction format: [signature(64) + transaction_data]
                    let mut signed_transaction = Vec::with_capacity(64 + transaction_data.len());
                    signed_transaction.extend_from_slice(&signature_bytes);
                    signed_transaction.extend_from_slice(&transaction_data);

                    Ok(signed_transaction)
                })
            }).await
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid Turnkey connection".to_string()
            ))
        }
    }

    async fn disconnect(&self, connection: &WalletConnection) -> Result<()> {
        if let ConnectionData::Turnkey { session_token } = &connection.connection_data {
            let _ = self.client
                .post(&format!("{}/v1/logout", self.config.api_url))
                .json(&serde_json::json!({
                    "session_token": session_token
                }))
                .send()
                .await;

            Ok(())
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid Turnkey connection".to_string()
            ))
        }
    }
}

impl Default for TurnkeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
