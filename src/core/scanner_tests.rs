#[cfg(test)]
mod tests {
    use crate::core::{SolanaRecoverError};
    use solana_sdk::pubkey::Pubkey;
    use std::sync::Arc;
    use std::str::FromStr;
    use chrono::Utc;
    use solana_account_decoder::UiAccountEncoding;
    use base64::{Engine as _, engine::general_purpose};

    fn create_mock_keyed_account(pubkey_str: &str, owner: &str, lamports: u64, data: Vec<u8>) -> solana_client::rpc_response::RpcKeyedAccount {
        solana_client::rpc_response::RpcKeyedAccount {
            pubkey: pubkey_str.to_string(),
            account: solana_client::rpc_response::RpcAccount {
                lamports,
                data: data.clone(),
                owner: owner.to_string(),
                executable: false,
                rent_epoch: 0,
            },
        }
    }

    fn create_mock_token_account(pubkey_str: &str, lamports: u64, amount: u64) -> solana_client::rpc_response::RpcKeyedAccount {
        // Create token account data structure (simplified)
        let mut data = vec![0u8; 82]; // Token account is typically 82 bytes
        
        // Add mint (first 32 bytes)
        let mint_pubkey = Pubkey::from_str("11111111111111111111111111111111").unwrap();
        data[0..32].copy_from_slice(&mint_pubkey.to_bytes());
        
        // Add owner (next 32 bytes)
        let owner_pubkey = Pubkey::from_str("11111111111111111111111111111112").unwrap();
        data[32..64].copy_from_slice(&owner_pubkey.to_bytes());
        
        // Add amount (bytes 64-72)
        data[64..72].copy_from_slice(&amount.to_le_bytes());
        
        create_mock_keyed_account(pubkey_str, &spl_token::id().to_string(), lamports, data)
    }

    fn create_mock_system_account(pubkey_str: &str, lamports: u64) -> solana_client::rpc_response::RpcKeyedAccount {
        create_mock_keyed_account(pubkey_str, &solana_program::system_program::id().to_string(), lamports, vec![])
    }

    #[tokio::test]
    async fn test_parse_token_account_from_binary_base64() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        let token_account = create_mock_token_account("TestTokenAccount", 1000000, 0);
        let data_str = general_purpose::STANDARD.encode(&token_account.account.data);
        
        let result = scanner.parse_token_account_from_binary(&data_str, &UiAccountEncoding::Base64);
        assert!(result.is_ok());
        
        let token_info = result.unwrap();
        assert_eq!(token_info.amount, 0);
        assert_eq!(token_info.mint, "11111111111111111111111111111111");
    }

    #[tokio::test]
    async fn test_parse_token_account_from_binary_base58() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        let token_account = create_mock_token_account("TestTokenAccount", 1000000, 1000000);
        let data_str = bs58::encode(&token_account.account.data).into_string();
        
        let result = scanner.parse_token_account_from_binary(&data_str, &UiAccountEncoding::Base58);
        assert!(result.is_ok());
        
        let token_info = result.unwrap();
        assert_eq!(token_info.amount, 1000000);
        assert_eq!(token_info.mint, "11111111111111111111111111111111");
    }

    #[tokio::test]
    async fn test_parse_token_account_invalid_data() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        // Test with too short data
        let short_data = "YWJj"; // "abc" in base64
        let result = scanner.parse_token_account_from_binary(short_data, &UiAccountEncoding::Base64);
        assert!(result.is_err());
        
        // Test with invalid base64
        let invalid_base64 = "invalid_base64!";
        let result = scanner.parse_token_account_from_binary(invalid_base64, &UiAccountEncoding::Base64);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_empty_token_account() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        // Test empty token account (0 tokens, but with lamports)
        let empty_token_account = create_mock_token_account("EmptyTokenAccount", 2039280, 0);
        let wallet_address = "11111111111111111111111111111112";
        
        let result = scanner.check_empty_account(&empty_token_account, wallet_address).await;
        assert!(result.is_ok());
        
        let empty_account = result.unwrap();
        assert!(empty_account.is_some());
        let account = empty_account.unwrap();
        assert_eq!(account.address, "EmptyTokenAccount");
        assert_eq!(account.lamports, 2039280);
        assert_eq!(account.owner, spl_token::id().to_string());
        assert!(account.mint.is_some());
        assert_eq!(account.mint.unwrap(), "11111111111111111111111111111111");
    }

    #[tokio::test]
    async fn test_check_non_empty_token_account() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        // Test non-empty token account (has tokens)
        let non_empty_token_account = create_mock_token_account("NonEmptyTokenAccount", 2039280, 1000000);
        let wallet_address = "11111111111111111111111111111112";
        
        let result = scanner.check_empty_account(&non_empty_token_account, wallet_address).await;
        assert!(result.is_ok());
        
        let empty_account = result.unwrap();
        assert!(empty_account.is_none()); // Should not be considered empty
    }

    #[tokio::test]
    async fn test_check_empty_system_account() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        // Test system account with rent-exempt amount
        let rent_exempt_amount = 2039280;
        let system_account = create_mock_system_account("SystemAccount", rent_exempt_amount);
        let wallet_address = "11111111111111111111111111111112";
        
        let result = scanner.check_empty_account(&system_account, wallet_address).await;
        assert!(result.is_ok());
        
        let empty_account = result.unwrap();
        assert!(empty_account.is_some());
        let account = empty_account.unwrap();
        assert_eq!(account.address, "SystemAccount");
        assert_eq!(account.lamports, rent_exempt_amount);
        assert_eq!(account.owner, solana_program::system_program::id().to_string());
        assert!(account.mint.is_none());
    }

    #[tokio::test]
    async fn test_check_main_wallet_address() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        // Test that main wallet address is never flagged as recoverable
        let wallet_address = "11111111111111111111111111111112";
        let wallet_account = create_mock_system_account(wallet_address, 1000000);
        
        let result = scanner.check_empty_account(&wallet_account, wallet_address).await;
        assert!(result.is_ok());
        
        let empty_account = result.unwrap();
        assert!(empty_account.is_none()); // Should never recover the main wallet
    }

    #[tokio::test]
    async fn test_scan_wallet_invalid_address() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        let invalid_address = "invalid_wallet_address";
        let result = scanner.scan_wallet(invalid_address).await;
        
        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.status, ScanStatus::Failed);
        assert!(scan_result.error.is_some());
    }

    #[tokio::test]
    async fn test_scan_wallet_structure() {
        let scanner = WalletScanner::new(Arc::new(MockConnectionPool::new()));
        
        let wallet_address = "11111111111111111111111111111112";
        let result = scanner.scan_wallet(wallet_address).await;
        
        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.wallet_address, wallet_address);
        assert_eq!(scan_result.id.to_string().len(), 36); // UUID string length
        assert!(scan_result.created_at <= Utc::now());
    }

    // Mock connection pool for testing
    struct MockConnectionPool;

    impl MockConnectionPool {
        fn new() -> Self {
            Self
        }
    }

    #[async_trait::async_trait]
    impl crate::rpc::ConnectionPoolTrait for MockConnectionPool {
        async fn get_client(&self) -> Result<Arc<crate::rpc::RpcClientWrapper>> {
            // Return a mock client that will fail on any actual RPC call
            // This is fine for unit tests that don't need real RPC calls
            Err(SolanaRecoverError::NetworkError("Mock connection pool".to_string()))
        }
    }

    #[test]
    fn test_token_account_info_structure() {
        let token_info = TokenAccountInfo {
            mint: "11111111111111111111111111111111".to_string(),
            amount: 1000000,
        };
        
        assert_eq!(token_info.mint, "11111111111111111111111111111111");
        assert_eq!(token_info.amount, 1000000);
    }

    #[test]
    fn test_constants() {
        assert_eq!(LAMPORTS_PER_SOL, 1_000_000_000.0);
    }

    #[test]
    fn test_wallet_scanner_creation() {
        let mock_pool = Arc::new(MockConnectionPool::new());
        let scanner = WalletScanner::new(mock_pool);
        
        // Scanner should be created successfully
        // We can't test much more without a real connection pool
        assert!(true);
    }
}
