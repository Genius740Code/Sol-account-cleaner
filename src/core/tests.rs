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
            pubkey: Pubkey::from_str("11111111111111111111111111111112").unwrap(),
            total_accounts: 10,
            empty_accounts: 3,
            recoverable_lamports: 5000000,
            recoverable_sol: 0.005,
            empty_account_addresses: vec![
                "11111111111111111111111111111113".to_string(),
                "11111111111111111111111111111114".to_string(),
                "11111111111111111111111111111115".to_string(),
            ],
            scan_time_ms: 1000,
        };

        // Test JSON serialization
        let json = serde_json::to_string(&wallet_info);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: WalletInfo = serde_json::from_str(&json.unwrap()).expect("Failed to deserialize WalletInfo");
        assert_eq!(deserialized.address, wallet_info.address);
        assert_eq!(deserialized.total_accounts, wallet_info.total_accounts);
        assert_eq!(deserialized.empty_accounts, wallet_info.empty_accounts);
        assert_eq!(deserialized.recoverable_sol, wallet_info.recoverable_sol);
    }

    #[test]
    fn test_empty_account_serialization() {
        let empty_account = EmptyAccount {
            address: "11111111111111111111111111111112".to_string(),
            lamports: 5000000,
            owner: "11111111111111111111111111111111".to_string(),
            mint: Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&empty_account);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: EmptyAccount = serde_json::from_str(&json.unwrap()).expect("Failed to deserialize EmptyAccount");
        assert_eq!(deserialized.address, empty_account.address);
        assert_eq!(deserialized.lamports, empty_account.lamports);
        assert_eq!(deserialized.owner, empty_account.owner);
        assert_eq!(deserialized.mint, empty_account.mint);
    }
}
