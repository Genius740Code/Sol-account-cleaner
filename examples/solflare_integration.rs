//! Solflare Wallet Integration Example
//! 
//! This example demonstrates how to integrate with Solflare wallet
//! supporting both browser extension and mobile app connections.

use solana_recover::wallet::*;
use solana_recover::utils::{LoggingConfig, Logger};
use std::sync::Arc;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let logging_config = LoggingConfig {
        level: "info".to_string(),
        format: solana_recover::utils::LogFormat::Pretty,
        output: solana_recover::utils::LogOutput::Stdout,
        file_path: None,
        json_fields: vec![],
    };
    
    Logger::init(logging_config).expect("Failed to initialize logger");
    
    info!("☀️ Starting Solflare Wallet Integration Example");
    
    // Create wallet manager with Solflare-optimized configuration
    let config = WalletManagerConfig {
        enable_turnkey: false,
        enable_phantom: false,
        enable_solflare: true,
        enable_private_key: false,
        solflare_timeout_ms: 20000, // Longer timeout for mobile
        solflare_retry_attempts: 5,
        enable_solflare_mobile: true,
        enable_solflare_web: true,
        max_connections: 5,
        connection_timeout_seconds: 600, // 10 minutes for mobile
    };
    
    let wallet_manager = Arc::new(WalletManager::with_config(config));
    
    // Demonstrate Solflare wallet integration
    demo_solflare_detection(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    demo_solflare_connection(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    demo_solflare_mobile_integration(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    demo_solflare_transaction_signing(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    demo_solflare_multi_wallet_support(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    
    info!("✅ Solflare Wallet Integration Example completed!");
    Ok(())
}

async fn demo_solflare_detection(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n🔍 === Solflare Detection Demo ===");
    
    let supported_wallets = wallet_manager.get_supported_wallets().await;
    
    if supported_wallets.contains(&WalletType::Solflare) {
        info!("✅ Solflare wallet provider is available");
        
        // Get connection metrics to see configuration
        let metrics = wallet_manager.get_connection_metrics().await;
        info!("📊 Solflare Configuration:");
        info!("   Enabled: {}", metrics["config"]["enable_solflare"]);
        info!("   Mobile Support: {}", metrics["config"]["enable_solflare_mobile"]);
        info!("   Web Support: {}", metrics["config"]["enable_solflare_web"]);
        info!("   Timeout: {}ms", metrics["config"]["solflare_timeout_ms"]);
        info!("   Retry Attempts: {}", metrics["config"]["solflare_retry_attempts"]);
    } else {
        warn!("⚠️  Solflare wallet provider is not enabled");
    }
    
    Ok(())
}

async fn demo_solflare_connection(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n🔗 === Solflare Connection Demo ===");
    
    // Create Solflare credentials
    let solflare_credentials = WalletCredentials {
        wallet_type: WalletType::Solflare,
        credentials: WalletCredentialData::Solflare {
            public_key: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        },
    };
    
    info!("📡 Attempting to connect to Solflare wallet...");
    info!("   This will try: Browser Extension → Mobile App → Web Wallet");
    
    match wallet_manager.connect_wallet(solflare_credentials).await {
        Ok(connection) => {
            info!("✅ Successfully connected to Solflare wallet");
            info!("   Connection ID: {}", connection.id);
            info!("   Wallet Type: {:?}", connection.wallet_type);
            info!("   Created at: {}", connection.created_at);
            
            // Get and display wallet information
            if let Some(wallet_info) = wallet_manager.get_wallet_info(&connection.id).await {
                info!("📋 Wallet Information:");
                info!("   Public Key: {}", wallet_info.public_key);
                info!("   Label: {:?}", wallet_info.label);
                info!("   Last Used: {:?}", wallet_info.last_used);
            }
            
            // Validate the connection
            match wallet_manager.validate_connection(&connection.id).await {
                Ok(is_valid) => {
                    info!("🔍 Connection validation: {}", if is_valid { "✅ Valid" } else { "❌ Invalid" });
                }
                Err(e) => {
                    warn!("⚠️  Connection validation failed: {}", e);
                }
            }
            
            // Keep connection open for other demos
            info!("💡 Connection kept open for subsequent demos");
            Ok(())
        }
        Err(e) => {
            error!("❌ Failed to connect to Solflare wallet: {}", e);
            info!("💡 Troubleshooting tips:");
            info!("   1. Ensure Solflare extension is installed (Chrome/Firefox)");
            info!("   2. Make sure Solflare mobile app is installed (iOS/Android)");
            info!("   3. Check that your site has permission to access Solflare");
            info!("   4. Ensure you're using HTTPS (required for wallet communication)");
            info!("   5. Try refreshing the page and reconnecting");
            Err(e.into())
        }
    }
}

async fn demo_solflare_mobile_integration(_wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n📱 === Solflare Mobile Integration Demo ===");
    
    // This demonstrates how to handle mobile-specific Solflare integration
    info!("📲 Mobile Integration Features:");
    info!("   ✅ Deep link support for mobile app");
    info!("   ✅ Extended timeout for mobile network conditions");
    info!("   ✅ Retry logic for mobile connectivity");
    info!("   ✅ Fallback to web wallet if app not available");
    
    // Simulate mobile deep link creation
    let public_key = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    let deep_link = create_solflare_deep_link(public_key);
    
    info!("🔗 Generated Deep Link:");
    info!("   {}", deep_link);
    
    info!("💡 Mobile Integration Tips:");
    info!("   1. Use deep links to launch Solflare app");
    info!("   2. Implement proper callback handling");
    info!("   3. Handle app switching gracefully");
    info!("   4. Provide fallback to web wallet");
    
    Ok(())
}

async fn demo_solflare_transaction_signing(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n✍️ === Solflare Transaction Signing Demo ===");
    
    let active_connections = wallet_manager.list_active_connections();
    
    if active_connections.is_empty() {
        warn!("⚠️  No active Solflare connections found. Please run connection demo first.");
        return Ok(());
    }
    
    let connection = &active_connections[0];
    info!("📝 Using Solflare connection: {}", connection.id);
    
    // Create different types of transactions for comprehensive testing
    let transactions = vec![
        ("SOL Transfer", create_sol_transfer_transaction()),
        ("SPL Token Transfer", create_spl_token_transaction()),
        ("Staking Instruction", create_staking_transaction()),
        ("NFT Transfer", create_nft_transfer_transaction()),
    ];
    
    for (tx_type, transaction) in transactions {
        info!("\n📄 Signing {} transaction", tx_type);
        
        match wallet_manager.sign_with_wallet(&connection.id, &transaction).await {
            Ok(signature) => {
                info!("✅ Transaction signed successfully");
                info!("   Transaction type: {}", tx_type);
                info!("   Signature length: {} bytes", signature.len());
                
                // Display signature preview (first 16 bytes)
                let signature_preview = &signature[..signature.len().min(16)];
                info!("   Signature preview: {:02x?}", signature_preview);
                
                // Validate signature format
                if signature.len() == 64 {
                    info!("   ✅ Valid signature length (64 bytes)");
                } else {
                    warn!("   ⚠️  Unexpected signature length: {} bytes", signature.len());
                }
            }
            Err(e) => {
                warn!("⚠️  Failed to sign {} transaction: {}", tx_type, e);
                
                // Provide specific error handling advice
                if e.to_string().contains("user rejected") {
                    info!("💡 User rejected the transaction in Solflare");
                } else if e.to_string().contains("insufficient") {
                    info!("💡 Insufficient funds for this transaction");
                } else if e.to_string().contains("network") {
                    info!("💡 Network error - please check your connection");
                }
            }
        }
        
        // Add delay between transactions to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    Ok(())
}

async fn demo_solflare_multi_wallet_support(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n👥 === Solflare Multi-Wallet Support Demo ===");
    
    let active_connections = wallet_manager.list_active_connections();
    
    if active_connections.is_empty() {
        warn!("⚠️  No active Solflare connections found");
        return Ok(());
    }
    
    let connection = &active_connections[0];
    info!("🔄 Demonstrating multi-wallet operations with: {}", connection.id);
    
    // Create multiple transactions for batch processing
    let batch_transactions: Vec<Vec<u8>> = vec![
        create_sol_transfer_transaction(),
        create_spl_token_transaction(),
        create_staking_transaction(),
        create_nft_transfer_transaction(),
    ];
    
    info!("📦 Created {} transactions for batch processing", batch_transactions.len());
    
    // Perform batch signing
    match wallet_manager.batch_sign_transactions(&connection.id, &batch_transactions).await {
        Ok(results) => {
            info!("✅ Batch signing completed");
            info!("   Total transactions: {}", results.len());
            
            let mut successful_count = 0;
            let mut failed_count = 0;
            
            for (i, result) in results.iter().enumerate() {
                let tx_types = ["SOL Transfer", "SPL Token", "Staking", "NFT Transfer"];
                
                match result {
                    Ok(signature) => {
                        successful_count += 1;
                        info!("   {}: ✅ {} ({} bytes)", 
                               tx_types[i], tx_types[i], signature.len());
                    }
                    Err(e) => {
                        failed_count += 1;
                        warn!("   {}: ❌ {} - {}", tx_types[i], tx_types[i], e);
                    }
                }
            }
            
            info!("📊 Batch Results:");
            info!("   Successful: {}", successful_count);
            info!("   Failed: {}", failed_count);
            info!("   Success Rate: {:.1}%", (successful_count as f64 / results.len() as f64) * 100.0);
        }
        Err(e) => {
            warn!("⚠️  Batch signing failed: {}", e);
        }
    }
    
    // Demonstrate connection monitoring
    info!("\n📊 Connection Monitoring:");
    let metrics = wallet_manager.get_connection_metrics().await;
    info!("   Active Connections: {}", metrics["total_connections"]);
    info!("   Max Connections: {}", metrics["max_connections"]);
    
    // Validate connection health
    match wallet_manager.validate_connection(&connection.id).await {
        Ok(is_valid) => {
            info!("   Connection Health: {}", if is_valid { "✅ Healthy" } else { "❌ Unhealthy" });
        }
        Err(e) => {
            warn!("   Health check failed: {}", e);
        }
    }
    
    Ok(())
}

// Helper functions
fn create_solflare_deep_link(public_key: &str) -> String {
    format!(
        "solflare://connect?publicKey={}&dapp={}&callback={}",
        urlencoding::encode(public_key),
        urlencoding::encode("solana-recover-demo"),
        urlencoding::encode("https://your-app.com/solflare-callback")
    )
}

fn create_sol_transfer_transaction() -> Vec<u8> {
    // SOL transfer transaction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        1, 0, 0, 0, // Program ID index (System program)
        2, 0, 0, 0, // Accounts count
        8, 0, 0, 0, // Data length
        2, 0, 0, 0, // Transfer instruction
        0, 0, 0, 0, // Lamports (simplified)
        0, 0, 0, 0, // Recipient (simplified)
    ]
}

fn create_spl_token_transaction() -> Vec<u8> {
    // SPL token transfer transaction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        2, 0, 0, 0, // Program ID index (Token program)
        3, 0, 0, 0, // Accounts count
        9, 0, 0, 0, // Data length
        3, 0, 0, 0, // Transfer instruction
        0, 0, 0, 0, // Amount (simplified)
        0, 0, 0, 0, // Token account (simplified)
        0, 0, 0, 0, // Owner (simplified)
    ]
}

fn create_staking_transaction() -> Vec<u8> {
    // Staking transaction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        3, 0, 0, 0, // Program ID index (Stake program)
        2, 0, 0, 0, // Accounts count
        4, 0, 0, 0, // Data length
        2, 0, 0, 0, // Delegate instruction
        0, 0, 0, 0, // Stake account (simplified)
        0, 0, 0, 0, // Vote account (simplified)
    ]
}

fn create_nft_transfer_transaction() -> Vec<u8> {
    // NFT transfer transaction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        4, 0, 0, 0, // Program ID index (Metaplex)
        4, 0, 0, 0, // Accounts count
        1, 0, 0, 0, // Data length
        12, 0, 0, 0, // Transfer instruction
        0, 0, 0, 0, // NFT data (simplified)
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solflare_deep_link_creation() {
        let public_key = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let deep_link = create_solflare_deep_link(public_key);
        
        assert!(deep_link.starts_with("solflare://"));
        assert!(deep_link.contains("publicKey="));
        assert!(deep_link.contains("dapp="));
        assert!(deep_link.contains("callback="));
    }

    #[test]
    fn test_transaction_creation() {
        let sol_tx = create_sol_transfer_transaction();
        let spl_tx = create_spl_token_transaction();
        let stake_tx = create_staking_transaction();
        let nft_tx = create_nft_transfer_transaction();
        
        // Verify all transactions have different structures
        assert_ne!(sol_tx, spl_tx);
        assert_ne!(spl_tx, stake_tx);
        assert_ne!(stake_tx, nft_tx);
        
        // Verify minimum transaction size
        assert!(sol_tx.len() > 10);
        assert!(spl_tx.len() > 10);
        assert!(stake_tx.len() > 10);
        assert!(nft_tx.len() > 10);
    }

    #[tokio::test]
    async fn test_solflare_credentials_structure() {
        let credentials = WalletCredentials {
            wallet_type: WalletType::Solflare,
            credentials: WalletCredentialData::Solflare {
                public_key: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            },
        };
        
        assert!(matches!(credentials.wallet_type, WalletType::Solflare));
        if let WalletCredentialData::Solflare { public_key } = credentials.credentials {
            assert_eq!(public_key, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        } else {
            panic!("Expected Solflare credential data");
        }
    }
}
