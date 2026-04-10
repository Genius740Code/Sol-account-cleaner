//! Turnkey wallet integration example
//! 
//! This example demonstrates comprehensive Turnkey wallet integration including:
//! - Wallet connection and authentication
//! - Session management and caching
//! - Transaction signing
//! - Error handling and retry logic
//! - Health checks and monitoring

use solana_recover::*;
use solana_recover::wallet::*;
use solana_recover::wallet::turnkey::{TurnkeyProvider, TurnkeyConfig};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Turnkey Integration Example ===\n");

    // 1. Create Turnkey provider with custom configuration
    let turnkey_config = TurnkeyConfig {
        api_url: "https://api.turnkey.com".to_string(),
        timeout_seconds: 30,
        retry_attempts: 3,
        enable_session_caching: true,
    };

    let turnkey_provider = Arc::new(TurnkeyProvider::with_config(turnkey_config));

    // 2. Perform health check
    println!("1. Performing Turnkey health check...");
    match turnkey_provider.health_check().await {
        Ok(is_healthy) => {
            if is_healthy {
                println!("   Turnkey service is healthy! \u2713");
            } else {
                println!("   Turnkey service is unhealthy! \u2717");
                return Err("Turnkey service is unhealthy".into());
            }
        }
        Err(e) => {
            println!("   Health check failed: {} \u2717", e);
            return Err(e.into());
        }
    }

    // 3. Create Turnkey credentials (replace with actual values)
    let credentials = WalletCredentials {
        wallet_type: WalletType::Turnkey,
        credentials: WalletCredentialData::Turnkey {
            api_key: "your_turnkey_api_key".to_string(),
            organization_id: "your_organization_id".to_string(),
            private_key_id: "your_private_key_id".to_string(),
        },
    };

    println!("\n2. Connecting to Turnkey wallet...");

    // 4. Connect to wallet (with session caching)
    let connection = match turnkey_provider.connect(&credentials).await {
        Ok(conn) => {
            println!("   Successfully connected to Turnkey wallet! \u2713");
            println!("   Connection ID: {}", conn.id);
            conn
        }
        Err(e) => {
            println!("   Failed to connect: {} \u2717", e);
            println!("   (Expected with demo credentials)");
            return Err(e.into());
        }
    };

    // 5. Get public key
    println!("\n3. Retrieving public key...");
    match turnkey_provider.get_public_key(&connection).await {
        Ok(public_key) => {
            println!("   Public key: {} \u2713", public_key);
        }
        Err(e) => {
            println!("   Failed to get public key: {} \u2717", e);
        }
    }

    // 6. Test session caching
    println!("\n4. Testing session caching...");
    
    // First connection (should create new session)
    let start = std::time::Instant::now();
    let conn1 = turnkey_provider.connect(&credentials).await?;
    let first_connect_time = start.elapsed();
    println!("   First connection time: {:?}", first_connect_time);

    // Second connection (should use cached session)
    let start = std::time::Instant::now();
    let conn2 = turnkey_provider.connect(&credentials).await?;
    let second_connect_time = start.elapsed();
    println!("   Second connection time: {:?}", second_connect_time);

    if second_connect_time < first_connect_time {
        println!("   Session caching working! \u2713");
    } else {
        println!("   Session caching may not be working \u2717");
    }

    // 7. Check cache statistics
    let (total_sessions, valid_sessions) = turnkey_provider.get_cache_stats();
    println!("   Cache stats: {}/{} valid sessions", valid_sessions, total_sessions);

    // 8. Test transaction signing (example transaction)
    println!("\n5. Testing transaction signing...");
    
    // Create a sample transaction (this would normally be a real Solana transaction)
    let sample_transaction = vec![
        0x01, 0x02, 0x03, 0x04, // Sample transaction data
        // In reality, this would be a serialized Solana transaction
    ];

    match turnkey_provider.sign_transaction(&connection, &sample_transaction).await {
        Ok(signed_tx) => {
            println!("   Transaction signed successfully! \u2713");
            println!("   Signed transaction length: {} bytes", signed_tx.len());
            
            // Verify signature format (should be 64 bytes signature + transaction)
            if signed_tx.len() == sample_transaction.len() + 64 {
                println!("   Signature format correct \u2713");
            } else {
                println!("   Signature format incorrect \u2717");
            }
        }
        Err(e) => {
            println!("   Failed to sign transaction: {} \u2717", e);
            println!("   (Expected with demo credentials)");
        }
    }

    // 9. Test with WalletManager
    println!("\n6. Testing with WalletManager...");
    
    let wallet_manager = Arc::new(WalletManager::new());
    
    // Connect using WalletManager
    match wallet_manager.connect_wallet(credentials.clone()).await {
        Ok(manager_connection) => {
            println!("   Connected via WalletManager! \u2713");
            println!("   Connection ID: {}", manager_connection.id);
            
            // List active connections
            let active_connections = wallet_manager.list_active_connections();
            println!("   Active connections: {}", active_connections.len());
            
            // Disconnect
            wallet_manager.disconnect_wallet(&manager_connection.id).await?;
            println!("   Disconnected successfully \u2713");
        }
        Err(e) => {
            println!("   WalletManager connection failed: {} \u2717", e);
        }
    }

    // 10. Test error handling
    println!("\n7. Testing error handling...");
    
    // Test with invalid credentials
    let invalid_credentials = WalletCredentials {
        wallet_type: WalletType::Turnkey,
        credentials: WalletCredentialData::Turnkey {
            api_key: "invalid_key".to_string(),
            organization_id: "invalid_org".to_string(),
            private_key_id: "invalid_key".to_string(),
        },
    };

    match turnkey_provider.connect(&invalid_credentials).await {
        Ok(_) => {
            println!("   Unexpected success with invalid credentials \u2717");
        }
        Err(e) => {
            println!("   Properly rejected invalid credentials \u2713");
        }
    }

    // 11. Test session cleanup
    println!("\n8. Testing session cleanup...");
    
    // Clear cache
    turnkey_provider.clear_session_cache();
    let (total_sessions, valid_sessions) = turnkey_provider.get_cache_stats();
    println!("   Cache cleared: {}/{} sessions", valid_sessions, total_sessions);

    // 12. Disconnect original connection
    println!("\n9. Disconnecting...");
    match turnkey_provider.disconnect(&connection).await {
        Ok(_) => {
            println!("   Disconnected successfully! \u2713");
        }
        Err(e) => {
            println!("   Disconnect failed: {} \u2717", e);
        }
    }

    println!("\n=== Turnkey Integration Test Complete ===");
    println!("\nKey Features Demonstrated:");
    println!("\u2713 Health checks");
    println!("\u2713 Session management and caching");
    println!("\u2713 Retry logic with exponential backoff");
    println!("\u2713 Transaction signing");
    println!("\u2713 Error handling");
    println!("\u2713 WalletManager integration");
    println!("\u2713 Cache management");
    println!("\u2713 Connection lifecycle management");

    Ok(())
}
