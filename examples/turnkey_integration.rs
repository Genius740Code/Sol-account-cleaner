//! Turnkey wallet integration example
//! 
//! This example demonstrates how to use the Solana Recover library
//! with Turnkey wallet integration for enterprise-grade key management.

use solana_recover::*;
use solana_recover::wallet::*;
use std::sync::Arc;
use tracing::{info, error, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let logging_config = LoggingConfig {
        level: "info".to_string(),
        format: solana_recover::utils::LogFormat::Pretty,
        output: solana_recover::utils::LogOutput::Stdout,
        file_path: None,
        json_fields: vec![],
    };
    
    Logger::init(logging_config)?;
    
    info!("Starting Turnkey integration example");
    
    // Create wallet manager
    let wallet_manager = Arc::new(WalletManager::new());
    
    // Example Turnkey credentials (in production, these would come from secure storage)
    let turnkey_credentials = WalletCredentials {
        wallet_type: WalletType::Turnkey,
        credentials: WalletCredentialData::Turnkey {
            api_key: "your_turnkey_api_key".to_string(),
            organization_id: "your_organization_id".to_string(),
            private_key_id: "your_private_key_id".to_string(),
        },
    };
    
    // Connect to Turnkey wallet
    info!("Connecting to Turnkey wallet...");
    match wallet_manager.connect_wallet(turnkey_credentials).await {
        Ok(connection) => {
            info!("Successfully connected to Turnkey wallet");
            info!("Connection ID: {}", connection.id);
            info!("Connected at: {}", connection.created_at);
            
            // Get public key from Turnkey
            let turnkey_provider = crate::wallet::turnkey::TurnkeyProvider::new();
            match turnkey_provider.get_public_key(&connection).await {
                Ok(public_key) => {
                    info!("Retrieved public key from Turnkey: {}", public_key);
                    
                    // Create scanner and scan the wallet
                    let config = Config::load()?;
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
                    let scanner = Arc::new(WalletScanner::new(connection_pool));
                    
                    info!("Scanning Turnkey wallet: {}", public_key);
                    match scanner.scan_wallet(&public_key).await {
                        Ok(scan_result) => {
                            info!("Scan completed for Turnkey wallet");
                            info!("Scan ID: {}", scan_result.id);
                            info!("Status: {:?}", scan_result.status);
                            
                            if let Some(wallet_info) = scan_result.result {
                                info!("Total accounts: {}", wallet_info.total_accounts);
                                info!("Empty accounts: {}", wallet_info.empty_accounts);
                                info!("Recoverable SOL: {:.9}", wallet_info.recoverable_sol);
                                
                                // Calculate enterprise fee structure
                                let enterprise_fee_structure = FeeStructure {
                                    percentage: 0.10, // 10% enterprise rate
                                    minimum_lamports: 500_000, // Lower minimum for enterprise
                                    maximum_lamports: Some(50_000_000), // Higher maximum
                                    waive_below_lamports: Some(1_000_000), // Higher waiver threshold
                                };
                                
                                let fee_calculation = FeeCalculator::calculate_wallet_fee(&wallet_info, &enterprise_fee_structure);
                                
                                info!("Enterprise fee calculation:");
                                info!("  Fee percentage: {:.1}%", enterprise_fee_structure.percentage * 100.0);
                                info!("  Fee amount: {:.9} SOL", fee_calculation.fee_lamports as f64 / 1_000_000_000.0);
                                info!("  Net recoverable: {:.9} SOL", fee_calculation.net_recoverable_lamports as f64 / 1_000_000_000.0);
                                info!("  Fee waived: {}", fee_calculation.fee_waived);
                                
                                // Example: Sign a recovery transaction using Turnkey
                                if wallet_info.recoverable_lamports > 0 {
                                    info!("Demonstrating transaction signing with Turnkey...");
                                    
                                    // Create a dummy recovery transaction (in reality, this would be a proper Solana transaction)
                                    let dummy_transaction = vec![
                                        1, 0, 0, 0, // Version
                                        2, 0, 0, 0, // Number of signatures required
                                        // ... rest of transaction data would go here
                                    ];
                                    
                                    match turnkey_provider.sign_transaction(&connection, &dummy_transaction).await {
                                        Ok(signature) => {
                                            info!("Successfully signed transaction with Turnkey");
                                            info!("Signature: {}", hex::encode(&signature));
                                        }
                                        Err(e) => {
                                            warn!("Failed to sign transaction with Turnkey: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to scan Turnkey wallet: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get public key from Turnkey: {}", e);
                }
            }
            
            // Demonstrate multiple wallet management
            info!("\n=== Multiple Wallet Management Demo ===");
            
            // Create additional Turnkey credentials for different users
            let user_credentials = vec![
                ("user_1", "api_key_1", "org_1", "key_1"),
                ("user_2", "api_key_2", "org_2", "key_2"),
            ];
            
            let mut connections = vec![];
            
            for (user_id, api_key, org_id, key_id) in user_credentials {
                let credentials = WalletCredentials {
                    wallet_type: WalletType::Turnkey,
                    credentials: WalletCredentialData::Turnkey {
                        api_key: api_key.to_string(),
                        organization_id: org_id.to_string(),
                        private_key_id: key_id.to_string(),
                    },
                };
                
                match wallet_manager.connect_wallet(credentials).await {
                    Ok(conn) => {
                        info!("Connected wallet for user: {}", user_id);
                        connections.push((user_id, conn));
                    }
                    Err(e) => {
                        warn!("Failed to connect wallet for user {}: {}", user_id, e);
                    }
                }
            }
            
            // List all active connections
            let active_connections = wallet_manager.list_active_connections();
            info!("Total active connections: {}", active_connections.len());
            
            for conn in active_connections {
                info!("Connection: {} ({})", conn.id, conn.wallet_type);
            }
            
            // Disconnect wallets
            info!("Disconnecting Turnkey wallets...");
            for (user_id, connection) in connections {
                match wallet_manager.disconnect_wallet(&connection.id).await {
                    Ok(_) => {
                        info!("Disconnected wallet for user: {}", user_id);
                    }
                    Err(e) => {
                        error!("Failed to disconnect wallet for user {}: {}", user_id, e);
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to connect to Turnkey wallet: {}", e);
            
            // Demonstrate error handling and fallback
            warn!("This is expected in demo environment without valid Turnkey credentials");
            info!("In production, ensure:");
            info!("1. Valid Turnkey API key");
            info!("2. Correct organization ID");
            info!("3. Valid private key ID");
            info!("4. Network connectivity to Turnkey API");
        }
    }
    
    // Demonstrate Turnkey configuration
    info!("\n=== Turnkey Configuration Example ===");
    
    let turnkey_config = TurnkeyConfig {
        api_url: "https://api.turnkey.com".to_string(),
        timeout_ms: 15000, // 15 seconds timeout for enterprise
    };
    
    info!("Turnkey API URL: {}", turnkey_config.api_url);
    info!("Turnkey timeout: {}ms", turnkey_config.timeout_ms);
    
    // Security best practices for Turnkey
    info!("\n=== Turnkey Security Best Practices ===");
    info!("1. Store API keys in secure environment variables");
    info!("2. Use short-lived API tokens when possible");
    info!("3. Implement proper access controls");
    info!("4. Monitor API usage and anomalies");
    info!("5. Regularly rotate API keys");
    info!("6. Use IP whitelisting for API access");
    info!("7. Enable audit logging for all operations");
    
    info!("Turnkey integration example completed");
    Ok(())
}
