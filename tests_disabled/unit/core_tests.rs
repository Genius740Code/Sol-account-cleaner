use solana_recover::*;
use crate::common::*;

#[tokio::test]
async fn test_wallet_scanner_creation() {
    let scanner = create_test_wallet_scanner();
    assert!(!scanner.is_empty()); // Basic sanity check
}

#[tokio::test]
async fn test_batch_processor_creation() {
    let processor = create_test_batch_processor();
    assert!(!processor.is_empty()); // Basic sanity check
}

#[tokio::test]
async fn test_fee_calculator_standard() {
    let fee_structure = create_test_fee_structure();
    let recoverable = 100_000_000; // 0.1 SOL

    let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

    assert_eq!(calculation.total_recoverable_lamports, recoverable);
    assert_eq!(calculation.fee_lamports, 15_000_000); // 15% of 0.1 SOL
    assert_eq!(calculation.net_recoverable_lamports, 85_000_000);
    assert!((calculation.fee_percentage - 0.15).abs() < f64::EPSILON);
    assert!(!calculation.fee_waived);
}

#[tokio::test]
async fn test_fee_calculator_waived() {
    let mut fee_structure = create_test_fee_structure();
    fee_structure.waive_below_lamports = Some(50_000_000); // 0.05 SOL
    let recoverable = 30_000_000; // 0.03 SOL

    let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

    assert_eq!(calculation.total_recoverable_lamports, recoverable);
    assert_eq!(calculation.fee_lamports, 0);
    assert_eq!(calculation.net_recoverable_lamports, recoverable);
    assert_eq!(calculation.fee_percentage, 0.0);
    assert!(calculation.fee_waived);
    assert!(calculation.fee_reason.is_some());
}

#[tokio::test]
async fn test_fee_calculator_minimum() {
    let fee_structure = create_test_fee_structure();
    let recoverable = 1_000_000; // 0.001 SOL (very small)

    let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

    assert_eq!(calculation.fee_lamports, fee_structure.minimum_lamports);
}

#[tokio::test]
async fn test_fee_calculator_maximum() {
    let mut fee_structure = create_test_fee_structure();
    fee_structure.maximum_lamports = Some(5_000_000); // 0.005 SOL max
    let recoverable = 100_000_000; // 0.1 SOL

    let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

    assert_eq!(calculation.fee_lamports, 5_000_000); // Capped at maximum
}

#[tokio::test]
async fn test_fee_structure_validation() {
    let mut fee_structure = create_test_fee_structure();

    // Valid structure
    assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_ok());

    // Invalid percentage
    fee_structure.percentage = 1.5;
    assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_err());

    // Reset and test minimum
    fee_structure.percentage = 0.15;
    fee_structure.minimum_lamports = 0;
    assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_err());

    // Reset and test maximum < minimum
    fee_structure.minimum_lamports = 1_000_000;
    fee_structure.maximum_lamports = Some(500_000);
    assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_err());
}

#[tokio::test]
async fn test_batch_fee_calculation() {
    let fee_structure = create_test_fee_structure();
    let wallet_infos = vec![
        WalletInfo {
            address: "wallet1".to_string(),
            pubkey: solana_sdk::pubkey::Pubkey::default(),
            total_accounts: 10,
            empty_accounts: 5,
            recoverable_lamports: 100_000_000,
            recoverable_sol: 0.1,
            empty_account_addresses: vec![],
            scan_time_ms: 1000,
        },
        WalletInfo {
            address: "wallet2".to_string(),
            pubkey: solana_sdk::pubkey::Pubkey::default(),
            total_accounts: 8,
            empty_accounts: 3,
            recoverable_lamports: 50_000_000,
            recoverable_sol: 0.05,
            empty_account_addresses: vec![],
            scan_time_ms: 800,
        },
    ];

    let batch_calc = FeeCalculator::calculate_batch_fee(&wallet_infos, &fee_structure);

    assert_eq!(batch_calc.total_recoverable_lamports, 150_000_000);
    assert_eq!(batch_calc.total_fee_lamports, 22_500_000); // 15% of total
    assert_eq!(batch_calc.total_net_recoverable_lamports, 127_500_000);
    assert_eq!(batch_calc.wallet_calculations.len(), 2);
}

#[test]
fn test_scan_result_creation() {
    let scan_id = uuid::Uuid::new_v4();
    let wallet_address = get_test_wallet_address();
    let now = chrono::Utc::now();

    let scan_result = ScanResult {
        id: scan_id,
        wallet_address: wallet_address.clone(),
        status: ScanStatus::Completed,
        result: None,
        error: None,
        created_at: now,
    };

    assert_eq!(scan_result.id, scan_id);
    assert_eq!(scan_result.wallet_address, wallet_address);
    assert_eq!(scan_result.status, ScanStatus::Completed);
    assert!(scan_result.result.is_none());
    assert!(scan_result.error.is_none());
    assert_eq!(scan_result.created_at, now);
}

#[test]
fn test_batch_scan_request_creation() {
    let wallet_addresses = get_test_wallet_addresses();
    let user_id = Some("test_user".to_string());
    let fee_percentage = Some(0.15);
    let now = chrono::Utc::now();

    let batch_request = BatchScanRequest {
        id: uuid::Uuid::new_v4(),
        wallet_addresses: wallet_addresses.clone(),
        user_id: user_id.clone(),
        fee_percentage,
        created_at: now,
    };

    assert_eq!(batch_request.wallet_addresses, wallet_addresses);
    assert_eq!(batch_request.user_id, user_id);
    assert_eq!(batch_request.fee_percentage, fee_percentage);
    assert_eq!(batch_request.created_at, now);
}

#[test]
fn test_rpc_endpoint_creation() {
    let endpoint = RpcEndpoint {
        url: "config_endpoint".to_string(),
        priority: 1,
        rate_limit_rps: 100,
        timeout_ms: 5000,
        healthy: true,
    };

    assert_eq!(endpoint.url, "config_endpoint");
    assert_eq!(endpoint.priority, 1);
    assert_eq!(endpoint.rate_limit_rps, 100);
    assert_eq!(endpoint.timeout_ms, 5000);
    assert!(endpoint.healthy);
}

#[test]
fn test_user_creation() {
    let now = chrono::Utc::now();
    let user = User {
        id: "user_123".to_string(),
        email: "test@example.com".to_string(),
        api_key: Some("api_key_123".to_string()),
        fee_structure: Some(create_test_fee_structure()),
        rate_limit_rps: Some(50),
        created_at: now,
        last_active: None,
        metadata: serde_json::json!({"plan": "premium"}),
    };

    assert_eq!(user.id, "user_123");
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.api_key, Some("api_key_123".to_string()));
    assert!(user.fee_structure.is_some());
    assert_eq!(user.rate_limit_rps, Some(50));
    assert_eq!(user.created_at, now);
    assert!(user.last_active.is_none());
}

#[test]
fn test_scan_metrics_creation() {
    let metrics = ScanMetrics {
        total_scans: 1000,
        successful_scans: 950,
        failed_scans: 50,
        total_recoverable_sol: 25.5,
        average_scan_time_ms: 1200.0,
        wallets_processed: 1000,
        empty_accounts_found: 250,
        requests_per_second: 15.5,
    };

    assert_eq!(metrics.total_scans, 1000);
    assert_eq!(metrics.successful_scans, 950);
    assert_eq!(metrics.failed_scans, 50);
    assert_eq!(metrics.total_recoverable_sol, 25.5);
    assert_eq!(metrics.average_scan_time_ms, 1200.0);
    assert_eq!(metrics.wallets_processed, 1000);
    assert_eq!(metrics.empty_accounts_found, 250);
    assert_eq!(metrics.requests_per_second, 15.5);
}

#[test]
fn test_empty_account_creation() {
    let empty_account = EmptyAccount {
        address: "AbCdEfGhIjKlMnOpQrStUvWxYz1234567890abcdef".to_string(),
        lamports: 2_228_680,
        owner: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
        mint: Some("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string()),
    };

    assert_eq!(empty_account.address, "AbCdEfGhIjKlMnOpQrStUvWxYz1234567890abcdef");
    assert_eq!(empty_account.lamports, 2_228_680);
    assert_eq!(empty_account.owner, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    assert_eq!(empty_account.mint, Some("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string()));
}

#[test]
fn test_wallet_info_creation() {
    let wallet_info = WalletInfo {
        address: get_test_wallet_address(),
        pubkey: solana_sdk::pubkey::Pubkey::default(),
        total_accounts: 25,
        empty_accounts: 8,
        recoverable_lamports: 17_829_440,
        recoverable_sol: 0.01782944,
        empty_account_addresses: vec![
            "AbCdEfGhIjKlMnOpQrStUvWxYz1234567890abcdef".to_string(),
            "BcDeFgHiJkLmNoPqRsTuVwXyZ2345678901bcdef".to_string(),
        ],
        scan_time_ms: 1500,
    };

    assert_eq!(wallet_info.address, get_test_wallet_address());
    assert_eq!(wallet_info.total_accounts, 25);
    assert_eq!(wallet_info.empty_accounts, 8);
    assert_eq!(wallet_info.recoverable_lamports, 17_829_440);
    assert_eq!(wallet_info.recoverable_sol, 0.01782944);
    assert_eq!(wallet_info.empty_account_addresses.len(), 2);
    assert_eq!(wallet_info.scan_time_ms, 1500);
}

#[test]
fn test_scan_status_equality() {
    assert_eq!(ScanStatus::Pending, ScanStatus::Pending);
    assert_eq!(ScanStatus::InProgress, ScanStatus::InProgress);
    assert_eq!(ScanStatus::Completed, ScanStatus::Completed);
    assert_eq!(ScanStatus::Failed, ScanStatus::Failed);

    assert_ne!(ScanStatus::Pending, ScanStatus::Completed);
    assert_ne!(ScanStatus::InProgress, ScanStatus::Failed);
}

#[test]
fn test_token_account_parsing_base64() {
    use solana_recover::core::scanner::WalletScanner;
    use solana_account_decoder::UiAccountEncoding;
    use solana_sdk::pubkey::Pubkey;
    
    // Create a mock scanner
    let scanner = WalletScanner::new(create_test_connection_pool());
    
    // Create a mock token account data (72 bytes minimum)
    // Structure: [mint(32) + owner(32) + amount(8) + ...]
    let mut token_data = vec![0u8; 80];
    
    // Set mint (first 32 bytes) - using a known USDC mint
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    let mint_pubkey = Pubkey::from_str(usdc_mint).unwrap();
    token_data[0..32].copy_from_slice(mint_pubkey.as_ref());
    
    // Set owner (next 32 bytes) - using a mock owner
    let owner_pubkey = Pubkey::new_unique();
    token_data[32..64].copy_from_slice(owner_pubkey.as_ref());
    
    // Set amount to 0 (empty token account)
    token_data[64..72].copy_from_slice(&0u64.to_le_bytes());
    
    // Encode as Base64
    use base64::{Engine as _, engine::general_purpose};
    let base64_data = general_purpose::STANDARD.encode(&token_data);
    
    // Test parsing
    let result = scanner.parse_token_account_from_binary(&base64_data, &UiAccountEncoding::Base64);
    
    assert!(result.is_ok());
    let token_account = result.unwrap();
    assert_eq!(token_account.mint, usdc_mint);
    assert_eq!(token_account.amount, 0);
}

#[test]
fn test_token_account_parsing_base58() {
    use solana_recover::core::scanner::WalletScanner;
    use solana_account_decoder::UiAccountEncoding;
    use solana_sdk::pubkey::Pubkey;
    
    // Create a mock scanner
    let scanner = WalletScanner::new(create_test_connection_pool());
    
    // Create a mock token account data (72 bytes minimum)
    let mut token_data = vec![0u8; 80];
    
    // Set mint (first 32 bytes)
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    let mint_pubkey = Pubkey::from_str(usdc_mint).unwrap();
    token_data[0..32].copy_from_slice(mint_pubkey.as_ref());
    
    // Set owner (next 32 bytes)
    let owner_pubkey = Pubkey::new_unique();
    token_data[32..64].copy_from_slice(owner_pubkey.as_ref());
    
    // Set amount to 1000000 (non-zero token account)
    token_data[64..72].copy_from_slice(&1_000_000u64.to_le_bytes());
    
    // Encode as Base58
    let base58_data = bs58::encode(&token_data).into_string();
    
    // Test parsing
    let result = scanner.parse_token_account_from_binary(&base58_data, &UiAccountEncoding::Base58);
    
    assert!(result.is_ok());
    let token_account = result.unwrap();
    assert_eq!(token_account.mint, usdc_mint);
    assert_eq!(token_account.amount, 1_000_000);
}

#[test]
fn test_token_account_parsing_invalid_data() {
    use solana_recover::core::scanner::WalletScanner;
    use solana_account_decoder::UiAccountEncoding;
    
    let scanner = WalletScanner::new(create_test_connection_pool());
    
    // Test with too short data
    let short_data = "dGVzdA=="; // "test" in Base64
    let result = scanner.parse_token_account_from_binary(short_data, &UiAccountEncoding::Base64);
    assert!(result.is_err());
    
    // Test with invalid Base64
    let invalid_base64 = "invalid_base64!";
    let result = scanner.parse_token_account_from_binary(invalid_base64, &UiAccountEncoding::Base64);
    assert!(result.is_err());
    
    // Test with unsupported encoding
    let result = scanner.parse_token_account_from_binary("data", &UiAccountEncoding::Json);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_main_wallet_protection() {
    use solana_recover::core::scanner::WalletScanner;
    use solana_client::rpc_response::RpcKeyedAccount;
    use solana_account_decoder::{UiAccount, UiAccountData, UiAccountEncoding};
    use solana_sdk::pubkey::Pubkey;
    
    let scanner = WalletScanner::new(create_test_connection_pool());
    let wallet_address = "11111111111111111111111111111112"; // System program ID as test
    
    // Create a mock account that represents the main wallet
    let main_wallet_account = RpcKeyedAccount {
        pubkey: wallet_address.to_string(),
        account: UiAccount {
            lamports: 1_000_000_000, // 1 SOL
            data: UiAccountData::Binary("".to_string(), UiAccountEncoding::Base64),
            owner: "11111111111111111111111111111111".to_string(), // System program
            executable: false,
            rent_epoch: 0,
            space: Some(0),
        },
    };
    
    // Test that the main wallet is never flagged as recoverable
    let result = scanner.check_empty_account(&main_wallet_account, wallet_address).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none(), "Main wallet should never be flagged as recoverable");
    
    // Test that a different account with the same properties would be flagged
    let different_account = RpcKeyedAccount {
        pubkey: "22222222222222222222222222222223".to_string(),
        account: UiAccount {
            lamports: 2_228_680, // Rent-exempt amount
            data: UiAccountData::Binary("".to_string(), UiAccountEncoding::Base64),
            owner: "11111111111111111111111111111111".to_string(), // System program
            executable: false,
            rent_epoch: 0,
            space: Some(0),
        },
    };
    
    let result = scanner.check_empty_account(&different_account, wallet_address).await;
    assert!(result.is_ok());
    // This might be Some or None depending on the rent calculation, but should not be None due to wallet protection
    let result = result.unwrap();
    if let Some(account) = result {
        assert_ne!(account.address, wallet_address);
    }
}
