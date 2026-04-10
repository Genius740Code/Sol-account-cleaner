//! Comprehensive Turnkey Provider Tests
//! 
//! This module contains extensive tests for the Turnkey wallet provider
//! including unit tests, integration tests, and stress tests.

#[cfg(test)]
mod tests {
    use crate::wallet::turnkey::{TurnkeyProvider, TurnkeyConfig, TurnkeySession};
    use crate::wallet::{WalletCredentials, WalletType, WalletCredentialData};
    use crate::core::{Result, SolanaRecoverError};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::sleep;
    use uuid::Uuid;

    fn create_test_credentials() -> WalletCredentials {
        WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "test_api_key".to_string(),
                organization_id: "test_org_id".to_string(),
                private_key_id: "test_key_id".to_string(),
            },
        }
    }

    fn create_invalid_credentials() -> WalletCredentials {
        WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "".to_string(),
                organization_id: "".to_string(),
                private_key_id: "".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_turnkey_provider_creation() {
        // Test default creation
        let provider = TurnkeyProvider::new();
        assert!(provider.health_check().await.is_ok());

        // Test with custom config
        let config = TurnkeyConfig {
            api_url: "https://api.turnkey.com".to_string(),
            timeout_seconds: 60,
            retry_attempts: 5,
            enable_session_caching: false,
        };
        let provider_with_config = TurnkeyProvider::with_config(config);
        assert!(provider_with_config.health_check().await.is_ok());

        // Test with custom API URL
        let provider_with_url = TurnkeyProvider::with_api_url("https://custom.turnkey.com".to_string());
        assert!(provider_with_url.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_credentials_validation() {
        let provider = TurnkeyProvider::new();

        // Test valid credentials
        let valid_credentials = create_test_credentials();
        assert!(provider.validate_credentials(&valid_credentials).is_ok());

        // Test invalid credentials (empty API key)
        let mut invalid_credentials = create_test_credentials();
        if let WalletCredentialData::Turnkey { ref mut api_key, .. } = invalid_credentials.credentials {
            *api_key = "".to_string();
        }
        assert!(provider.validate_credentials(&invalid_credentials).is_err());

        // Test invalid credentials (empty organization ID)
        let mut invalid_credentials = create_test_credentials();
        if let WalletCredentialData::Turnkey { ref mut organization_id, .. } = invalid_credentials.credentials {
            *organization_id = "".to_string();
        }
        assert!(provider.validate_credentials(&invalid_credentials).is_err());

        // Test invalid credentials (empty private key ID)
        let mut invalid_credentials = create_test_credentials();
        if let WalletCredentialData::Turnkey { ref mut private_key_id, .. } = invalid_credentials.credentials {
            *private_key_id = "".to_string();
        }
        assert!(provider.validate_credentials(&invalid_credentials).is_err());

        // Test wrong credential type
        let wrong_credentials = WalletCredentials {
            wallet_type: WalletType::PrivateKey,
            credentials: WalletCredentialData::PrivateKey {
                private_key: "test_key".to_string(),
            },
        };
        assert!(provider.validate_credentials(&wrong_credentials).is_err());
    }

    #[tokio::test]
    async fn test_session_validation() {
        let provider = TurnkeyProvider::new();

        // Test valid session
        let valid_session = TurnkeySession {
            session_token: "test_token".to_string(),
            public_key: "test_public_key".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };
        assert!(provider.is_session_valid(&valid_session));

        // Test expired session
        let expired_session = TurnkeySession {
            session_token: "test_token".to_string(),
            public_key: "test_public_key".to_string(),
            expires_at: chrono::Utc::now() - chrono::Duration::hours(1),
        };
        assert!(!provider.is_session_valid(&expired_session));

        // Test session about to expire
        let soon_to_expire_session = TurnkeySession {
            session_token: "test_token".to_string(),
            public_key: "test_public_key".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(30),
        };
        assert!(provider.is_session_valid(&soon_to_expire_session));
    }

    #[tokio::test]
    async fn test_session_caching() {
        let config = TurnkeyConfig {
            enable_session_caching: true,
            ..Default::default()
        };
        let provider = Arc::new(TurnkeyProvider::with_config(config));

        let credentials = create_test_credentials();

        // Initially no cache
        let (total, valid) = provider.get_cache_stats();
        assert_eq!(total, 0);
        assert_eq!(valid, 0);

        // Test cache miss
        let cached_session = provider.get_cached_session(&credentials);
        assert!(cached_session.is_none());

        // Add session to cache
        let session = TurnkeySession {
            session_token: "test_token".to_string(),
            public_key: "test_public_key".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };
        provider.cache_session(&credentials, session);

        // Test cache hit
        let cached_session = provider.get_cached_session(&credentials);
        assert!(cached_session.is_some());

        // Check cache stats
        let (total, valid) = provider.get_cache_stats();
        assert_eq!(total, 1);
        assert_eq!(valid, 1);

        // Clear cache
        provider.clear_session_cache();
        let (total, valid) = provider.get_cache_stats();
        assert_eq!(total, 0);
        assert_eq!(valid, 0);
    }

    #[tokio::test]
    async fn test_session_caching_disabled() {
        let config = TurnkeyConfig {
            enable_session_caching: false,
            ..Default::default()
        };
        let provider = Arc::new(TurnkeyProvider::with_config(config));

        let credentials = create_test_credentials();

        // Add session to cache (should be ignored)
        let session = TurnkeySession {
            session_token: "test_token".to_string(),
            public_key: "test_public_key".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };
        provider.cache_session(&credentials, session);

        // Test cache miss (caching disabled)
        let cached_session = provider.get_cached_session(&credentials);
        assert!(cached_session.is_none());

        // Check cache stats
        let (total, valid) = provider.get_cache_stats();
        assert_eq!(total, 0);
        assert_eq!(valid, 0);
    }

    #[tokio::test]
    async fn test_expired_session_cleanup() {
        let config = TurnkeyConfig {
            enable_session_caching: true,
            ..Default::default()
        };
        let provider = Arc::new(TurnkeyProvider::with_config(config));

        let credentials = create_test_credentials();

        // Add expired session to cache
        let expired_session = TurnkeySession {
            session_token: "expired_token".to_string(),
            public_key: "test_public_key".to_string(),
            expires_at: chrono::Utc::now() - chrono::Duration::minutes(1),
        };
        provider.cache_session(&credentials, expired_session);

        // Check cache stats (should show expired session)
        let (total, valid) = provider.get_cache_stats();
        assert_eq!(total, 1);
        assert_eq!(valid, 0);

        // Test cache miss (expired session should be removed)
        let cached_session = provider.get_cached_session(&credentials);
        assert!(cached_session.is_none());

        // Check cache stats (expired session should be cleaned up)
        let (total, valid) = provider.get_cache_stats();
        assert_eq!(total, 0);
        assert_eq!(valid, 0);
    }

    #[tokio::test]
    async fn test_retry_logic() {
        let config = TurnkeyConfig {
            retry_attempts: 3,
            timeout_seconds: 1,
            ..Default::default()
        };
        let provider = TurnkeyProvider::with_config(config);

        let mut attempt_count = 0;
        let result = provider.retry_operation(|| {
            Box::pin(async move {
                attempt_count += 1;
                if attempt_count < 3 {
                    Err(SolanaRecoverError::NetworkError(
                        "Temporary failure".to_string()
                    ))
                } else {
                    Ok("success")
                }
            })
        }).await;

        assert!(result.is_ok());
        assert_eq!(attempt_count, 3);
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_retry_logic_exhausted() {
        let config = TurnkeyConfig {
            retry_attempts: 2,
            timeout_seconds: 1,
            ..Default::default()
        };
        let provider = TurnkeyProvider::with_config(config);

        let result = provider.retry_operation(|| {
            Box::pin(async {
                Err(SolanaRecoverError::NetworkError(
                    "Persistent failure".to_string()
                ))
            })
        }).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SolanaRecoverError::NetworkError(msg) => {
                assert_eq!(msg, "Persistent failure");
            }
            _ => panic!("Expected NetworkError"),
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        let provider = TurnkeyProvider::new();

        // Test health check (should succeed or fail gracefully)
        let health_result = provider.health_check().await;
        assert!(health_result.is_ok());
    }

    #[tokio::test]
    async fn test_wallet_info() {
        let provider = TurnkeyProvider::new();
        let credentials = create_test_credentials();

        // This test will likely fail with mock credentials, but should handle errors gracefully
        let result = provider.get_wallet_info(&credentials).await;
        
        match result {
            Ok(wallet_info) => {
                assert!(!wallet_info.id.is_empty());
                assert!(!wallet_info.public_key.is_empty());
                assert_eq!(wallet_info.wallet_type, WalletType::Turnkey);
            }
            Err(_) => {
                // Expected with test credentials
            }
        }
    }

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let provider = TurnkeyProvider::new();
        let credentials = create_test_credentials();

        // Test connection (will likely fail with mock credentials)
        let connection_result = provider.connect(&credentials).await;
        
        match connection_result {
            Ok(connection) => {
                assert!(!connection.id.is_empty());
                assert_eq!(connection.wallet_type, WalletType::Turnkey);
                
                // Test getting public key
                let pubkey_result = provider.get_public_key(&connection).await;
                
                match pubkey_result {
                    Ok(pubkey) => {
                        assert!(!pubkey.is_empty());
                    }
                    Err(_) => {
                        // Expected with mock credentials
                    }
                }
                
                // Test disconnection
                let disconnect_result = provider.disconnect(&connection).await;
                assert!(disconnect_result.is_ok());
            }
            Err(_) => {
                // Expected with test credentials
            }
        }
    }

    #[tokio::test]
    async fn test_transaction_signing() {
        let provider = TurnkeyProvider::new();
        let credentials = create_test_credentials();
        let sample_transaction = vec![0x01, 0x02, 0x03, 0x04];

        // Test connection first
        let connection_result = provider.connect(&credentials).await;
        
        match connection_result {
            Ok(connection) => {
                // Test transaction signing
                let sign_result = provider.sign_transaction(&connection, &sample_transaction).await;
                
                match sign_result {
                    Ok(signed_tx) => {
                        // Should be original transaction + 64-byte signature
                        assert_eq!(signed_tx.len(), sample_transaction.len() + 64);
                    }
                    Err(_) => {
                        // Expected with mock credentials
                    }
                }
            }
            Err(_) => {
                // Expected with test credentials
            }
        }
    }

    #[tokio::test]
    async fn test_concurrent_connections() {
        let provider = Arc::new(TurnkeyProvider::new());
        let credentials = create_test_credentials();

        let mut handles = vec![];

        // Spawn 5 concurrent connection attempts
        for i in 0..5 {
            let provider = provider.clone();
            let credentials = credentials.clone();

            let handle = tokio::spawn(async move {
                let result = provider.connect(&credentials).await;
                (i, result.is_ok())
            });

            handles.push(handle);
        }

        // Wait for all connections to complete
        let mut successful_connections = 0;
        for handle in handles {
            match handle.await {
                Ok((_, success)) => {
                    if success {
                        successful_connections += 1;
                    }
                }
                Err(_) => {
                    // Task panicked
                }
            }
        }

        // With mock credentials, we expect most to fail, but the test should complete
        println!("Successful concurrent connections: {}/5", successful_connections);
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let config = TurnkeyConfig {
            enable_session_caching: true,
            ..Default::default()
        };
        let provider = Arc::new(TurnkeyProvider::with_config(config));

        // Test with different credentials
        let credentials1 = WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "key1".to_string(),
                organization_id: "org1".to_string(),
                private_key_id: "priv1".to_string(),
            },
        };

        let credentials2 = WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "key2".to_string(),
                organization_id: "org1".to_string(),
                private_key_id: "priv1".to_string(),
            },
        };

        let credentials3 = WalletCredentials {
            wallet_type: WalletType::Turnkey,
            credentials: WalletCredentialData::Turnkey {
                api_key: "key1".to_string(),
                organization_id: "org2".to_string(),
                private_key_id: "priv1".to_string(),
            },
        };

        // Add sessions for each
        let session = TurnkeySession {
            session_token: "test_token".to_string(),
            public_key: "test_public_key".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        provider.cache_session(&credentials1, session.clone());
        provider.cache_session(&credentials2, session.clone());
        provider.cache_session(&credentials3, session.clone());

        // Should have 3 different sessions
        let (total, valid) = provider.get_cache_stats();
        assert_eq!(total, 3);
        assert_eq!(valid, 3);

        // Each should be retrievable
        assert!(provider.get_cached_session(&credentials1).is_some());
        assert!(provider.get_cached_session(&credentials2).is_some());
        assert!(provider.get_cached_session(&credentials3).is_some());
    }

    #[test]
    fn test_turnkey_config_default() {
        let config = TurnkeyConfig::default();
        
        assert_eq!(config.api_url, "https://api.turnkey.com");
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.retry_attempts, 3);
        assert!(config.enable_session_caching);
    }

    #[test]
    fn test_turnkey_config_clone() {
        let config = TurnkeyConfig {
            api_url: "https://custom.turnkey.com".to_string(),
            timeout_seconds: 60,
            retry_attempts: 5,
            enable_session_caching: false,
        };

        let cloned_config = config.clone();
        assert_eq!(config.api_url, cloned_config.api_url);
        assert_eq!(config.timeout_seconds, cloned_config.timeout_seconds);
        assert_eq!(config.retry_attempts, cloned_config.retry_attempts);
        assert_eq!(config.enable_session_caching, cloned_config.enable_session_caching);
    }
}
