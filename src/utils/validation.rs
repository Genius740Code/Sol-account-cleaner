use crate::core::{Result, SolanaRecoverError};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Comprehensive input validation utilities
pub struct InputValidator;

impl InputValidator {
    /// Validate wallet address format and structure
    pub fn validate_wallet_address(address: &str) -> Result<()> {
        // Check length
        if address.len() != 44 {
            return Err(SolanaRecoverError::InvalidInput(
                format!("Invalid address length: expected 44, got {}", address.len())
            ));
        }
        
        // Check character set
        if !address.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Err(SolanaRecoverError::InvalidInput(
                "Invalid address characters: only alphanumeric, underscore, and hyphen allowed".to_string()
            ));
        }
        
        // Validate as pubkey
        Pubkey::from_str(address)
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid Solana pubkey format".to_string()))?;
        
        Ok(())
    }
    
    /// Validate batch size limits
    pub fn validate_batch_size(size: usize) -> Result<()> {
        if size == 0 {
            return Err(SolanaRecoverError::InvalidInput("Batch size cannot be zero".to_string()));
        }
        
        if size > 1000 {
            return Err(SolanaRecoverError::InvalidInput(
                format!("Batch size too large: maximum is 1000, got {}", size)
            ));
        }
        
        Ok(())
    }
    
    /// Validate SOL amount limits
    pub fn validate_amount(amount: u64) -> Result<()> {
        if amount == 0 {
            return Err(SolanaRecoverError::InvalidInput("Amount cannot be zero".to_string()));
        }
        
        // Maximum 1M SOL to prevent excessive transactions
        const MAX_AMOUNT: u64 = 1_000_000_000_000_000; // 1M SOL in lamports
        if amount > MAX_AMOUNT {
            return Err(SolanaRecoverError::InvalidInput(
                format!("Amount exceeds maximum limit: {} lamports", MAX_AMOUNT)
            ));
        }
        
        Ok(())
    }
    
    /// Validate timeout value
    pub fn validate_timeout(timeout_ms: u64) -> Result<()> {
        if timeout_ms == 0 {
            return Err(SolanaRecoverError::InvalidInput("Timeout cannot be zero".to_string()));
        }
        
        if timeout_ms > 300_000 { // 5 minutes max
            return Err(SolanaRecoverError::InvalidInput(
                format!("Timeout too large: maximum is 300000ms (5 minutes), got {}", timeout_ms)
            ));
        }
        
        Ok(())
    }
    
    /// Validate RPC endpoint URL
    pub fn validate_rpc_endpoint(url: &str) -> Result<()> {
        if url.is_empty() {
            return Err(SolanaRecoverError::InvalidInput("RPC endpoint cannot be empty".to_string()));
        }
        
        // Check URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(SolanaRecoverError::InvalidInput(
                "RPC endpoint must start with http:// or https://".to_string()
            ));
        }
        
        // Check URL length
        if url.len() > 2048 {
            return Err(SolanaRecoverError::InvalidInput(
                "RPC endpoint URL too long (max 2048 characters)".to_string()
            ));
        }
        
        // Basic URL validation
        if let Err(e) = url::Url::parse(url) {
            return Err(SolanaRecoverError::InvalidInput(
                format!("Invalid RPC endpoint URL: {}", e)
            ));
        }
        
        Ok(())
    }
    
    /// Validate destination address for SOL recovery
    pub fn validate_destination_address(address: &str) -> Result<()> {
        // First validate as wallet address
        Self::validate_wallet_address(address)?;
        
        // Additional validation for destination addresses
        let pubkey = Pubkey::from_str(address)
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid destination address".to_string()))?;
        
        // Check if it's a system program or other restricted address
        if pubkey == solana_sdk::system_program::id() {
            return Err(SolanaRecoverError::InvalidInput(
                "Cannot send SOL to system program".to_string()
            ));
        }
        
        if pubkey == solana_sdk::sysvar::rent::id() {
            return Err(SolanaRecoverError::InvalidInput(
                "Cannot send SOL to rent sysvar".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Validate private key format (for validation only, never store/log)
    pub fn validate_private_key_format(private_key: &str) -> Result<()> {
        if private_key.is_empty() {
            return Err(SolanaRecoverError::InvalidInput("Private key cannot be empty".to_string()));
        }
        
        // Check for common base64/bs58 formats
        if let Err(_) = bs58::decode(private_key).into_vec() {
            // Try base64
            use base64::Engine;
            if let Err(_) = base64::engine::general_purpose::STANDARD.decode(private_key) {
                return Err(SolanaRecoverError::InvalidInput(
                    "Invalid private key format: must be base58 or base64 encoded".to_string()
                ));
            }
        }
        
        Ok(())
    }
    
    /// Validate user ID format
    pub fn validate_user_id(user_id: &str) -> Result<()> {
        if user_id.is_empty() {
            return Err(SolanaRecoverError::InvalidInput("User ID cannot be empty".to_string()));
        }
        
        if user_id.len() > 128 {
            return Err(SolanaRecoverError::InvalidInput(
                "User ID too long (max 128 characters)".to_string()
            ));
        }
        
        // Check for valid characters (alphanumeric, underscore, hyphen)
        if !user_id.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '@' || c == '.') {
            return Err(SolanaRecoverError::InvalidInput(
                "Invalid user ID characters: only alphanumeric, underscore, hyphen, @, and . allowed".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Validate transaction signature format
    pub fn validate_signature(signature: &str) -> Result<()> {
        if signature.len() != 88 && signature.len() != 64 {
            return Err(SolanaRecoverError::InvalidInput(
                format!("Invalid signature length: expected 64 or 88, got {}", signature.len())
            ));
        }
        
        // Try to parse as signature
        solana_sdk::signature::Signature::from_str(signature)
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid signature format".to_string()))?;
        
        Ok(())
    }
    
    /// Validate priority fee
    pub fn validate_priority_fee(fee_lamports: u64) -> Result<()> {
        const MAX_PRIORITY_FEE: u64 = 10_000_000_000; // 10 SOL max priority fee
        
        if fee_lamports > MAX_PRIORITY_FEE {
            return Err(SolanaRecoverError::InvalidInput(
                format!("Priority fee too high: maximum is {} lamports", MAX_PRIORITY_FEE)
            ));
        }
        
        Ok(())
    }
    
    /// Validate max fee
    pub fn validate_max_fee(fee_lamports: u64) -> Result<()> {
        const MAX_FEE: u64 = 100_000_000_000; // 100 SOL max total fee
        
        if fee_lamports > MAX_FEE {
            return Err(SolanaRecoverError::InvalidInput(
                format!("Max fee too high: maximum is {} lamports", MAX_FEE)
            ));
        }
        
        Ok(())
    }
    
    /// Validate network name
    pub fn validate_network(network: &str) -> Result<()> {
        match network {
            "mainnet" | "mainnet-beta" | "devnet" | "testnet" => Ok(()),
            _ => Err(SolanaRecoverError::InvalidInput(
                format!("Unsupported network: {}. Supported: mainnet, devnet, testnet", network)
            )),
        }
    }
    
    /// Validate commitment level
    pub fn validate_commitment(commitment: &str) -> Result<()> {
        match commitment {
            "processed" | "confirmed" | "finalized" | "recent" | "single" | "singleGossip" | "root" => Ok(()),
            _ => Err(SolanaRecoverError::InvalidInput(
                format!("Invalid commitment level: {}", commitment)
            )),
        }
    }
}

/// Input sanitization utilities
pub struct InputSanitizer;

impl InputSanitizer {
    /// Sanitize string input by removing dangerous characters
    pub fn sanitize_string(input: &str, max_length: usize) -> String {
        input
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.' || *c == '@')
            .take(max_length)
            .collect()
    }
    
    /// Sanitize and validate wallet address
    pub fn sanitize_and_validate_address(address: &str) -> Result<String> {
        let sanitized = Self::sanitize_string(address, 44);
        InputValidator::validate_wallet_address(&sanitized)?;
        Ok(sanitized)
    }
    
    /// Sanitize user ID
    pub fn sanitize_user_id(user_id: &str) -> String {
        Self::sanitize_string(user_id, 128)
    }
    
    /// Sanitize operation name
    pub fn sanitize_operation_name(operation: &str) -> String {
        Self::sanitize_string(operation, 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_wallet_address() {
        // Valid address
        let valid_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        assert!(InputValidator::validate_wallet_address(valid_address).is_ok());
        
        // Invalid length
        let invalid_address = "short";
        assert!(InputValidator::validate_wallet_address(invalid_address).is_err());
        
        // Invalid characters
        let invalid_chars = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWW@";
        assert!(InputValidator::validate_wallet_address(invalid_chars).is_err());
    }
    
    #[test]
    fn test_validate_batch_size() {
        assert!(InputValidator::validate_batch_size(10).is_ok());
        assert!(InputValidator::validate_batch_size(0).is_err());
        assert!(InputValidator::validate_batch_size(1001).is_err());
    }
    
    #[test]
    fn test_validate_amount() {
        assert!(InputValidator::validate_amount(1_000_000).is_ok());
        assert!(InputValidator::validate_amount(0).is_err());
        assert!(InputValidator::validate_amount(2_000_000_000_000_000).is_err()); // Over 1M SOL
    }
    
    #[test]
    fn test_sanitize_string() {
        let input = "test@123#$%^&*()";
        let sanitized = InputSanitizer::sanitize_string(input, 50);
        assert_eq!(sanitized, "test@123");
    }
}
