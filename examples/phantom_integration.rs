//! Phantom Wallet Integration Example
//! 
//! This example demonstrates how to integrate with Phantom wallet
//! for browser-based Solana interactions.

use solana_recover::*;
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
    
    info!("👻 Starting Phantom Wallet Integration Example");
    
    // Create wallet manager
    let wallet_manager = Arc::new(WalletManager::new());
    
    // Demonstrate Phantom wallet integration
    demo_phantom_connection(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    demo_phantom_transaction_signing(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    demo_phantom_batch_operations(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    demo_phantom_error_handling(&wallet_manager).await.map_err(|e| -> Box<dyn std::error::Error> { e })?;
    
    info!("✅ Phantom Wallet Integration Example completed!");
    Ok(())
}

async fn demo_phantom_connection(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n🔗 === Phantom Connection Demo ===");
    
    // Create Phantom credentials
    let phantom_credentials = WalletCredentials {
        wallet_type: WalletType::Phantom,
        credentials: WalletCredentialData::Phantom {
            encrypted_private_key: "demo_phantom_encrypted_key".to_string(),
        },
    };
    
    info!("📡 Attempting to connect to Phantom wallet...");
    info!("   Note: Make sure Phantom extension is installed and unlocked");
    
    match wallet_manager.connect_wallet(phantom_credentials).await {
        Ok(connection) => {
            info!("✅ Successfully connected to Phantom wallet");
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
            error!("❌ Failed to connect to Phantom wallet: {}", e);
            info!("💡 Troubleshooting tips:");
            info!("   1. Ensure Phantom extension is installed");
            info!("   2. Make sure Phantom is unlocked");
            info!("   3. Check that your site has permission to access Phantom");
            info!("   4. Ensure you're using HTTPS (required for wallet communication)");
            Err(e.into())
        }
    }
}

async fn demo_phantom_transaction_signing(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n✍️ === Phantom Transaction Signing Demo ===");
    
    // Get active connections (should have one from previous demo)
    let active_connections = wallet_manager.list_active_connections();
    
    if active_connections.is_empty() {
        warn!("⚠️  No active Phantom connections found. Please run connection demo first.");
        return Ok(());
    }
    
    let connection = &active_connections[0];
    info!("📝 Using Phantom connection: {}", connection.id);
    
    // Create different types of demo transactions
    let transactions = vec![
        ("Simple Transfer", create_simple_transfer_transaction()),
        ("Token Transfer", create_token_transfer_transaction()),
        ("Custom Instruction", create_custom_instruction_transaction()),
    ];
    
    for (tx_type, transaction) in transactions {
        info!("\n📄 Signing {} transaction", tx_type);
        
        match wallet_manager.sign_with_wallet(&connection.id, &transaction).await {
            Ok(signature) => {
                info!("✅ Transaction signed successfully");
                info!("   Signature length: {} bytes", signature.len());
                info!("   Transaction type: {}", tx_type);
                
                // Decode and display signature (first 16 bytes for demo)
                let signature_preview = &signature[..signature.len().min(16)];
                info!("   Signature preview: {:02x?}", signature_preview);
            }
            Err(e) => {
                warn!("⚠️  Failed to sign {} transaction: {}", tx_type, e);
                
                // Provide specific error handling advice
                if e.to_string().contains("user rejected") {
                    info!("💡 User rejected the transaction. This is normal behavior.");
                } else if e.to_string().contains("insufficient funds") {
                    info!("💡 Insufficient funds. Please check your SOL balance.");
                }
            }
        }
    }
    
    Ok(())
}

async fn demo_phantom_batch_operations(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n📦 === Phantom Batch Operations Demo ===");
    
    let active_connections = wallet_manager.list_active_connections();
    
    if active_connections.is_empty() {
        warn!("⚠️  No active Phantom connections found");
        return Ok(());
    }
    
    let connection = &active_connections[0];
    info!("🔄 Performing batch operations with connection: {}", connection.id);
    
    // Create multiple transactions for batch signing
    let batch_transactions: Vec<Vec<u8>> = (0..5)
        .map(|i| create_batch_transaction(i))
        .collect();
    
    info!("📝 Created {} transactions for batch signing", batch_transactions.len());
    
    // Perform batch signing
    match wallet_manager.batch_sign_transactions(&connection.id, &batch_transactions).await {
        Ok(results) => {
            info!("✅ Batch signing completed");
            info!("   Total transactions: {}", results.len());
            
            let mut successful_count = 0;
            let mut failed_count = 0;
            
            for (i, result) in results.iter().enumerate() {
                match result {
                    Ok(signature) => {
                        successful_count += 1;
                        info!("   Transaction {}: ✅ Signed ({} bytes)", i + 1, signature.len());
                    }
                    Err(e) => {
                        failed_count += 1;
                        warn!("   Transaction {}: ❌ Failed - {}", i + 1, e);
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
    
    Ok(())
}

async fn demo_phantom_error_handling(wallet_manager: &Arc<WalletManager>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("\n🚨 === Phantom Error Handling Demo ===");
    
    // Test 1: Invalid connection ID
    info!("🧪 Testing invalid connection ID...");
    match wallet_manager.validate_connection("invalid_connection_id").await {
        Ok(is_valid) => {
            info!("   Invalid connection validation: {} (expected: false)", is_valid);
        }
        Err(e) => {
            warn!("   Validation error: {}", e);
        }
    }
    
    // Test 2: Transaction signing with invalid connection
    info!("🧪 Testing transaction signing with invalid connection...");
    let test_transaction = create_simple_transfer_transaction();
    
    match wallet_manager.sign_with_wallet("invalid_connection_id", &test_transaction).await {
        Ok(_) => {
            warn!("   Unexpected success with invalid connection");
        }
        Err(e) => {
            info!("   Expected error with invalid connection: {}", e);
        }
    }
    
    // Test 3: Batch operations with invalid connection
    info!("🧪 Testing batch operations with invalid connection...");
    let batch_tx = vec![create_simple_transfer_transaction()];
    
    match wallet_manager.batch_sign_transactions("invalid_connection_id", &batch_tx).await {
        Ok(_) => {
            warn!("   Unexpected batch success with invalid connection");
        }
        Err(e) => {
            info!("   Expected batch error with invalid connection: {}", e);
        }
    }
    
    // Test 4: Connection cleanup
    info!("🧪 Testing connection cleanup...");
    
    // Clean up any existing connections
    let active_connections = wallet_manager.list_active_connections();
    for connection in active_connections {
        info!("   Disconnecting: {}", connection.id);
        match wallet_manager.disconnect_wallet(&connection.id).await {
            Ok(_) => {
                info!("   ✅ Disconnected successfully");
            }
            Err(e) => {
                warn!("   ⚠️  Disconnection failed: {}", e);
            }
        }
    }
    
    // Verify cleanup
    let remaining_connections = wallet_manager.list_active_connections();
    info!("   Remaining connections: {}", remaining_connections.len());
    
    Ok(())
}

// Transaction creation helpers
fn create_simple_transfer_transaction() -> Vec<u8> {
    // Simple SOL transfer transaction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        1, 0, 0, 0, // Program ID index (System program)
        2, 0, 0, 0, // Accounts count
        0, 0, 0, 0, // Data length
        0, 0, 0, 0, // Data (simplified for demo)
    ]
}

fn create_token_transfer_transaction() -> Vec<u8> {
    // SPL token transfer transaction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        2, 0, 0, 0, // Program ID index (Token program)
        3, 0, 0, 0, // Accounts count
        0, 0, 0, 0, // Data length
        0, 0, 0, 0, // Data (simplified for demo)
    ]
}

fn create_custom_instruction_transaction() -> Vec<u8> {
    // Custom program instruction
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        3, 0, 0, 0, // Program ID index (Custom program)
        1, 0, 0, 0, // Accounts count
        4, 0, 0, 0, // Data length
        1, 2, 3, 4, // Custom instruction data
    ]
}

fn create_batch_transaction(index: u8) -> Vec<u8> {
    // Create a unique transaction for batch testing
    vec![
        1, 0, 0, 0, // Version
        1, 0, 0, 0, // Number of signatures
        0, 0, 0, 0, // Number of required signatures
        1, 0, 0, 0, // Number of instructions
        4, 0, 0, 0, // Program ID index
        1, 0, 0, 0, // Accounts count
        1, 0, 0, 0, // Data length
        index, // Unique identifier
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_creation() {
        let tx = create_simple_transfer_transaction();
        assert!(!tx.is_empty());
        assert!(tx.len() > 10);
    }

    #[test]
    fn test_batch_transaction_uniqueness() {
        let tx1 = create_batch_transaction(1);
        let tx2 = create_batch_transaction(2);
        
        assert_ne!(tx1, tx2);
        assert_eq!(tx1[tx1.len() - 1], 1);
        assert_eq!(tx2[tx2.len() - 1], 2);
    }

    #[tokio::test]
    async fn test_phantom_credentials_structure() {
        let credentials = WalletCredentials {
            wallet_type: WalletType::Phantom,
            credentials: WalletCredentialData::Phantom {
                encrypted_private_key: "test_key".to_string(),
            },
        };
        
        assert!(matches!(credentials.wallet_type, WalletType::Phantom));
        if let WalletCredentialData::Phantom { encrypted_private_key } = credentials.credentials {
            assert_eq!(encrypted_private_key, "test_key");
        } else {
            panic!("Expected Phantom credential data");
        }
    }
}
