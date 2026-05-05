#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::private_key::PrivateKeyProvider;
    use crate::wallet::{WalletCredentials, WalletCredentialData};

    #[tokio::test]
    async fn test_private_key_connection_flow() {
        let provider = PrivateKeyProvider::new();
        
        // Test with a sample private key from environment variable
        let test_private_key = std::env::var("TEST_PRIVATE_KEY")
            .unwrap_or_else(|_| {
                // Generate a test keypair if no environment variable is set
                let test_keypair = solana_sdk::signature::Keypair::new();
                bs58::encode(test_keypair.to_bytes()).into_string()
            });
        
        println!("Testing private key parsing...");
        let parse_result = provider.parse_private_key(&test_private_key);
        assert!(parse_result.is_ok(), "Private key parsing should succeed");
        
        let keypair = parse_result.unwrap();
        println!("Public key: {}", keypair.pubkey());
        
        // Test wallet connection
        println!("Testing wallet connection...");
        let credentials = WalletCredentials {
            credentials: WalletCredentialData::PrivateKey {
                private_key: test_private_key,
            },
        };
        
        let connection_result = provider.connect(&credentials).await;
        assert!(connection_result.is_ok(), "Wallet connection should succeed");
        
        let connection = connection_result.unwrap();
        println!("Connection ID: {}", connection.id);
        assert_eq!(connection.wallet_type, crate::wallet::WalletType::PrivateKey);
        
        // Test getting public key
        let pubkey_result = provider.get_public_key(&connection).await;
        assert!(pubkey_result.is_ok(), "Getting public key should succeed");
        
        let pubkey = pubkey_result.unwrap();
        println!("Retrieved public key: {}", pubkey);
        assert_eq!(pubkey, keypair.pubkey().to_string());
    }
}
