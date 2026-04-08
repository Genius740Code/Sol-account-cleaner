//! Comprehensive Wallet Integration Demo
//! 
//! This example demonstrates all supported wallet providers:
//! - Turnkey (Enterprise-grade API-based wallet)
//! - Phantom (Browser extension wallet)
//! - Solflare (Browser/mobile wallet)
//! - PrivateKey (Direct key management)

use solana_recover::*;
use solana_recover::wallet::*;
use std::sync::Arc;
use tracing::{info, warn, error};

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
    
    info!("🚀 Starting Comprehensive Wallet Integration Demo");
    
    // Create wallet manager with custom configuration
    let config = WalletManagerConfig {
        enable_turnkey: true,
        enable_phantom: true,
        enable_solflare: true,
        enable_private_key: true,
        solflare_timeout_ms: 15000,
        solflare_retry_attempts: 3,
        enable_solflare_mobile: true,
        enable_solflare_web: true,
        max_connections: 10,
        connection_timeout_seconds: 300,
    };
    
    let wallet_manager = Arc::new(WalletManager::with_config(config));
    
    // Display supported wallets
    let supported_wallets = wallet_manager.get_supported_wallets().await;
    info!("📋 Supported wallet types: {:?}", supported_wallets);
    
    // Demo each wallet type
    demo_turnkey_wallet(&wallet_manager).await?;
    demo_phantom_wallet(&wallet_manager).await?;
    demo_solflare_wallet(&wallet_manager).await?;
    demo_private_key_wallet(&wallet_manager).await?;
    
    // Demo wallet manager features
    demo_wallet_manager_features(&wallet_manager).await?;
    
    info!("✅ Comprehensive Wallet Integration Demo completed successfully!");
    Ok(())
}

async fn demo_turnkey_wallet(wallet_manager: &Arc<WalletManager>) -> Result<(), Box<dyn std::error::Error>> {
    info!("\n🔐 === Turnkey Wallet Demo ===");
    
    let turnkey_credentials = WalletCredentials {
        wallet_type: WalletType::Turnkey,
        credentials: WalletCredentialData::Turnkey {
            api_key: "demo_turnkey_api_key".to_string(),
            organization_id: "demo_organization_id".to_string(),
            private_key_id: "demo_private_key_id".to_string(),
        },
    };
    
    match wallet_manager.connect_wallet(turnkey_credentials).await {
        Ok(connection) => {
            info!("✅ Successfully connected to Turnkey wallet");
            info!("   Connection ID: {}", connection.id);
            info!("   Wallet Type: {:?}", connection.wallet_type);
            info!("   Connected at: {}", connection.created_at);
            
            // Get wallet info
            if let Some(wallet_info) = wallet_manager.get_wallet_info(&connection.id).await {
                info!("   Public Key: {}", wallet_info.public_key.unwrap_or("N/A".to_string()));
            }
            
            // Demonstrate transaction signing
            let demo_transaction = create_demo_transaction();
            match wallet_manager.sign_with_wallet(&connection.id, &demo_transaction).await {
                Ok(signature) => {
                    info!("✅ Transaction signed successfully");
                    info!("   Signature length: {} bytes", signature.len());
                }
                Err(e) => {
                    warn!("⚠️  Failed to sign transaction: {}", e);
                }
            }
            
            // Cleanup
            let _ = wallet_manager.disconnect_wallet(&connection.id).await;
        }
        Err(e) => {
            warn!("⚠️  Failed to connect to Turnkey wallet: {}", e);
            info!("   Note: This is expected in demo environment without valid Turnkey credentials");
        }
    }
    
    Ok(())
}

async fn demo_phantom_wallet(wallet_manager: &Arc<WalletManager>) -> Result<(), Box<dyn std::error::Error>> {
    info!("\n👻 === Phantom Wallet Demo ===");
    
    let phantom_credentials = WalletCredentials {
        wallet_type: WalletType::Phantom,
        credentials: WalletCredentialData::Phantom {
            encrypted_private_key: "demo_encrypted_private_key".to_string(),
        },
    };
    
    match wallet_manager.connect_wallet(phantom_credentials).await {
        Ok(connection) => {
            info!("✅ Successfully connected to Phantom wallet");
            info!("   Connection ID: {}", connection.id);
            info!("   Wallet Type: {:?}", connection.wallet_type);
            
            // Get wallet info
            if let Some(wallet_info) = wallet_manager.get_wallet_info(&connection.id).await {
                info!("   Public Key: {}", wallet_info.public_key.unwrap_or("N/A".to_string()));
            }
            
            // Demonstrate transaction signing
            let demo_transaction = create_demo_transaction();
            match wallet_manager.sign_with_wallet(&connection.id, &demo_transaction).await {
                Ok(signature) => {
                    info!("✅ Transaction signed successfully");
                    info!("   Signature length: {} bytes", signature.len());
                }
                Err(e) => {
                    warn!("⚠️  Failed to sign transaction: {}", e);
                }
            }
            
            // Cleanup
            let _ = wallet_manager.disconnect_wallet(&connection.id).await;
        }
        Err(e) => {
            warn!("⚠️  Failed to connect to Phantom wallet: {}", e);
            info!("   Note: This might fail if Phantom extension is not installed");
        }
    }
    
    Ok(())
}

async fn demo_solflare_wallet(wallet_manager: &Arc<WalletManager>) -> Result<(), Box<dyn std::error::Error>> {
    info!("\n☀️ === Solflare Wallet Demo ===");
    
    let solflare_credentials = WalletCredentials {
        wallet_type: WalletType::Solflare,
        credentials: WalletCredentialData::Solflare {
            public_key: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        },
    };
    
    match wallet_manager.connect_wallet(solflare_credentials).await {
        Ok(connection) => {
            info!("✅ Successfully connected to Solflare wallet");
            info!("   Connection ID: {}", connection.id);
            info!("   Wallet Type: {:?}", connection.wallet_type);
            
            // Get wallet info
            if let Some(wallet_info) = wallet_manager.get_wallet_info(&connection.id).await {
                info!("   Public Key: {}", wallet_info.public_key.unwrap_or("N/A".to_string()));
            }
            
            // Demonstrate transaction signing
            let demo_transaction = create_demo_transaction();
            match wallet_manager.sign_with_wallet(&connection.id, &demo_transaction).await {
                Ok(signature) => {
                    info!("✅ Transaction signed successfully");
                    info!("   Signature length: {} bytes", signature.len());
                }
                Err(e) => {
                    warn!("⚠️  Failed to sign transaction: {}", e);
                }
            }
            
            // Cleanup
            let _ = wallet_manager.disconnect_wallet(&connection.id).await;
        }
        Err(e) => {
            warn!("⚠️  Failed to connect to Solflare wallet: {}", e);
            info!("   Note: This might fail if Solflare is not installed");
        }
    }
    
    Ok(())
}

async fn demo_private_key_wallet(wallet_manager: &Arc<WalletManager>) -> Result<(), Box<dyn std::error::Error>> {
    info!("\n🔑 === Private Key Wallet Demo ===");
    
    // Generate a new keypair for demo
    let keypair = solana_sdk::signer::keypair::Keypair::new();
    let private_key = bs58::encode(keypair.to_bytes());
    
    let private_key_credentials = WalletCredentials {
        wallet_type: WalletType::PrivateKey,
        credentials: WalletCredentialData::PrivateKey {
            private_key,
        },
    };
    
    match wallet_manager.connect_wallet(private_key_credentials).await {
        Ok(connection) => {
            info!("✅ Successfully connected to Private Key wallet");
            info!("   Connection ID: {}", connection.id);
            info!("   Wallet Type: {:?}", connection.wallet_type);
            
            // Get wallet info
            if let Some(wallet_info) = wallet_manager.get_wallet_info(&connection.id).await {
                info!("   Public Key: {}", wallet_info.public_key.unwrap_or("N/A".to_string()));
            }
            
            // Demonstrate transaction signing
            let demo_transaction = create_demo_transaction();
            match wallet_manager.sign_with_wallet(&connection.id, &demo_transaction).await {
                Ok(signature) => {
                    info!("✅ Transaction signed successfully");
                    info!("   Signature length: {} bytes", signature.len());
                }
                Err(e) => {
                    warn!("⚠️  Failed to sign transaction: {}", e);
                }
            }
            
            // Cleanup
            let _ = wallet_manager.disconnect_wallet(&connection.id).await;
        }
        Err(e) => {
            error!("❌ Failed to connect to Private Key wallet: {}", e);
        }
    }
    
    Ok(())
}

async fn demo_wallet_manager_features(wallet_manager: &Arc<WalletManager>) -> Result<(), Box<dyn std::error::Error>> {
    info!("\n🛠️ === Wallet Manager Features Demo ===");
    
    // Get connection metrics
    let metrics = wallet_manager.get_connection_metrics().await;
    info!("📊 Connection Metrics:");
    info!("   Total Connections: {}", metrics["total_connections"]);
    info!("   Max Connections: {}", metrics["max_connections"]);
    info!("   Supported Wallets: {:?}", metrics["supported_wallets"]);
    
    // Demonstrate batch transaction signing
    info!("\n📦 Batch Transaction Signing Demo");
    
    // Create a test connection for batch demo
    let test_credentials = WalletCredentials {
        wallet_type: WalletType::Phantom,
        credentials: WalletCredentialData::Phantom {
            encrypted_private_key: "batch_test_key".to_string(),
        },
    };
    
    if let Ok(connection) = wallet_manager.connect_wallet(test_credentials).await {
        let transactions = vec![
            create_demo_transaction(),
            create_demo_transaction(),
            create_demo_transaction(),
        ];
        
        match wallet_manager.batch_sign_transactions(&connection.id, &transactions).await {
            Ok(results) => {
                info!("✅ Batch signing completed");
                info!("   Total transactions: {}", results.len());
                for (i, result) in results.iter().enumerate() {
                    match result {
                        Ok(signature) => {
                            info!("   Transaction {}: ✅ Signed ({} bytes)", i + 1, signature.len());
                        }
                        Err(e) => {
                            warn!("   Transaction {}: ❌ Failed - {}", i + 1, e);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("⚠️  Batch signing failed: {}", e);
            }
        }
        
        // Cleanup
        let _ = wallet_manager.disconnect_wallet(&connection.id).await;
    }
    
    // Demonstrate connection validation
    info!("\n🔍 Connection Validation Demo");
    let validation_result = wallet_manager.validate_connection("non_existent").await;
    match validation_result {
        Ok(is_valid) => {
            info!("   Non-existent connection validation: {}", is_valid);
        }
        Err(e) => {
            warn!("   Validation error: {}", e);
        }
    }
    
    // Demonstrate cleanup
    info!("\n🧹 Connection Cleanup Demo");
    let cleaned_count = wallet_manager.cleanup_expired_connections().await?;
    info!("   Cleaned up {} expired connections", cleaned_count);
    
    Ok(())
}

fn create_demo_transaction() -> Vec<u8> {
    // Create a simple demo transaction
    // In a real implementation, this would be a proper Solana transaction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        2, 0, 0, 0, // Number of instructions
        // Instruction 1: System program transfer
        1, 0, 0, 0, // Program ID index
        0, 0, 0, 0, // Accounts count
        0, 0, 0, 0, // Data length
        // Instruction 2: Custom instruction
        2, 0, 0, 0, // Program ID index
        0, 0, 0, 0, // Accounts count
        0, 0, 0, 0, // Data length
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_demo_transaction_creation() {
        let tx = create_demo_transaction();
        assert!(!tx.is_empty());
        assert!(tx.len() > 20); // Should have some content
    }

    #[tokio::test]
    async fn test_wallet_manager_config() {
        let config = WalletManagerConfig {
            enable_turnkey: true,
            enable_phantom: true,
            enable_solflare: true,
            enable_private_key: true,
            ..Default::default()
        };
        
        let manager = WalletManager::with_config(config);
        let supported = manager.get_supported_wallets().await;
        
        assert!(supported.contains(&WalletType::Turnkey));
        assert!(supported.contains(&WalletType::Phantom));
        assert!(supported.contains(&WalletType::Solflare));
        assert!(supported.contains(&WalletType::PrivateKey));
    }
}
