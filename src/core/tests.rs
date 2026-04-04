#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{WalletInfo, EmptyAccount, ScanResult, ScanStatus};
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_scan_wallet_success() {
        // Mock connection pool for testing
        let pool = crate::rpc::ConnectionPool::new(
            vec![crate::core::RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 0,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            }],
            1
        );
        let scanner = WalletScanner::new(std::sync::Arc::new(pool));
        
        // Test with a known address
        let result = scanner.scan_wallet("11111111111111111111111111111112").await;
        
        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.status, ScanStatus::Completed);
        assert!(scan_result.result.is_some());
        assert!(scan_result.error.is_none());
    }

    #[tokio::test]
    async fn test_scan_wallet_invalid_address() {
        let pool = crate::rpc::ConnectionPool::new(
            vec![crate::core::RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 0,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            }],
            1
        );
        let scanner = WalletScanner::new(std::sync::Arc::new(pool));
        
        // Test with invalid address
        let result = scanner.scan_wallet("invalid_address").await;
        
        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.status, ScanStatus::Failed);
        assert!(scan_result.result.is_none());
        assert!(scan_result.error.is_some());
    }

    #[test]
    fn test_wallet_info_serialization() {
        let wallet_info = WalletInfo {
            address: "11111111111111111111111111111112".to_string(),
            pubkey: vec![0u8; 32],
            total_accounts: 100,
            empty_accounts: 2,
            recoverable_lamports: 5000000,
            recoverable_sol: 0.005,
            empty_account_addresses: vec![
                "3GZteV3GuQEm1B47yHnoYFYHpKWYvqxzgGWCBe5fZiSD".to_string(),
                "7L3nzrnqYajCPe1RG5CiwURYcZyvH5oNVcjESyCg7fnA".to_string(),
            ],
            scan_time_ms: 1000,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&wallet_info);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: WalletInfo = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
    }

    #[test]
    fn test_empty_account_serialization() {
        let empty_account = EmptyAccount {
            address: "11111111111111111111111111111112".to_string(),
            lamports: 5000000,
            sol: 0.005,
            account_type: "system".to_string(),
            executable: false,
            owner: "11111111111111111111111111111111".to_string(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&empty_account);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: EmptyAccount = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
    }
}
