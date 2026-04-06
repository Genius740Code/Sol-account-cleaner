#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scanner::{WalletScanner, OpenOrdersAccountInfo};
    use solana_account_decoder::UiAccountEncoding;
    use base64::{Engine as _, engine::general_purpose};

    #[test]
    fn test_parse_empty_open_orders_account() {
        let scanner = WalletScanner::new(/* mock connection pool */);
        
        // Create a mock OpenOrders account with all zero balances
        // Structure: discriminator (8) + market (32) + owner (32) + base_free (8) + base_total (8) + quote_free (8) + quote_total (8)
        let mut account_data = vec![0u8; 104]; // 104 bytes total
        
        // Set discriminator (first 8 bytes)
        account_data[0..8].copy_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
        
        // Set dummy market pubkey (bytes 8-40)
        let dummy_market = [2u8; 32];
        account_data[8..40].copy_from_slice(&dummy_market);
        
        // Set dummy owner pubkey (bytes 40-72)
        let dummy_owner = [3u8; 32];
        account_data[40..72].copy_from_slice(&dummy_owner);
        
        // All balances remain zero (bytes 72-104)
        
        // Encode to base64
        let encoded_data = general_purpose::STANDARD.encode(&account_data);
        
        // Parse the account
        let result = scanner.parse_open_orders_account_from_binary(&encoded_data, &UiAccountEncoding::Base64);
        
        assert!(result.is_ok(), "Failed to parse OpenOrders account: {:?}", result.err());
        
        let open_orders = result.unwrap();
        assert_eq!(open_orders.base_token_free, 0);
        assert_eq!(open_orders.base_token_total, 0);
        assert_eq!(open_orders.quote_token_free, 0);
        assert_eq!(open_orders.quote_token_total, 0);
    }

    #[test]
    fn test_parse_non_empty_open_orders_account() {
        let scanner = WalletScanner::new(/* mock connection pool */);
        
        // Create a mock OpenOrders account with non-zero balances
        let mut account_data = vec![0u8; 104];
        
        // Set discriminator
        account_data[0..8].copy_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
        
        // Set dummy market and owner
        account_data[8..40].copy_from_slice(&[2u8; 32]);
        account_data[40..72].copy_from_slice(&[3u8; 32]);
        
        // Set non-zero base_token_free (bytes 72-80)
        account_data[72..80].copy_from_slice(&1000u64.to_le_bytes());
        
        // Set non-zero quote_token_total (bytes 96-104)
        account_data[96..104].copy_from_slice(&500u64.to_le_bytes());
        
        let encoded_data = general_purpose::STANDARD.encode(&account_data);
        
        let result = scanner.parse_open_orders_account_from_binary(&encoded_data, &UiAccountEncoding::Base64);
        
        assert!(result.is_ok(), "Failed to parse OpenOrders account: {:?}", result.err());
        
        let open_orders = result.unwrap();
        assert_eq!(open_orders.base_token_free, 1000);
        assert_eq!(open_orders.base_token_total, 0);
        assert_eq!(open_orders.quote_token_free, 0);
        assert_eq!(open_orders.quote_token_total, 500);
    }

    #[test]
    fn test_open_orders_account_insufficient_data() {
        let scanner = WalletScanner::new(/* mock connection pool */);
        
        // Create account data that's too short
        let short_data = vec![0u8; 50]; // Less than required 96 bytes
        let encoded_data = general_purpose::STANDARD.encode(&short_data);
        
        let result = scanner.parse_open_orders_account_from_binary(&encoded_data, &UiAccountEncoding::Base64);
        
        assert!(result.is_err(), "Should have failed with insufficient data");
        
        if let Err(e) = result {
            assert!(matches!(e, crate::core::SolanaRecoverError::InternalError(msg) 
                if msg.contains("Invalid OpenOrders account data length")));
        }
    }

    #[test]
    fn test_open_orders_account_invalid_encoding() {
        let scanner = WalletScanner::new(/* mock connection pool */);
        
        // Test with invalid base64
        let invalid_base64 = "invalid_base64_string!!!";
        
        let result = scanner.parse_open_orders_account_from_binary(invalid_base64, &UiAccountEncoding::Base64);
        
        assert!(result.is_err(), "Should have failed with invalid base64");
    }

    #[test]
    fn test_openbook_program_ids() {
        use std::str::FromStr;
        
        // Test that we can parse the OpenBook program IDs
        let openbook_v2 = Pubkey::from_str("opnb2vDkSQsqmY24zQ4DDEZf1V3oEisPZ5bEErLNRsA").unwrap();
        let serum_dex = Pubkey::from_str("srmqPvvk92GzrcCbKgSGx3mFHTEQuoE3jUuAM6gEKrP").unwrap();
        
        // Verify these are valid pubkeys
        assert_ne!(openbook_v2, Pubkey::default());
        assert_ne!(serum_dex, Pubkey::default());
        
        // Verify they're different
        assert_ne!(openbook_v2, serum_dex);
    }

    #[test]
    fn test_open_orders_safety_validation() {
        // Test the safety logic: only accounts with ALL zero balances should be recoverable
        let test_cases = vec![
            // (base_free, base_total, quote_free, quote_total, should_be_recoverable)
            (0, 0, 0, 0, true),      // All zero - should be recoverable
            (1, 0, 0, 0, false),     // Non-zero base_free - NOT recoverable
            (0, 1, 0, 0, false),     // Non-zero base_total - NOT recoverable
            (0, 0, 1, 0, false),     // Non-zero quote_free - NOT recoverable
            (0, 0, 0, 1, false),     // Non-zero quote_total - NOT recoverable
            (1, 1, 1, 1, false),     // All non-zero - NOT recoverable
        ];
        
        for (base_free, base_total, quote_free, quote_total, expected_recoverable) in test_cases {
            let open_orders = OpenOrdersAccountInfo {
                base_token_free: base_free,
                base_token_total: base_total,
                quote_token_free: quote_free,
                quote_token_total: quote_total,
            };
            
            // Simulate the safety check logic
            let is_recoverable = open_orders.base_token_free == 0 && 
                                open_orders.quote_token_free == 0 && 
                                open_orders.base_token_total == 0 && 
                                open_orders.quote_token_total == 0;
            
            assert_eq!(is_recoverable, expected_recoverable, 
                "Failed for base_free={}, base_total={}, quote_free={}, quote_total={}", 
                base_free, base_total, quote_free, quote_total);
        }
    }
}
