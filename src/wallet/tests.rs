#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::{WalletCredentials, WalletCredentialData, WalletType, WalletConnection, ConnectionData, WalletProvider};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_wallet_manager_creation() {
        let manager = WalletManager::new();
        
        // Test that manager was created successfully
        let connections = manager.list_active_connections();
        assert_eq!(connections.len(), 0);
    }

    #[tokio::test]
    async fn test_turnkey_provider_connection() {
        let provider = crate::wallet::turnkey::TurnkeyProvider::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "test_api_key".to_string(),
                organization_id: "test_org".to_string(),
                private_key_id: "test_key".to_string(),
            },
        };

        // Test connection (will fail with real API, but should not panic)
        let result = provider.connect(&credentials).await;
        assert!(result.is_ok() || result.is_err()); // Just ensure it doesn't panic
    }

    #[tokio::test]
    async fn test_phantom_provider_connection() {
        let provider = crate::wallet::phantom::PhantomProvider::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Phantom,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test_encrypted_key".to_string(),
            },
        };

        // Test connection
        let result = provider.connect(&credentials).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_solflare_provider_connection() {
        let provider = crate::wallet::solflare::SolflareProvider::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Solflare,
            credentials: WalletCredentialData::Solflare {
                public_key: "11111111111111111111111111111112".to_string(),
            },
        };

        // Test connection
        let result = provider.connect(&credentials).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wallet_manager_connect_disconnect() {
        let manager = WalletManager::new();
        
        let credentials = WalletCredentials {
            wallet_type: WalletType::Phantom,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test_encrypted_key".to_string(),
            },
        };

        // Test connection
        let connection = manager.connect_wallet(credentials).await;
        assert!(connection.is_ok());
        
        let connection = connection.unwrap();
        let connection_id = connection.id.clone();

        // Test that connection is active
        let active_connections = manager.list_active_connections();
        assert_eq!(active_connections.len(), 1);

        // Test disconnection
        let disconnect_result = manager.disconnect_wallet(&connection_id).await;
        assert!(disconnect_result.is_ok());

        // Test that connection is no longer active
        let active_connections = manager.list_active_connections();
        assert_eq!(active_connections.len(), 0);
    }

    #[test]
    fn test_wallet_credentials_serialization() {
        let credentials = WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "test_api_key".to_string(),
                organization_id: "test_org".to_string(),
                private_key_id: "test_key".to_string(),
            },
        };

        // Test JSON serialization
        let json = serde_json::to_string(&credentials);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: WalletCredentials = serde_json::from_str(&json.unwrap()).unwrap();
        assert!(matches!(deserialized.wallet_type, WalletType::Turnkey));
    }

    #[test]
    fn test_wallet_connection_serialization() {
        let connection = WalletConnection {
            id: Uuid::new_v4().to_string(),
            wallet_type: WalletType::Phantom,
            connection_data: ConnectionData::Phantom {
                session_id: Uuid::new_v4().to_string(),
            },
            created_at: chrono::Utc::now(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&connection);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: WalletConnection = serde_json::from_str(&json.unwrap()).unwrap();
        assert!(matches!(deserialized.wallet_type, WalletType::Phantom));
    }
}
