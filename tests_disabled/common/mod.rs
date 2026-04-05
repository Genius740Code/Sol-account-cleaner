use solana_recover::{
    WalletScanner, ConnectionPool, BatchProcessor, Config, RpcEndpoint,
    WalletCredentials, WalletCredentialData, WalletType, FeeStructure,
};
use std::sync::Arc;

pub fn create_test_config() -> Config {
    Config {
        server: solana_recover::ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            workers: 4,
        },
        rpc: solana_recover::RpcConfig {
            endpoints: vec!["config_endpoint".to_string()],
            pool_size: 5,
            timeout_ms: 5000,
            rate_limit_rps: 100,
        },
        scanner: solana_recover::ScannerConfig {
            batch_size: 10,
            max_concurrent_wallets: 50,
            retry_attempts: 2,
            retry_delay_ms: 500,
        },
        cache: solana_recover::ConfigCacheConfig {
            ttl_seconds: 300,
            max_size: 1000,
        },
        turnkey: solana_recover::TurnkeyConfig {
            api_url: "https://api.turnkey.com".to_string(),
            timeout_ms: 10000,
        },
        logging: solana_recover::LoggingConfig {
            level: "debug".to_string(),
            format: solana_recover::utils::LogFormat::Pretty,
            output: solana_recover::utils::LogOutput::Stdout,
            file_path: None,
            json_fields: vec![],
        },
        database: solana_recover::ConfigDatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 5,
        },
    }
}

pub fn create_test_wallet_scanner() -> Arc<WalletScanner> {
    let config = create_test_config();
    let rpc_endpoints: Vec<RpcEndpoint> = config.rpc.endpoints
        .iter()
        .enumerate()
        .map(|(i, url)| RpcEndpoint {
            url: url.clone(),
            priority: i as u8,
            rate_limit_rps: config.rpc.rate_limit_rps,
            timeout_ms: config.rpc.timeout_ms,
            healthy: true,
        })
        .collect();
    
    let connection_pool = Arc::new(ConnectionPool::new(rpc_endpoints, config.rpc.pool_size));
    Arc::new(WalletScanner::new(connection_pool))
}

pub fn create_test_batch_processor() -> Arc<BatchProcessor> {
    let scanner = create_test_wallet_scanner();
    let config = create_test_config();
    
    Arc::new(BatchProcessor::new(
        scanner,
        None, // No cache for tests
        None, // No persistence for tests
        config.scanner.into(),
    ))
}

pub fn create_test_wallet_credentials(wallet_type: WalletType) -> WalletCredentials {
    match wallet_type {
        WalletType::Turnkey => WalletCredentials {
            wallet_type,
            credentials: WalletCredentialData::Turnkey {
                api_key: "test_api_key".to_string(),
                organization_id: "test_org".to_string(),
                private_key_id: "test_key".to_string(),
            },
        },
        WalletType::Phantom => WalletCredentials {
            wallet_type,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test_encrypted_key".to_string(),
            },
        },
        WalletType::Solflare => WalletCredentials {
            wallet_type,
            credentials: WalletCredentialData::Solflare {
                public_key: "11111111111111111111111111111111112".to_string(),
            },
        },
        WalletType::PrivateKey => WalletCredentials {
            wallet_type,
            credentials: WalletCredentialData::PrivateKey {
                private_key: "test_private_key".to_string(),
            },
        },
    }
}

pub fn create_test_fee_structure() -> FeeStructure {
    FeeStructure {
        percentage: 0.15,
        minimum_lamports: 1_000_000,
        maximum_lamports: Some(10_000_000),
        waive_below_lamports: Some(5_000_000),
    }
}

pub fn get_test_wallet_address() -> String {
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string()
}

pub fn get_test_wallet_addresses() -> Vec<String> {
    vec![
        "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(),
    ]
}
