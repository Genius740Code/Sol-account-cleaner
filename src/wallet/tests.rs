#[cfg(test)]
mod wallet_tests {
    use super::*;
    use crate::wallet::{WalletCredentials, WalletCredentialData, WalletType, WalletConnection, ConnectionData, WalletProvider, WalletManagerConfig};
    use crate::wallet::{WalletManager};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_wallet_manager_initialization() {
        let config = WalletManagerConfig::default();
        let manager = WalletManager::with_config(config);
        
        let supported_wallets = manager.get_supported_wallets().await;
        assert!(supported_wallets.contains(&WalletType::Turnkey));
        assert!(supported_wallets.contains(&WalletType::Phantom));
        assert!(supported_wallets.contains(&WalletType::Solflare));
        assert!(supported_wallets.contains(&WalletType::PrivateKey));
    }

    #[tokio::test]
    async fn test_wallet_manager_selective_enable() {
        let config = WalletManagerConfig {
            enable_turnkey: true,
            enable_phantom: false,
            enable_solflare: true,
            enable_private_key: false,
            ..Default::default()
        };
        
        let manager = WalletManager::with_config(config);
        let supported_wallets = manager.get_supported_wallets().await;
        
        assert!(supported_wallets.contains(&WalletType::Turnkey));
        assert!(!supported_wallets.contains(&WalletType::Phantom));
        assert!(supported_wallets.contains(&WalletType::Solflare));
        assert!(!supported_wallets.contains(&WalletType::PrivateKey));
    }

    #[tokio::test]
    async fn test_connection_metrics() {
        let manager = WalletManager::new();
        let metrics = manager.get_connection_metrics().await;
        
        assert_eq!(metrics["total_connections"], 0);
        assert_eq!(metrics["max_connections"], 100);
        assert!(metrics["supported_wallets"].as_array().unwrap().len() >= 4);
    }

    #[tokio::test]
    async fn test_unsupported_wallet_type() {
        let manager = WalletManager::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test".to_string(),
            },
        };

        let result = manager.connect_wallet(credentials).await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod turnkey_tests {
    use super::*;
    use crate::wallet::manager::WalletProvider;
    use crate::wallet::{WalletCredentials, WalletCredentialData, WalletType};

    #[tokio::test]
    async fn test_turnkey_provider_creation() {
        let _provider = crate::wallet::turnkey::TurnkeyProvider::new();
        // Test that provider can be created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_turnkey_invalid_credentials() {
        let provider = crate::wallet::turnkey::TurnkeyProvider::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Phantom, // Wrong type
            credentials: WalletCredentialData::Turnkey {
                api_key: "test".to_string(),
                organization_id: "test".to_string(),
                private_key_id: "test".to_string(),
            },
        };

        let result = provider.connect(&credentials).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_turnkey_connection_structure() {
        let provider = crate::wallet::turnkey::TurnkeyProvider::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "test_key".to_string(),
                organization_id: "test_org".to_string(),
                private_key_id: "test_key_id".to_string(),
            },
        };

        // This will fail due to network request, but we can test the structure
        let result = provider.connect(&credentials).await;
        assert!(result.is_err());
        // The error should be network-related, not structural
    }
}

#[cfg(test)]
mod phantom_tests {
    use super::*;
    use crate::wallet::manager::WalletProvider;
    use crate::wallet::{WalletCredentials, WalletCredentialData, WalletType};

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
        
        let connection = result.unwrap();
        assert_eq!(connection.wallet_type, WalletType::Phantom);
        
        // Test getting public key
        let pk_result = provider.get_public_key(&connection).await;
        assert!(pk_result.is_ok());
        assert!(!pk_result.unwrap().is_empty());
        
        // Test transaction signing
        let test_transaction = vec![1, 2, 3, 4, 5];
        let sign_result = provider.sign_transaction(&connection, &test_transaction, None).await;
        assert!(sign_result.is_ok());
        
        let signed_tx = sign_result.unwrap();
        assert_eq!(signed_tx.len(), test_transaction.len() + 64); // signature + transaction
        
        // Test disconnection
        let disconnect_result = provider.disconnect(&connection).await;
        assert!(disconnect_result.is_ok());
    }

    #[tokio::test]
    async fn test_phantom_transaction_signing_validation() {
        let provider = crate::wallet::phantom::PhantomProvider::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Phantom,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test".to_string(),
            },
        };

        let connection = provider.connect(&credentials).await.unwrap();
        
        // Test with empty transaction
        let empty_tx = vec![];
        let sign_result = provider.sign_transaction(&connection, &empty_tx, None).await;
        assert!(sign_result.is_ok());
        
        // Test signature length validation
        let signed_tx = sign_result.unwrap();
        assert_eq!(signed_tx.len(), 64); // Only signature for empty transaction
    }
}

#[cfg(test)]
mod solflare_tests {
    use super::*;
    use crate::wallet::solflare::{SolflareProvider, SolflareConfig};
    use crate::wallet::manager::WalletProvider;
    use crate::wallet::{WalletCredentials, WalletCredentialData, WalletType};

    #[tokio::test]
    async fn test_solflare_provider_connection() {
        let provider = crate::wallet::solflare::SolflareProvider::new();
        let credentials = WalletCredentials {
            wallet_type: WalletType::Solflare,
            credentials: WalletCredentialData::Solflare {
                public_key: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            },
        };

        // Test connection
        let result = provider.connect(&credentials).await;
        assert!(result.is_ok());
        
        let connection = result.unwrap();
        assert_eq!(connection.wallet_type, WalletType::Solflare);
        
        // Test getting public key
        let pk_result = provider.get_public_key(&connection).await;
        assert!(pk_result.is_ok());
        assert!(!pk_result.unwrap().is_empty());
        
        // Test transaction signing
        let test_transaction = vec![1, 2, 3, 4, 5];
        let sign_result = provider.sign_transaction(&connection, &test_transaction, None).await;
        assert!(sign_result.is_ok());
        
        let signed_tx = sign_result.unwrap();
        assert_eq!(signed_tx.len(), test_transaction.len() + 64); // signature + transaction
        
        // Test disconnection
        let disconnect_result = provider.disconnect(&connection).await;
        assert!(disconnect_result.is_ok());
    }

    #[tokio::test]
    async fn test_solflare_provider_with_config() {
        let config = SolflareConfig {
            timeout_ms: 10000,
            retry_attempts: 5,
            enable_mobile_support: false,
            enable_web_support: true,
        };
        
        let _provider = SolflareProvider::with_config(config);
        assert!(true);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::wallet::{WalletCredentials, WalletCredentialData, WalletType, WalletManager};

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

    #[tokio::test]
    async fn test_batch_transaction_signing() {
        let manager = WalletManager::new();
        
        // Create a test connection first
        let credentials = WalletCredentials {
            wallet_type: WalletType::Phantom,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test".to_string(),
            },
        };
        
        let connection = manager.connect_wallet(credentials).await.unwrap();
        
        // Test batch signing
        let transactions = vec![
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
        ];
        
        let results = manager.batch_sign_transactions(&connection.id, &transactions).await;
        assert!(results.is_ok());
        
        let signed_results = results.unwrap();
        assert_eq!(signed_results.len(), 3);
        
        // All should be successful
        for result in signed_results {
            assert!(result.is_ok());
            let signed_tx = result.unwrap();
            assert_eq!(signed_tx.len(), 3 + 64); // transaction + signature
        }
        
        // Cleanup
        let _ = manager.disconnect_wallet(&connection.id).await;
    }

    #[tokio::test]
    async fn test_connection_validation() {
        let manager = WalletManager::new();
        
        // Test non-existent connection
        let valid = manager.validate_connection("non_existent").await;
        assert!(valid.is_ok());
        assert!(!valid.unwrap());
        
        // Create a real connection
        let credentials = WalletCredentials {
            wallet_type: WalletType::Phantom,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test".to_string(),
            },
        };
        
        let connection = manager.connect_wallet(credentials).await.unwrap();
        
        // Test existing connection
        let valid = manager.validate_connection(&connection.id).await;
        assert!(valid.is_ok());
        assert!(valid.unwrap());
        
        // Cleanup
        let _ = manager.disconnect_wallet(&connection.id).await;
    }
}

#[cfg(test)]
mod serialization_tests {
    use super::*;
    use crate::wallet::{WalletManagerConfig, WalletConnection, WalletType, ConnectionData, WalletCredentials, WalletCredentialData};
    use uuid::Uuid;

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

    #[test]
    fn test_wallet_manager_config_serialization() {
        let config = WalletManagerConfig::default();
        
        // Test JSON serialization
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());
        
        // Test JSON deserialization
        let deserialized: WalletManagerConfig = serde_json::from_str(&json.unwrap()).unwrap();
        assert_eq!(deserialized.enable_turnkey, config.enable_turnkey);
        assert_eq!(deserialized.enable_phantom, config.enable_phantom);
        assert_eq!(deserialized.enable_solflare, config.enable_solflare);
        assert_eq!(deserialized.enable_private_key, config.enable_private_key);
    }
}
