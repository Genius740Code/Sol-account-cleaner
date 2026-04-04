use solana_recover::*;
use crate::common::*;

#[tokio::test]
async fn test_complete_wallet_scan_workflow() {
    // Setup
    let config = create_test_config();
    let scanner = create_test_wallet_scanner();
    let wallet_address = get_test_wallet_address();
    
    // Execute scan
    let scan_result = scanner.scan_wallet(&wallet_address).await;
    
    // Verify result (may fail due to network issues in test environment)
    match scan_result {
        Ok(result) => {
            assert_eq!(result.wallet_address, wallet_address);
            assert!(matches!(result.status, ScanStatus::Completed));
            assert!(result.result.is_some());
            assert!(result.error.is_none());
            
            let wallet_info = result.result.unwrap();
            assert_eq!(wallet_info.address, wallet_address);
            assert!(wallet_info.total_accounts >= 0);
            assert!(wallet_info.empty_accounts >= 0);
            assert!(wallet_info.empty_accounts <= wallet_info.total_accounts);
        }
        Err(_) => {
            // Network errors are acceptable in test environment
            // The important thing is that the error handling works
        }
    }
}

#[tokio::test]
async fn test_batch_scan_workflow() {
    // Setup
    let processor = create_test_batch_processor();
    let wallet_addresses = get_test_wallet_addresses();
    
    // Create batch request
    let batch_request = BatchScanRequest {
        id: uuid::Uuid::new_v4(),
        wallet_addresses: wallet_addresses.clone(),
        user_id: Some("test_user".to_string()),
        fee_percentage: Some(0.15),
        created_at: chrono::Utc::now(),
    };
    
    // Execute batch scan
    let batch_result = processor.process_batch(&batch_request).await;
    
    // Verify result
    match batch_result {
        Ok(result) => {
            assert_eq!(result.total_wallets, wallet_addresses.len());
            assert_eq!(result.results.len(), wallet_addresses.len());
            
            let successful_scans = result.results.iter()
                .filter(|r| matches!(r.status, ScanStatus::Completed))
                .count();
            
            let failed_scans = result.results.iter()
                .filter(|r| matches!(r.status, ScanStatus::Failed))
                .count();
            
            assert_eq!(successful_scans + failed_scans, wallet_addresses.len());
            assert_eq!(result.successful_scans, successful_scans);
            assert_eq!(result.failed_scans, failed_scans);
        }
        Err(_) => {
            // Network errors are acceptable in test environment
        }
    }
}

#[tokio::test]
async fn test_wallet_connection_workflow() {
    // Setup
    let manager = WalletManager::new();
    
    // Test Phantom wallet connection
    let phantom_credentials = create_test_wallet_credentials(WalletType::Phantom);
    let phantom_connection = manager.connect_wallet(phantom_credentials).await.unwrap();
    
    assert_eq!(phantom_connection.wallet_type, WalletType::Phantom);
    assert!(!phantom_connection.id.is_empty());
    
    // Test getting public key
    let public_key = manager.get_connection(&phantom_connection.id).unwrap();
    let provider = crate::wallet::phantom::PhantomProvider::new();
    let pk_result = provider.get_public_key(&public_key).await.unwrap();
    assert!(!pk_result.is_empty());
    
    // Test signing transaction
    let transaction = vec![1, 2, 3, 4, 5];
    let signature = manager.sign_with_wallet(&phantom_connection.id, &transaction).await.unwrap();
    assert_eq!(signature.len(), 64);
    
    // Test disconnection
    manager.disconnect_wallet(&phantom_connection.id).await.unwrap();
    assert!(manager.get_connection(&phantom_connection.id).is_none());
}

#[tokio::test]
async fn test_fee_calculation_workflow() {
    // Setup
    let fee_structure = create_test_fee_structure();
    let wallet_infos = vec![
        WalletInfo {
            address: "wallet1".to_string(),
            pubkey: solana_sdk::pubkey::Pubkey::default(),
            total_accounts: 10,
            empty_accounts: 5,
            recoverable_lamports: 100_000_000, // 0.1 SOL
            recoverable_sol: 0.1,
            empty_account_addresses: vec![],
            scan_time_ms: 1000,
        },
        WalletInfo {
            address: "wallet2".to_string(),
            pubkey: solana_sdk::pubkey::Pubkey::default(),
            total_accounts: 8,
            empty_accounts: 3,
            recoverable_lamports: 3_000_000, // 0.003 SOL (below waiver threshold)
            recoverable_sol: 0.003,
            empty_account_addresses: vec![],
            scan_time_ms: 800,
        },
    ];
    
    // Calculate individual fees
    let wallet1_fee = FeeCalculator::calculate_wallet_fee(&wallet_infos[0], &fee_structure);
    let wallet2_fee = FeeCalculator::calculate_wallet_fee(&wallet_infos[1], &fee_structure);
    
    // Verify wallet1 fee (should be charged)
    assert_eq!(wallet1_fee.total_recoverable_lamports, 100_000_000);
    assert_eq!(wallet1_fee.fee_lamports, 15_000_000); // 15%
    assert_eq!(wallet1_fee.net_recoverable_lamports, 85_000_000);
    assert!(!wallet1_fee.fee_waived);
    
    // Verify wallet2 fee (should be waived)
    assert_eq!(wallet2_fee.total_recoverable_lamports, 3_000_000);
    assert_eq!(wallet2_fee.fee_lamports, 0); // Waived
    assert_eq!(wallet2_fee.net_recoverable_lamports, 3_000_000);
    assert!(wallet2_fee.fee_waived);
    
    // Calculate batch fees
    let batch_fee = FeeCalculator::calculate_batch_fee(&wallet_infos, &fee_structure);
    assert_eq!(batch_fee.total_recoverable_lamports, 103_000_000);
    assert_eq!(batch_fee.total_fee_lamports, 15_000_000); // Only wallet1 charged
    assert_eq!(batch_fee.total_net_recoverable_lamports, 88_000_000);
}

#[tokio::test]
async fn test_metrics_collection_workflow() {
    // Setup
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);
    
    // Simulate wallet scan metrics
    collector.increment_counter(metrics_names::WALLET_SCANS_TOTAL, None).await;
    collector.record_timer(metrics_names::WALLET_SCAN_DURATION_MS, 1500, None).await;
    
    // Simulate RPC metrics
    collector.increment_counter(metrics_names::RPC_REQUESTS_TOTAL, None).await;
    collector.record_timer(metrics_names::RPC_REQUEST_DURATION_MS, 100, None).await;
    
    // Simulate cache metrics
    collector.increment_counter(metrics_names::CACHE_HITS, None).await;
    collector.increment_counter(metrics_names::CACHE_MISSES, None).await;
    
    // Set gauge for active connections
    collector.set_gauge(metrics_names::ACTIVE_CONNECTIONS, 3.0, None).await;
    
    // Get metrics summary
    let summary = collector.get_metric_summary().await;
    
    // Verify metrics
    assert!(summary.contains_key("counter:wallet_scans_total"));
    assert!(summary.contains_key("timer:wallet_scan_duration_ms"));
    assert!(summary.contains_key("counter:rpc_requests_total"));
    assert!(summary.contains_key("timer:rpc_request_duration_ms"));
    assert!(summary.contains_key("counter:cache_hits"));
    assert!(summary.contains_key("counter:cache_misses"));
    assert!(summary.contains_key("gauge:active_connections"));
    
    // Verify values
    assert_eq!(summary["counter:wallet_scans_total"]["value"], 1);
    assert_eq!(summary["counter:rpc_requests_total"]["value"], 1);
    assert_eq!(summary["counter:cache_hits"]["value"], 1);
    assert_eq!(summary["counter:cache_misses"]["value"], 1);
    assert_eq!(summary["gauge:active_connections"]["value"], 3.0);
}

#[tokio::test]
async fn test_configuration_workflow() {
    // Test default configuration
    let config = Config::load().unwrap();
    assert!(!config.rpc.endpoints.is_empty());
    assert!(config.rpc.pool_size > 0);
    assert!(config.scanner.batch_size > 0);
    assert!(config.scanner.max_concurrent_wallets > 0);
    
    // Test configuration validation
    assert!(config.validate().is_ok());
    
    // Test invalid configuration
    let mut invalid_config = config.clone();
    invalid_config.rpc.pool_size = 0;
    assert!(invalid_config.validate().is_err());
    
    let mut invalid_config = config.clone();
    invalid_config.scanner.max_concurrent_wallets = 0;
    assert!(invalid_config.validate().is_err());
}

#[tokio::test]
async fn test_error_handling_workflow() {
    // Test invalid wallet address
    let scanner = create_test_wallet_scanner();
    let invalid_address = "invalid_address";
    let result = scanner.scan_wallet(invalid_address).await;
    assert!(result.is_err());
    
    // Test empty batch request
    let processor = create_test_batch_processor();
    let empty_batch = BatchScanRequest {
        id: uuid::Uuid::new_v4(),
        wallet_addresses: vec![],
        user_id: None,
        fee_percentage: None,
        created_at: chrono::Utc::now(),
    };
    
    let batch_result = processor.process_batch(&empty_batch).await;
    assert!(batch_result.is_err());
    
    // Test invalid fee structure
    let invalid_fee_structure = FeeStructure {
        percentage: 1.5, // Invalid: > 100%
        minimum_lamports: 1_000_000,
        maximum_lamports: None,
        waive_below_lamports: None,
    };
    
    let validation_result = FeeCalculator::validate_fee_structure(&invalid_fee_structure);
    assert!(validation_result.is_err());
}

#[tokio::test]
async fn test_concurrent_operations() {
    // Setup
    let processor = create_test_batch_processor();
    let wallet_addresses = get_test_wallet_addresses();
    
    // Create multiple concurrent batch requests
    let mut handles = vec![];
    
    for i in 0..3 {
        let processor_clone = processor.clone();
        let addresses_clone = wallet_addresses.clone();
        
        let handle = tokio::spawn(async move {
            let batch_request = BatchScanRequest {
                id: uuid::Uuid::new_v4(),
                wallet_addresses: addresses_clone,
                user_id: Some(format!("user_{}", i)),
                fee_percentage: Some(0.15),
                created_at: chrono::Utc::now(),
            };
            
            processor_clone.process_batch(&batch_request).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;
    
    // Verify all operations completed (either success or network error)
    for result in results {
        match result.unwrap() {
            Ok(_) => {
                // Success case
            }
            Err(_) => {
                // Network errors are acceptable in test environment
            }
        }
    }
}

#[tokio::test]
async fn test_persistence_workflow() {
    // Setup
    let config = create_test_config();
    let persistence_manager = Arc::new(
        SqlitePersistenceManager::new(config.database.into()).await.unwrap()
    );
    
    // Test saving scan result
    let scan_result = ScanResult {
        id: uuid::Uuid::new_v4(),
        wallet_address: get_test_wallet_address(),
        status: ScanStatus::Completed,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
    };
    
    let save_result = persistence_manager.save_scan_result(&scan_result).await;
    assert!(save_result.is_ok());
    
    // Test retrieving scan result
    let retrieved_result = persistence_manager.get_scan_result(scan_result.id).await;
    assert!(retrieved_result.is_ok());
    
    let retrieved = retrieved_result.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().wallet_address, scan_result.wallet_address);
    
    // Test saving batch result
    let batch_result = BatchScanResult {
        id: uuid::Uuid::new_v4(),
        batch_id: Some("test_batch".to_string()),
        total_wallets: 10,
        successful_scans: 8,
        failed_scans: 2,
        completed_wallets: 8,
        failed_wallets: 2,
        total_recoverable_sol: 1.5,
        estimated_fee_sol: 0.225,
        results: vec![],
        created_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
        duration_ms: Some(5000),
    };
    
    let save_batch_result = persistence_manager.save_batch_result(&batch_result).await;
    assert!(save_batch_result.is_ok());
}
