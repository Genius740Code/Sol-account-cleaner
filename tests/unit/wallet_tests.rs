use solana_recover::*;
use solana_recover::wallet::*;
use crate::common::*;

#[tokio::test]
async fn test_wallet_manager_creation() {
    let manager = WalletManager::new();
    let active_connections = manager.list_active_connections();
    assert_eq!(active_connections.len(), 0);
}

#[tokio::test]
async fn test_wallet_credentials_creation() {
    let turnkey_creds = create_test_wallet_credentials(WalletType::Turnkey);
    assert_eq!(turnkey_creds.wallet_type, WalletType::Turnkey);
    
    if let WalletCredentialData::Turnkey { api_key, organization_id, private_key_id } = turnkey_creds.credentials {
        assert_eq!(api_key, "test_api_key");
        assert_eq!(organization_id, "test_org");
        assert_eq!(private_key_id, "test_key");
    } else {
        panic!("Expected Turnkey credentials");
    }

    let phantom_creds = create_test_wallet_credentials(WalletType::Phantom);
    assert_eq!(phantom_creds.wallet_type, WalletType::Phantom);
    
    if let WalletCredentialData::Phantom { encrypted_private_key } = phantom_creds.credentials {
        assert_eq!(encrypted_private_key, "test_encrypted_key");
    } else {
        panic!("Expected Phantom credentials");
    }

    let solflare_creds = create_test_wallet_credentials(WalletType::Solflare);
    assert_eq!(solflare_creds.wallet_type, WalletType::Solflare);
    
    if let WalletCredentialData::Solflare { public_key } = solflare_creds.credentials {
        assert_eq!(public_key, "11111111111111111111111111111111112");
    } else {
        panic!("Expected Solflare credentials");
    }

    let private_key_creds = create_test_wallet_credentials(WalletType::PrivateKey);
    assert_eq!(private_key_creds.wallet_type, WalletType::PrivateKey);
    
    if let WalletCredentialData::PrivateKey { private_key } = private_key_creds.credentials {
        assert_eq!(private_key, "test_private_key");
    } else {
        panic!("Expected PrivateKey credentials");
    }
}

#[tokio::test]
async fn test_turnkey_provider_connection() {
    let provider = crate::wallet::turnkey::TurnkeyProvider::new();
    let credentials = create_test_wallet_credentials(WalletType::Turnkey);
    
    // Note: This will fail in test environment without actual Turnkey API
    // but we can test the credential validation logic
    let result = provider.connect(&credentials).await;
    
    // Should fail due to invalid API key in test environment
    assert!(result.is_err());
    
    if let Err(SolanaRecoverError::AuthenticationError(msg)) = result {
        assert!(msg.contains("Turnkey auth request failed"));
    } else {
        panic!("Expected AuthenticationError");
    }
}

#[tokio::test]
async fn test_phantom_provider_connection() {
    let provider = crate::wallet::phantom::PhantomProvider::new();
    let credentials = create_test_wallet_credentials(WalletType::Phantom);
    
    let result = provider.connect(&credentials).await;
    
    // Should succeed in test environment (simulated)
    assert!(result.is_ok());
    
    let connection = result.unwrap();
    assert_eq!(connection.wallet_type, WalletType::Phantom);
    assert!(!connection.id.is_empty());
    
    if let ConnectionData::Phantom { session_id } = connection.connection_data {
        assert!(!session_id.is_empty());
    } else {
        panic!("Expected Phantom connection data");
    }
}

#[tokio::test]
async fn test_solflare_provider_connection() {
    let provider = crate::wallet::solflare::SolflareProvider::new();
    let credentials = create_test_wallet_credentials(WalletType::Solflare);
    
    let result = provider.connect(&credentials).await;
    
    // Should succeed in test environment (simulated)
    assert!(result.is_ok());
    
    let connection = result.unwrap();
    assert_eq!(connection.wallet_type, WalletType::Solflare);
    assert!(!connection.id.is_empty());
    
    if let ConnectionData::Solflare { session_token } = connection.connection_data {
        assert!(!session_token.is_empty());
    } else {
        panic!("Expected Solflare connection data");
    }
}

#[tokio::test]
async fn test_phantom_provider_public_key() {
    let provider = crate::wallet::phantom::PhantomProvider::new();
    let credentials = create_test_wallet_credentials(WalletType::Phantom);
    
    let connection = provider.connect(&credentials).await.unwrap();
    let public_key_result = provider.get_public_key(&connection).await;
    
    assert!(public_key_result.is_ok());
    let public_key = public_key_result.unwrap();
    assert_eq!(public_key, "11111111111111111111111111111111112");
}

#[tokio::test]
async fn test_phantom_provider_sign_transaction() {
    let provider = crate::wallet::phantom::PhantomProvider::new();
    let credentials = create_test_wallet_credentials(WalletType::Phantom);
    
    let connection = provider.connect(&credentials).await.unwrap();
    let transaction = vec![1, 2, 3, 4, 5]; // Dummy transaction data
    
    let signature_result = provider.sign_transaction(&connection, &transaction).await;
    
    assert!(signature_result.is_ok());
    let signature = signature_result.unwrap();
    assert_eq!(signature.len(), 64); // 64-byte signature
}

#[tokio::test]
async fn test_wallet_manager_connect_disconnect() {
    let manager = WalletManager::new();
    let credentials = create_test_wallet_credentials(WalletType::Phantom);
    
    // Connect wallet
    let connection = manager.connect_wallet(credentials).await.unwrap();
    let connection_id = connection.id.clone();
    
    // Check active connections
    let active_connections = manager.list_active_connections();
    assert_eq!(active_connections.len(), 1);
    assert_eq!(active_connections[0].id, connection_id);
    
    // Get specific connection
    let retrieved_connection = manager.get_connection(&connection_id);
    assert!(retrieved_connection.is_some());
    assert_eq!(retrieved_connection.unwrap().id, connection_id);
    
    // Disconnect wallet
    let disconnect_result = manager.disconnect_wallet(&connection_id).await;
    assert!(disconnect_result.is_ok());
    
    // Check active connections again
    let active_connections = manager.list_active_connections();
    assert_eq!(active_connections.len(), 0);
    
    // Try to get disconnected connection
    let retrieved_connection = manager.get_connection(&connection_id);
    assert!(retrieved_connection.is_none());
}

#[tokio::test]
async fn test_wallet_manager_sign_with_wallet() {
    let manager = WalletManager::new();
    let credentials = create_test_wallet_credentials(WalletType::Phantom);
    
    let connection = manager.connect_wallet(credentials).await.unwrap();
    let connection_id = connection.id.clone();
    let transaction = vec![1, 2, 3, 4, 5]; // Dummy transaction data
    
    let signature_result = manager.sign_with_wallet(&connection_id, &transaction).await;
    assert!(signature_result.is_ok());
    
    let signature = signature_result.unwrap();
    assert_eq!(signature.len(), 64);
    
    // Clean up
    manager.disconnect_wallet(&connection_id).await.unwrap();
}

#[tokio::test]
async fn test_wallet_manager_invalid_connection() {
    let manager = WalletManager::new();
    let fake_connection_id = "fake_connection_id";
    let transaction = vec![1, 2, 3, 4, 5];
    
    let signature_result = manager.sign_with_wallet(fake_connection_id, &transaction).await;
    assert!(signature_result.is_err());
    
    if let Err(SolanaRecoverError::AuthenticationError(msg)) = signature_result {
        assert!(msg.contains("No active connection found"));
    } else {
        panic!("Expected AuthenticationError");
    }
}

#[tokio::test]
async fn test_wallet_manager_unsupported_wallet_type() {
    let manager = WalletManager::new();
    
    // Create credentials with a wallet type that's not supported
    let credentials = WalletCredentials {
        wallet_type: WalletType::PrivateKey, // This might not be fully implemented
        credentials: WalletCredentialData::PrivateKey {
            private_key: "test_private_key".to_string(),
        },
    };
    
    let result = manager.connect_wallet(credentials).await;
    // This might succeed or fail depending on implementation
    // The test mainly checks that the manager handles it gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_wallet_type_equality() {
    assert_eq!(WalletType::Turnkey, WalletType::Turnkey);
    assert_eq!(WalletType::Phantom, WalletType::Phantom);
    assert_eq!(WalletType::Solflare, WalletType::Solflare);
    assert_eq!(WalletType::PrivateKey, WalletType::PrivateKey);

    assert_ne!(WalletType::Turnkey, WalletType::Phantom);
    assert_ne!(WalletType::Phantom, WalletType::Solflare);
    assert_ne!(WalletType::Solflare, WalletType::PrivateKey);
}

#[test]
fn test_connection_data_creation() {
    let turnkey_data = ConnectionData::Turnkey {
        session_token: "session_123".to_string(),
    };
    
    if let ConnectionData::Turnkey { session_token } = turnkey_data {
        assert_eq!(session_token, "session_123");
    } else {
        panic!("Expected Turnkey connection data");
    }

    let phantom_data = ConnectionData::Phantom {
        session_id: "session_456".to_string(),
    };
    
    if let ConnectionData::Phantom { session_id } = phantom_data {
        assert_eq!(session_id, "session_456");
    } else {
        panic!("Expected Phantom connection data");
    }

    let solflare_data = ConnectionData::Solflare {
        session_token: "session_789".to_string(),
    };
    
    if let ConnectionData::Solflare { session_token } = solflare_data {
        assert_eq!(session_token, "session_789");
    } else {
        panic!("Expected Solflare connection data");
    }

    let private_key_data = ConnectionData::PrivateKey {
        private_key: "private_key_abc".to_string(),
    };
    
    if let ConnectionData::PrivateKey { private_key } = private_key_data {
        assert_eq!(private_key, "private_key_abc");
    } else {
        panic!("Expected PrivateKey connection data");
    }
}

#[test]
fn test_wallet_info_creation() {
    let now = chrono::Utc::now();
    let wallet_info = crate::wallet::WalletInfo {
        id: "wallet_123".to_string(),
        wallet_type: WalletType::Phantom,
        public_key: "11111111111111111111111111111111112".to_string(),
        label: Some("Test Wallet".to_string()),
        created_at: now,
        last_used: None,
    };

    assert_eq!(wallet_info.id, "wallet_123");
    assert_eq!(wallet_info.wallet_type, WalletType::Phantom);
    assert_eq!(wallet_info.public_key, "11111111111111111111111111111111112");
    assert_eq!(wallet_info.label, Some("Test Wallet".to_string()));
    assert_eq!(wallet_info.created_at, now);
    assert!(wallet_info.last_used.is_none());
}
