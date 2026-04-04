use crate::core::{Result, SolanaRecoverError};
use crate::wallet::{WalletProvider, WalletCredentials, WalletConnection, ConnectionData};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct TurnkeyProvider {
    client: Client,
    api_url: String,
}

impl TurnkeyProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_url: "https://api.turnkey.com".to_string(),
        }
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
        if let crate::wallet::WalletCredentialData::Turnkey { api_key, organization_id, private_key_id } = &credentials.credentials {
            let auth_request = TurnkeyAuthRequest {
                api_key: api_key.clone(),
                organization_id: organization_id.clone(),
                private_key_id: private_key_id.clone(),
            };

            let response = self.client
                .post(&format!("{}/v1/auth", self.api_url))
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

            let connection = WalletConnection {
                id: uuid::Uuid::new_v4().to_string(),
                wallet_type: crate::wallet::WalletType::Turnkey,
                connection_data: ConnectionData::Turnkey {
                    session_token: auth_response.session_token,
                },
                created_at: chrono::Utc::now(),
            };

            Ok(connection)
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid Turnkey credentials".to_string()
            ))
        }
    }

    async fn get_public_key(&self, connection: &WalletConnection) -> Result<String> {
        if let ConnectionData::Turnkey { session_token } = &connection.connection_data {
            let response = self.client
                .get(&format!("{}/v1/public-key?session_token={}", self.api_url, session_token))
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
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid Turnkey connection".to_string()
            ))
        }
    }

    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8]) -> Result<Vec<u8>> {
        if let ConnectionData::Turnkey { session_token } = &connection.connection_data {
            let transaction_hex = hex::encode(transaction);
            
            let sign_request = TurnkeySignRequest {
                session_token: session_token.clone(),
                transaction: transaction_hex,
            };

            let response = self.client
                .post(&format!("{}/v1/sign", self.api_url))
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

            hex::decode(&sign_response.signature)
                .map_err(|e| SolanaRecoverError::TransactionFailed(
                    format!("Failed to decode signature: {}", e)
                ))
        } else {
            Err(SolanaRecoverError::AuthenticationError(
                "Invalid Turnkey connection".to_string()
            ))
        }
    }

    async fn disconnect(&self, connection: &WalletConnection) -> Result<()> {
        if let ConnectionData::Turnkey { session_token } = &connection.connection_data {
            let _ = self.client
                .post(&format!("{}/v1/logout", self.api_url))
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
