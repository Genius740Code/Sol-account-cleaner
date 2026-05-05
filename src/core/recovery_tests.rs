#[cfg(test)]
mod tests {
    use crate::core::types::*;
    use crate::wallet::WalletType;
    use crate::wallet::private_key::SecretKey;
    use crate::rpc::ConnectionPool;
    use crate::wallet::{WalletManager, WalletCredentials, WalletCredentialData, PrivateKeyProvider};
    use crate::core::RecoveryManager;
    use std::str::FromStr;
    use uuid::Uuid;

    fn create_test_recovery_config() -> RecoveryConfig {
        RecoveryConfig {
            max_accounts_per_transaction: 10,
            min_balance_lamports: 1000,
            max_concurrent_recoveries: Some(5),
            priority_fee_lamports: 5000,
            max_fee_lamports: 5_000_000,
            confirmation_timeout_seconds: 120,
            retry_attempts: 3,
        }
    }

    fn create_test_fee_structure() -> FeeStructure {
        FeeStructure {
            percentage: 0.15,
            minimum_lamports: 1_000_000,
            maximum_lamports: Some(10_000_000),
            waive_below_lamports: Some(5_000_000),
            firm_wallet_address: Some("11111111111111111111111111111112".to_string()),
            authorized_firm_wallets: vec![],
        }
    }

    fn create_test_wallet_credentials() -> WalletCredentials {
        WalletCredentials {
            wallet_type: WalletType::PrivateKey,
            credentials: WalletCredentialData::PrivateKey {
                private_key: "5KQwrPbwdL6PhXujxW37FSSQZ1JiwsST4cqYz4eg5vZ8LJjKxHn3".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_parse_private_key_base58() {
        let provider = PrivateKeyProvider::new();
        
        // Test valid base58 private key (this is a test key, not real)
        let valid_key = "5KQwrPbwdL6PhXujxW37FSSQZ1JiwsST4cqYz4eg5vZ8LJjKxHn3";
        let result = provider.parse_private_key(valid_key);
        
        // This should fail with our test implementation since it's not a real Solana keypair
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_private_key_hex() {
        let provider = PrivateKeyProvider::new();
        
        // Test invalid hex format (odd number of characters)
        let hex_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde";
        let result = provider.parse_private_key(hex_key);
        
        // Should fail with invalid key
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_private_key_invalid() {
        let provider = PrivateKeyProvider::new();
        
        // Test invalid format
        let invalid_key = "not_a_valid_key";
        let result = provider.parse_private_key(invalid_key);
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_secret_key_zeroization() {
        let secret_bytes = vec![1u8; 32];
        let secret_key = SecretKey::new(secret_bytes.clone());
        
        // Verify we can access the bytes
        let accessed_bytes = secret_key.as_bytes().unwrap();
        assert_eq!(accessed_bytes, secret_bytes);
        
        // Drop should trigger zeroization
        drop(secret_key);
        
        // We can't directly test that memory was zeroized, but we can verify the Drop trait works
        // In a real scenario, you'd use memory debugging tools to verify zeroization
    }

    #[tokio::test]
    async fn test_validate_recovery_request() {
        let connection_pool = Arc::new(ConnectionPool::new(vec![], 1));
        let wallet_manager = Arc::new(WalletManager::default());
        let config = create_test_recovery_config();
        let fee_structure = create_test_fee_structure();
        
        let recovery_manager = RecoveryManager::new(
            connection_pool,
            wallet_manager,
            config,
            fee_structure,
        );

        // Test valid request
        let valid_request = RecoveryRequest {
            id: Uuid::new_v4(),
            wallet_address: "11111111111111111111111111111112".to_string(),
            destination_address: "11111111111111111111111111111113".to_string(),
            empty_accounts: vec![
                "11111111111111111111111111111114".to_string(),
                "11111111111111111111111111111115".to_string(),
            ],
            max_fee_lamports: Some(100000),
            priority_fee_lamports: Some(5000),
            wallet_connection_id: None,
            user_id: None,
            created_at: chrono::Utc::now(),
        };

        let result = recovery_manager.validate_recovery_request(&valid_request).await;
        assert!(result.is_ok());

        // Test invalid wallet address
        let mut invalid_request = valid_request.clone();
        invalid_request.wallet_address = "invalid_address".to_string();
        
        let result = recovery_manager.validate_recovery_request(&invalid_request).await;
        assert!(result.is_err());

        // Test empty accounts list
        let mut invalid_request = valid_request.clone();
        invalid_request.empty_accounts.clear();
        
        let result = recovery_manager.validate_recovery_request(&invalid_request).await;
        assert!(result.is_err());

        // Test duplicate accounts
        let mut invalid_request = valid_request.clone();
        invalid_request.empty_accounts = vec![
            "11111111111111111111111111111114".to_string(),
            "11111111111111111111111111111114".to_string(), // duplicate
        ];
        
        let result = recovery_manager.validate_recovery_request(&invalid_request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_estimate_recovery_fees() {
        let connection_pool = Arc::new(ConnectionPool::new(vec![], 1));
        let wallet_manager = Arc::new(WalletManager::default());
        let config = create_test_recovery_config();
        let fee_structure = create_test_fee_structure();
        
        let recovery_manager = RecoveryManager::new(
            connection_pool,
            wallet_manager,
            config,
            fee_structure,
        );

        let accounts = vec![
            "11111111111111111111111111111114".to_string(),
            "11111111111111111111111111111115".to_string(),
            "11111111111111111111111111111116".to_string(),
        ];

        let estimated_fees = recovery_manager.estimate_recovery_fees(&accounts).await.unwrap();
        
        // Should estimate 5000 lamports per account
        assert_eq!(estimated_fees, 15000);
    }
}
