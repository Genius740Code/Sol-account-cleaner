#[cfg(test)]
mod tests {
    use crate::core::types::*;
    use crate::rpc::ConnectionPool;
    use crate::wallet::{WalletManager, WalletCredentials, WalletCredentialData};
    use solana_sdk::pubkey::Pubkey;
    use std::sync::Arc;
    use std::str::FromStr;
    use uuid::Uuid;

    fn create_test_recovery_config() -> RecoveryConfig {
        RecoveryConfig {
            max_accounts_per_transaction: 10,
            min_balance_lamports: 1000,
            max_concurrent_recoveries: Some(5),
            priority_fee_lamports: 5000,
        }
    }

    fn create_test_fee_structure() -> FeeStructure {
        FeeStructure {
            fee_percentage_bps: 100, // 1%
            min_fee_lamports: 5000,
            max_fee_lamports: 1_000_000,
            fee_waiver_threshold_lamports: 10_000,
            firm_wallet_address: Some("11111111111111111111111111111112".to_string()),
            fee_waived: false,
        }
    }

    fn create_test_wallet_credentials() -> WalletCredentials {
        WalletCredentials {
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
        
        // Test hex format
        let hex_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
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
    async fn test_group_accounts_for_recovery() {
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

        // Test with 25 accounts, should create 3 batches (10, 10, 5)
        let accounts: Vec<String> = (0..25)
            .map(|i| format!("111111111111111111111111111111{:02x}", i))
            .collect();

        let batches = recovery_manager.group_accounts_for_recovery(&accounts).unwrap();
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 10);
        assert_eq!(batches[1].len(), 10);
        assert_eq!(batches[2].len(), 5);

        // Test with empty accounts list
        let empty_accounts: Vec<String> = vec![];
        let result = recovery_manager.group_accounts_for_recovery(&empty_accounts);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_destination_address() {
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

        // Test valid address
        let valid_address = "11111111111111111111111111111112";
        let result = recovery_manager.validate_destination_address(valid_address);
        assert!(result.is_ok());

        // Test invalid address
        let invalid_address = "invalid_address";
        let result = recovery_manager.validate_destination_address(invalid_address);
        assert!(result.is_err());

        // Test system program address (should be rejected)
        let system_address = "11111111111111111111111111111111";
        let result = recovery_manager.validate_destination_address(system_address);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_generate_audit_signature() {
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

        let request = RecoveryRequest {
            id: Uuid::new_v4(),
            wallet_address: "11111111111111111111111111111112".to_string(),
            destination_address: "11111111111111111111111111111113".to_string(),
            empty_accounts: vec![
                "11111111111111111111111111111114".to_string(),
                "11111111111111111111111111111115".to_string(),
            ],
            max_fee_lamports: Some(100000),
            wallet_connection_id: None,
            user_id: None,
            created_at: chrono::Utc::now(),
        };

        let timestamp = chrono::Utc::now();
        let signature = recovery_manager.generate_audit_signature(&request, timestamp);
        
        // Should generate a hex string
        assert!(!signature.is_empty());
        assert!(signature.len() == 64); // HMAC-SHA256 produces 64 char hex string
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

    #[test]
    fn test_recovery_security_new() {
        let security = RecoverySecurity::new();
        
        assert_eq!(security.max_recovery_lamports, 100_000_000_000);
        assert!(security.allowed_destinations.is_empty());
        assert!(!security.require_multi_sig);
        assert_eq!(security.session_timeout_secs, 3600);
        assert!(!security.audit_key.is_empty());
    }

    #[test]
    fn test_recovery_security_with_limits() {
        let allowed_destinations = vec![
            Pubkey::from_str("11111111111111111111111111111112").unwrap(),
            Pubkey::from_str("11111111111111111111111111111113").unwrap(),
        ];
        
        let security = RecoverySecurity::with_limits(
            50_000_000_000,
            allowed_destinations.clone(),
        );
        
        assert_eq!(security.max_recovery_lamports, 50_000_000_000);
        assert_eq!(security.allowed_destinations, allowed_destinations);
        assert!(security.require_multi_sig);
    }

    #[test]
    fn test_generate_nonce() {
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

        let nonce1 = recovery_manager.generate_nonce();
        let nonce2 = recovery_manager.generate_nonce();
        
        // Nonces should be different (time-based)
        assert_ne!(nonce1, nonce2);
        
        // Should be reasonable values (not zero, not too large)
        assert!(nonce1 > 0);
        assert!(nonce2 > 0);
        assert!(nonce1 < u64::MAX / 2);
        assert!(nonce2 < u64::MAX / 2);
    }

    #[tokio::test]
    async fn test_private_key_provider_connection() {
        let provider = PrivateKeyProvider::new();
        let credentials = create_test_wallet_credentials();
        
        // This should fail with invalid test key, but the structure should be correct
        let result = provider.connect(&credentials).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_rate_limit() {
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

        // Test rate limiting with empty audit log
        let wallet_address = "11111111111111111111111111111112";
        let is_rate_limited = recovery_manager.check_rate_limit(wallet_address).await.unwrap();
        assert!(!is_rate_limited); // Should not be rate limited with empty log
    }
}
