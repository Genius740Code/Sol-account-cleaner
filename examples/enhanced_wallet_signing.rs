use solana_recover::wallet::{
    WalletManager, WalletCredentials, 
    PrivateKeyProvider, TransactionValidator, NonceManager, AuditLogger
};
use solana_recover::wallet::manager::{WalletCredentialData, WalletType};
use solana_recover::wallet::{RiskLevel, SecurityContext};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    message::Message,
    instruction::Instruction,
    system_program,
};
use std::time::Duration;
use tokio::time::sleep;
use bs58;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Enhanced Wallet Signing Demo");
    println!("============================");

    // Initialize enhanced wallet manager
    let wallet_manager = WalletManager::new();
    
    // Create test credentials
    let test_keypair = Keypair::new();
    let test_private_key = bs58::encode(test_keypair.to_bytes()).into_string();
    let credentials = WalletCredentials {
        wallet_type: WalletType::PrivateKey,
        credentials: WalletCredentialData::PrivateKey {
            private_key: test_private_key.to_string(),
        },
    };

    // Connect wallet
    println!("Connecting wallet...");
    let connection = wallet_manager.connect_wallet(credentials).await?;
    println!("Connected! Connection ID: {}", connection.id);

    // Create a sample transaction
    println!("Creating sample transaction...");
    let from_keypair = Keypair::new();
    let to_pubkey = Pubkey::new_unique();
    
    let instruction = Instruction::new_with_bytes(
        system_program::id(),
        &[
            2, // Transfer instruction index
            0, 0, 0, 0, 0, 0, 0, 0, // Amount (0 lamports)
        ],
        vec![
            solana_sdk::instruction::AccountMeta::new(from_keypair.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(to_pubkey, false),
        ],
    );

    let message = Message::new(&[instruction], Some(&from_keypair.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.sign(&[&from_keypair], solana_sdk::hash::Hash::new_unique());

    // Serialize transaction for signing
    let transaction_bytes = bincode::serialize(&transaction)?;

    // Test enhanced signing with validation
    println!("Testing enhanced signing with validation...");
    match wallet_manager.sign_transaction_enhanced(
        &connection.id,
        &transaction_bytes,
        Some("demo_user".to_string()),
    ).await {
        Ok(signed_tx) => {
            println!("Transaction signed successfully!");
            println!("Signed transaction size: {} bytes", signed_tx.len());
        }
        Err(e) => {
            println!("Signing failed: {}", e);
        }
    }

    // Test batch signing
    println!("Testing batch signing...");
    let transactions = vec![
        transaction_bytes.clone(),
        transaction_bytes.clone(),
        transaction_bytes.clone(),
    ];

    match wallet_manager.batch_sign_transactions(&connection.id, &transactions).await {
        Ok(results) => {
            println!("Batch signing completed!");
            for (i, result) in results.iter().enumerate() {
                match result {
                    Ok(signed_tx) => println!("  Transaction {}: Signed ({} bytes)", i + 1, signed_tx.len()),
                    Err(e) => println!("  Transaction {}: Failed - {}", i + 1, e),
                }
            }
        }
        Err(e) => {
            println!("Batch signing failed: {}", e);
        }
    }

    // Test transaction validation
    println!("Testing transaction validation...");
    let validator = TransactionValidator::new()
        .with_limits(5, 20, 1_000_000_000_000) // 5 signers, 20 instructions, 1000 SOL max
        .require_simulation(true);

    let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");
    match validator.validate_transaction(&transaction_bytes, &rpc_client).await {
        Ok(result) => {
            println!("Validation result:");
            println!("  Valid: {}", result.is_valid);
            println!("  Warnings: {}", result.warnings.len());
            println!("  Errors: {}", result.errors.len());
            
            if let Some(sim) = &result.simulation_result {
                println!("  Simulation:");
                println!("    Success: {}", sim.success);
                println!("    Units consumed: {}", sim.units_consumed);
                println!("    Fee: {} lamports", sim.fee);
            }
        }
        Err(e) => {
            println!("Validation failed: {}", e);
        }
    }

    // Test nonce management
    println!("Testing nonce management...");
    let nonce_manager = NonceManager::default();
    let nonce = solana_sdk::hash::Hash::new_unique();
    
    // Register nonce
    nonce_manager.register_nonce(from_keypair.pubkey(), nonce).await?;
    println!("Nonce registered: {}", nonce);

    // Validate transaction with nonce
    match nonce_manager.validate_transaction(&transaction).await {
        Ok(valid) => println!("Nonce validation: {}", valid),
        Err(e) => println!("Nonce validation failed: {}", e),
    }

    // Get nonce metrics
    let metrics = nonce_manager.get_metrics().await?;
    println!("Nonce metrics:");
    println!("  Active nonces: {}", metrics.active_nonces);
    println!("  Total signatures: {}", metrics.total_signatures);
    println!("  Signatures per hour: {:.2}", metrics.signatures_per_hour);

    // Test audit logging
    println!("Testing audit logging...");
    let audit_logger = AuditLogger::default();
    
    let security_context = SecurityContext {
        ip_address: Some("127.0.0.1".to_string()),
        user_agent: Some("enhanced-demo".to_string()),
        session_id: Some(connection.id.clone()),
        correlation_id: uuid::Uuid::new_v4().to_string(),
        request_id: uuid::Uuid::new_v4().to_string(),
        geo_location: None,
    };

    // Log wallet connection
    let event_id = audit_logger.log_wallet_connection(
        Some("demo_user".to_string()),
        "PrivateKey".to_string(),
        from_keypair.pubkey(),
        security_context.clone(),
    ).await?;
    println!("Wallet connection logged: {}", event_id);

    // Log transaction signing
    let event_id = audit_logger.log_transaction_signing(
        Some("demo_user".to_string()),
        "PrivateKey".to_string(),
        Some(from_keypair.pubkey()),
        &transaction,
        *transaction.signatures.first().unwrap(),
        security_context.clone(),
        RiskLevel::Low,
    ).await?;
    println!("Transaction signing logged: {}", event_id);

    // Get security metrics
    let security_metrics = audit_logger.get_security_metrics().await?;
    println!("Security metrics:");
    println!("  Total events: {}", security_metrics.total_events);
    println!("  Security violations: {}", security_metrics.security_violations);
    println!("  Replay attacks: {}", security_metrics.replay_attacks);

    // Test rate limiting
    println!("Testing rate limiting...");
    for i in 1..=3 {
        match wallet_manager.sign_transaction_enhanced(
            &connection.id,
            &transaction_bytes,
            Some("demo_user".to_string()),
        ).await {
            Ok(_) => println!("  Attempt {}: Success", i),
            Err(e) => println!("  Attempt {}: Failed - {}", i, e),
        }
        
        if i < 3 {
            sleep(Duration::from_millis(500)).await;
        }
    }

    // Cleanup
    println!("Cleaning up...");
    wallet_manager.disconnect_wallet(&connection.id).await?;
    println!("Disconnected wallet");

    println!("Demo completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_signing_flow() {
        let wallet_manager = WalletManager::new();
        
        // This test would require a valid private key
        // For demonstration purposes, we'll test the validation flow
        let validator = TransactionValidator::new();
        let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");
        
        // Create a minimal transaction
        let keypair = Keypair::new();
        let message = Message::new(&[], Some(&keypair.pubkey()));
        let transaction = Transaction::new_unsigned(message);
        let transaction_bytes = bincode::serialize(&transaction).unwrap();
        
        let result = validator.validate_transaction(&transaction_bytes, &rpc_client).await;
        assert!(result.is_ok());
        
        let validation = result.unwrap();
        // Should fail due to no recent blockhash
        assert!(!validation.is_valid);
    }

    #[tokio::test]
    async fn test_nonce_management() {
        let nonce_manager = NonceManager::default();
        let keypair = Keypair::new();
        let nonce = solana_sdk::hash::Hash::new_unique();
        
        // Test nonce registration
        assert!(nonce_manager.register_nonce(keypair.pubkey(), nonce).await.is_ok());
        
        // Test nonce validation
        let message = Message::new(&[], Some(&keypair.pubkey()));
        let mut transaction = Transaction::new_unsigned(message);
        transaction.message.recent_blockhash = nonce;
        
        assert!(nonce_manager.validate_transaction(&transaction).await.is_ok());
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let audit_logger = AuditLogger::default();
        let keypair = Keypair::new();
        
        let security_context = SecurityContext {
            ip_address: None,
            user_agent: Some("test-client".to_string()),
            session_id: Some("test-session".to_string()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            geo_location: None,
        };
        
        let event_id = audit_logger.log_wallet_connection(
            Some("test_user".to_string()),
            "PrivateKey".to_string(),
            keypair.pubkey(),
            security_context,
        ).await.unwrap();
        
        assert_ne!(event_id, uuid::Uuid::default());
    }
}
