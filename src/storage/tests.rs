#[cfg(test)]
mod tests {
    use crate::storage::{CacheConfig, DatabaseConfig, SqlitePersistenceManager, persistence::PersistenceManager, CacheManager};
    use crate::core::{ScanResult, WalletInfo, ScanStatus};
    use uuid::Uuid;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_cache_manager_creation() {
        let config = CacheConfig {
            ttl_seconds: 300,
            max_size: 1000,
            cleanup_interval_seconds: 60,
            enable_hierarchical_cache: true,
            l1_cache_size: 100,
            l2_cache_size: 200,
            compression_threshold: 1024,
            enable_metrics: true,
        };

        let cache = CacheManager::new(config);
        
        // Test that cache was created successfully
        assert_eq!(cache.stats().total_entries, 0);
    }

    #[tokio::test]
    async fn test_cache_put_get() {
        let config = CacheConfig {
            ttl_seconds: 300,
            max_size: 1000,
            cleanup_interval_seconds: 60,
            enable_hierarchical_cache: true,
            l1_cache_size: 100,
            l2_cache_size: 200,
            compression_threshold: 1024,
            enable_metrics: true,
        };

        let cache = CacheManager::new(config);
        
        let scan_result = ScanResult {
            id: Uuid::new_v4(),
            wallet_address: "11111111111111111111111111111112".to_string(),
            status: ScanStatus::Completed,
            result: None,
            empty_accounts_found: 0,
            recoverable_sol: 0.0,
            scan_time_ms: 0,
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            error_message: None,
        };

        // Test put
        let put_result = cache.set(&scan_result.wallet_address, &scan_result).await;
        assert!(put_result.is_ok());

        // Test get
        let get_result: Result<Option<crate::core::ScanResult>, crate::core::SolanaRecoverError> = cache.get(&scan_result.wallet_address).await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cache_ttl() {
        let config = CacheConfig {
            ttl_seconds: 1, // 1 second TTL
            max_size: 1000,
            cleanup_interval_seconds: 1,
            enable_hierarchical_cache: false,
            l1_cache_size: 100,
            l2_cache_size: 200,
            compression_threshold: 1024,
            enable_metrics: false,
        };

        let cache = CacheManager::new(config);
        
        let scan_result = ScanResult {
            id: Uuid::new_v4(),
            wallet_address: "11111111111111111111111111111112".to_string(),
            status: ScanStatus::Completed,
            result: None,
            empty_accounts_found: 0,
            recoverable_sol: 0.0,
            scan_time_ms: 0,
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            error_message: None,
        };

        // Test put
        let put_result = cache.set(&scan_result.wallet_address, &scan_result).await;
        assert!(put_result.is_ok());

        // Test get immediately (should work)
        let get_result: Result<Option<crate::core::ScanResult>, crate::core::SolanaRecoverError> = cache.get(&scan_result.wallet_address).await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().is_some());

        // Wait for TTL to expire
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Test get after TTL (should return None)
        let get_result: Result<Option<crate::core::ScanResult>, crate::core::SolanaRecoverError> = cache.get(&scan_result.wallet_address).await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sqlite_persistence_creation() {
        let config = DatabaseConfig {
            database_url: ":memory:".to_string(), // Use in-memory database for testing
            max_connections: 1,
        };

        let persistence = SqlitePersistenceManager::new(config).await;
        assert!(persistence.is_ok());
    }

    #[tokio::test]
    async fn test_sqlite_save_get_scan_result() {
        let config = DatabaseConfig {
            database_url: ":memory:".to_string(),
            max_connections: 1,
        };

        let persistence = SqlitePersistenceManager::new(config).await.unwrap();
        
        let scan_result = ScanResult {
            id: Uuid::new_v4(),
            wallet_address: "11111111111111111111111111111112".to_string(),
            status: ScanStatus::Completed,
            result: None,
            empty_accounts_found: 0,
            recoverable_sol: 0.0,
            scan_time_ms: 0,
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            error_message: None,
        };

        // Test save
        let save_result = persistence.save_scan_result(&scan_result).await;
        assert!(save_result.is_ok());

        // Test get
        let get_result = persistence.get_scan_result(&scan_result.id.to_string()).await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_sqlite_save_get_wallet_info() {
        let config = DatabaseConfig {
            database_url: ":memory:".to_string(),
            max_connections: 1,
        };

        let persistence = SqlitePersistenceManager::new(config).await.unwrap();
        
        let wallet_info = WalletInfo {
            address: "11111111111111111111111111111112".to_string(),
            pubkey: solana_sdk::pubkey::Pubkey::from_str("11111111111111111111111111111112").unwrap(),
            total_accounts: 100,
            empty_accounts: 2,
            recoverable_lamports: 5000000,
            recoverable_sol: 0.005,
            empty_account_addresses: vec![
                "3GZteV3GuQEm1B47yHnoYFYHpKWYvqxzgGWCBe5fZiSD".to_string(),
                "7L3nzrnqYajCPe1RG5CiwURYcZyvH5oNVcjESyCg7fnA".to_string(),
            ],
            scan_time_ms: 1000,
        };

        // Test save
        let save_result = persistence.save_wallet_info(&wallet_info).await;
        assert!(save_result.is_ok());

        // Test get
        let get_result = persistence.get_wallet_info(&wallet_info.address).await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().is_some());
    }

    #[test]
    fn test_cache_config_serialization() {
        let config = CacheConfig {
            ttl_seconds: 300,
            max_size: 1000,
            cleanup_interval_seconds: 60,
            enable_hierarchical_cache: true,
            l1_cache_size: 100,
            l2_cache_size: 200,
            compression_threshold: 1024,
            enable_metrics: true,
        };

        // Test JSON serialization
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: CacheConfig = serde_json::from_str(&json.unwrap()).unwrap();
        assert!(deserialized.ttl_seconds == 300);
        assert!(deserialized.max_size == 1000);
    }

    #[test]
    fn test_database_config_serialization() {
        let config = DatabaseConfig {
            database_url: "./test.db".to_string(),
            max_connections: 10,
        };

        // Test JSON serialization
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: DatabaseConfig = serde_json::from_str(&json.unwrap()).unwrap();
        assert!(deserialized.database_url == "./test.db");
        assert!(deserialized.max_connections == 10);
    }
}
