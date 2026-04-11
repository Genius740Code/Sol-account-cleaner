use crate::core::{Result, SolanaRecoverError, WalletInfo, ScanResult, ScanStatus, EmptyAccount};
use crate::rpc::{ConnectionPoolTrait};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use uuid::Uuid;
use std::time::Instant;
use chrono::Utc;
use std::str::FromStr;
use solana_account_decoder::UiAccountEncoding;
use bs58;
use base64;
use tracing::{info, debug, warn, error};

// Token account structure for binary parsing
#[derive(Debug, Clone)]
pub struct TokenAccountInfo {
    pub mint: String,
    pub amount: u64,
}

// OpenBook OpenOrders account structure for binary parsing
#[derive(Debug, Clone)]
pub struct OpenOrdersAccountInfo {
    pub base_token_free: u64,
    pub base_token_total: u64,
    pub quote_token_free: u64,
    pub quote_token_total: u64,
}

const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

#[derive(Clone)]
pub struct WalletScanner {
    connection_pool: Arc<dyn ConnectionPoolTrait>,
}

impl WalletScanner {
    pub fn new(connection_pool: Arc<dyn ConnectionPoolTrait>) -> Self {
        Self { connection_pool }
    }

    pub async fn scan_wallet(&self, wallet_address: &str) -> Result<ScanResult> {
        let scan_id = Uuid::new_v4();
        let start_time = Instant::now();
        
        let scan_result = ScanResult {
            id: scan_id,
            wallet_address: wallet_address.to_string(),
            status: ScanStatus::InProgress,
            result: None,
            error: None,
            created_at: Utc::now(),
        };

        match self.scan_wallet_internal(wallet_address).await {
            Ok(wallet_info) => {
                let scan_time = start_time.elapsed().as_millis() as u64;
                let mut result = scan_result;
                result.status = ScanStatus::Completed;
                let mut wallet_info = wallet_info;
                wallet_info.scan_time_ms = scan_time;
                result.result = Some(wallet_info);
                Ok(result)
            }
            Err(e) => {
                let mut result = scan_result;
                result.status = ScanStatus::Failed;
                result.error = Some(e.to_string());
                Ok(result)
            }
        }
    }

    pub async fn scan_wallet_internal(&self, wallet_address: &str) -> Result<WalletInfo> {
        let pubkey = Pubkey::from_str(wallet_address)
            .map_err(|_| SolanaRecoverError::InvalidWalletAddress(wallet_address.to_string()))?;

        let client = self.connection_pool.get_client().await?;

        // Get all token accounts that might have recoverable SOL
        let all_accounts = client.get_all_recoverable_accounts(&pubkey).await?;
        let total_accounts = all_accounts.len();

        info!("Found {} total accounts for wallet {}", total_accounts, wallet_address);
        for (i, account) in all_accounts.iter().enumerate() {
            debug!("  Account {}: {} (owner: {}, lamports: {})", i + 1, account.pubkey, account.account.owner, account.account.lamports);
        }

        let mut empty_accounts: Vec<EmptyAccount> = Vec::new();
        let mut total_recoverable_lamports: u64 = 0;
        
        // Deduplicate accounts by pubkey to prevent double counting
        let mut seen_accounts = std::collections::HashSet::new();
        let mut unique_accounts = Vec::new();
        
        for keyed_account in all_accounts {
            if seen_accounts.insert(keyed_account.pubkey.clone()) {
                unique_accounts.push(keyed_account);
            }
        }
        
        debug!("Found {} unique accounts after deduplication", unique_accounts.len());

        // Parallelize account checking using futures::future::join_all
        let check_futures: Vec<_> = unique_accounts
            .iter()
            .map(|account| self.check_empty_account(account, wallet_address))
            .collect();
        
        let results = futures::future::join_all(check_futures).await;
        
        for result in results {
            match result {
                Ok(Some(empty_account)) => {
                    info!("Found empty account: {} ({} lamports)", empty_account.address, empty_account.lamports);
                    total_recoverable_lamports += empty_account.lamports;
                    empty_accounts.push(empty_account);
                }
                Ok(None) => {
                    // Account not empty, skip
                }
                Err(e) => {
                    error!("Error checking account: {}", e);
                    // Continue processing other accounts even if one fails
                }
            }
        }

        let recoverable_sol = total_recoverable_lamports as f64 / LAMPORTS_PER_SOL;
        let empty_account_addresses: Vec<String> = empty_accounts
            .iter()
            .map(|acc| acc.address.clone())
            .collect();

        Ok(WalletInfo {
            address: wallet_address.to_string(),
            pubkey,
            total_accounts: total_accounts as u64,
            empty_accounts: empty_accounts.len() as u64,
            recoverable_lamports: total_recoverable_lamports,
            recoverable_sol,
            empty_account_addresses,
            scan_time_ms: 0, // Will be set by caller
        })
    }

    async fn check_empty_account(&self, keyed_account: &solana_client::rpc_response::RpcKeyedAccount, wallet_address: &str) -> Result<Option<EmptyAccount>> {
        let account_pubkey_str = &keyed_account.pubkey;
        let account = &keyed_account.account;
        
        // PROTECTION: Never flag the main wallet address as a recoverable account
        if account_pubkey_str == wallet_address {
            return Ok(None);
        }
        
        let owner_pubkey = Pubkey::from_str(&account.owner)
            .map_err(|_| SolanaRecoverError::InvalidWalletAddress(account.owner.clone()))?;

        // Case 1: Token Account (owned by SPL Token Program or Token-2022 Program)
        if owner_pubkey == spl_token::id() || owner_pubkey == spl_token_2022::id() {
            // Handle both Binary and Json data formats
            match &account.data {
                solana_account_decoder::UiAccountData::Binary(data_str, encoding) => {
                    // Parse the binary data to extract token account info
                    if let Ok(token_account) = self.parse_token_account_from_binary(data_str, encoding) {
                        if token_account.amount == 0 && account.lamports > 0 {
                            return Ok(Some(EmptyAccount {
                                address: account_pubkey_str.clone(),
                                lamports: account.lamports,
                                owner: account.owner.clone(),
                                mint: Some(token_account.mint),
                            }));
                        }
                    }
                }
                solana_account_decoder::UiAccountData::Json(parsed) => {
                    if let Some(info) = parsed.parsed.get("info") {
                        if let Some(token_amount) = info.get("tokenAmount") {
                            if let Some(amount_str) = token_amount.get("amount") {
                                match amount_str.as_str().unwrap_or("0").parse::<u64>() {
                                    Ok(amount) if amount == 0 && account.lamports > 0 => {
                                        let owner = info.get("owner")
                                            .and_then(|o| o.as_str())
                                            .unwrap_or("unknown")
                                            .to_string();
                                        let mint = info.get("mint")
                                            .and_then(|m| m.as_str())
                                            .map(|m| m.to_string());

                                        return Ok(Some(EmptyAccount {
                                            address: account_pubkey_str.clone(),
                                            lamports: account.lamports,
                                            owner,
                                            mint,
                                        }));
                                    }
                                    Ok(_) => {
                                        // Non-zero amount, not empty
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse token amount for {}: {}", account_pubkey_str, e);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    warn!("Unsupported data format for token account: {}", account_pubkey_str);
                }
            }
        } 
        // Case 2: System Account (owned by System Program)
        else if owner_pubkey == solana_program::system_program::id() {
            // A system account is considered "empty" if it only holds its rent-exempt minimum
            // and is not executable and has no data.
            if !account.executable {
                let client = self.connection_pool.get_client().await?;
                let min_rent_exemption = client.get_minimum_balance_for_rent_exemption(
                    account.space.unwrap_or(0) as usize
                ).await?;

                let is_data_empty = match &account.data {
                    solana_account_decoder::UiAccountData::Binary(data_str, _) => data_str.is_empty(),
                    solana_account_decoder::UiAccountData::Json(parsed) => {
                        parsed.parsed.is_null() ||
                        parsed.parsed.as_object().map_or(false, |obj| obj.is_empty()) ||
                        parsed.parsed.as_array().map_or(false, |arr| arr.is_empty())
                    },
                    solana_account_decoder::UiAccountData::LegacyBinary(_) => true,
                };

                // Consider it "empty" if its lamports are close to the rent-exempt minimum
                // and its data is empty. Use >= to catch accounts with slightly more than minimum
                if account.lamports >= min_rent_exemption && is_data_empty {
                    if account.lamports > 0 {
                        return Ok(Some(EmptyAccount {
                            address: account_pubkey_str.clone(),
                            lamports: account.lamports,
                            owner: account.owner.clone(),
                            mint: None, // System accounts don't have a mint
                        }));
                    }
                }
            }
        }
        
        // Case 3: Other program accounts that might be empty and hold recoverable SOL
        // This catches accounts from other programs that might have zero balance but hold rent
        else {
            // Check for OpenBook/Serum OpenOrders accounts specifically
            if owner_pubkey == Pubkey::from_str("opnb2vDkSQsqmY24zQ4DDEZf1V3oEisPZ5bEErLNRsA").unwrap_or_default() ||
               owner_pubkey == Pubkey::from_str("srmqPvvk92GzrcCbKgSGx3mFHTEQuoE3jUuAM6gEKrP").unwrap_or_default() {
                // Handle OpenBook/Serum OpenOrders accounts
                match &account.data {
                    solana_account_decoder::UiAccountData::Binary(data_str, encoding) => {
                        if let Ok(open_orders) = self.parse_open_orders_account_from_binary(data_str, encoding) {
                            // Safety checks: Only flag as recoverable if all balances are zero
                            if open_orders.base_token_free == 0 && 
                               open_orders.quote_token_free == 0 && 
                               open_orders.base_token_total == 0 && 
                               open_orders.quote_token_total == 0 && 
                               account.lamports > 0 {
                                return Ok(Some(EmptyAccount {
                                    address: account_pubkey_str.clone(),
                                    lamports: account.lamports,
                                    owner: account.owner.clone(),
                                    mint: None, // OpenOrders accounts don't have a mint
                                }));
                            }
                        }
                    }
                    _ => {
                        debug!("OpenBook account {} has non-binary data format", account_pubkey_str);
                    }
                }
            }
            
            // For other non-system, non-token accounts, check if they have data and are executable
            // If they're not executable and have minimal data, they might be recoverable
            if !account.executable && account.lamports > 0 {
                let client = self.connection_pool.get_client().await?;
                let min_rent_exemption = client.get_minimum_balance_for_rent_exemption(
                    account.space.unwrap_or(0) as usize
                ).await?;

                // Check if the account holds approximately rent-exempt amount
                // Allow some tolerance for small variations
                let tolerance = min_rent_exemption / 10; // 10% tolerance
                let is_rent_exempt = account.lamports >= min_rent_exemption.saturating_sub(tolerance) && 
                                    account.lamports <= min_rent_exemption.saturating_add(tolerance);

                if is_rent_exempt {
                    // Check if data is empty or minimal
                    let is_data_empty = match &account.data {
                        solana_account_decoder::UiAccountData::Binary(data_str, _) => {
                            data_str.is_empty() || data_str.len() < 50 // Small threshold for "minimal" data
                        },
                        solana_account_decoder::UiAccountData::Json(parsed) => {
                            parsed.parsed.is_null() ||
                            parsed.parsed.as_object().map_or(false, |obj| obj.is_empty()) ||
                            parsed.parsed.as_array().map_or(false, |arr| arr.is_empty())
                        },
                        solana_account_decoder::UiAccountData::LegacyBinary(_) => true,
                    };

                    if is_data_empty {
                        return Ok(Some(EmptyAccount {
                            address: account_pubkey_str.clone(),
                            lamports: account.lamports,
                            owner: account.owner.clone(),
                            mint: None, // Non-token accounts don't have a mint
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    // Helper method to parse token account from binary data
    pub fn parse_token_account_from_binary(&self, data_str: &str, encoding: &UiAccountEncoding) -> Result<TokenAccountInfo> {
        // Decode based on the encoding type
        let decoded_data = match encoding {
            UiAccountEncoding::Base64 => {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD.decode(data_str)
                    .map_err(|_| SolanaRecoverError::InternalError("Failed to decode Base64 data".to_string()))?
            }
            UiAccountEncoding::Base58 => {
                bs58::decode(data_str)
                    .into_vec()
                    .map_err(|_| SolanaRecoverError::InternalError("Failed to decode Base58 data".to_string()))?
            }
            _ => {
                return Err(SolanaRecoverError::InternalError("Unsupported encoding for token account".to_string()));
            }
        };

        // Token account structure (simplified):
        // - 32 bytes: mint (Pubkey)
        // - 32 bytes: owner (Pubkey) 
        // - 8 bytes: amount (u64)
        // - ... other fields we don't need for empty detection
        
        // Increased safety check - ensure we have at least 72 bytes for mint + owner + amount
        if decoded_data.len() < 72 {
            return Err(SolanaRecoverError::InternalError("Invalid token account data length".to_string()));
        }

        // Extract mint (first 32 bytes)
        let mut mint_array = [0u8; 32];
        mint_array.copy_from_slice(&decoded_data[0..32]);
        let mint_pubkey = Pubkey::new_from_array(mint_array);

        // Extract amount (bytes 64-72, after mint and owner)
        let amount_bytes = &decoded_data[64..72];
        let mut amount_array = [0u8; 8];
        amount_array.copy_from_slice(amount_bytes);
        let amount = u64::from_le_bytes(amount_array);

        Ok(TokenAccountInfo {
            mint: mint_pubkey.to_string(),
            amount,
        })
    }

    // Helper method to parse OpenBook OpenOrders account from binary data
    pub fn parse_open_orders_account_from_binary(&self, data_str: &str, encoding: &UiAccountEncoding) -> Result<OpenOrdersAccountInfo> {
        // Decode based on the encoding type
        let decoded_data = match encoding {
            UiAccountEncoding::Base64 => {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD.decode(data_str)
                    .map_err(|_| SolanaRecoverError::InternalError("Failed to decode Base64 data for OpenOrders".to_string()))?
            }
            UiAccountEncoding::Base58 => {
                bs58::decode(data_str)
                    .into_vec()
                    .map_err(|_| SolanaRecoverError::InternalError("Failed to decode Base58 data for OpenOrders".to_string()))?
            }
            _ => {
                return Err(SolanaRecoverError::InternalError("Unsupported encoding for OpenOrders account".to_string()));
            }
        };

        // OpenOrders account structure (simplified for safety checks):
        // - 8 bytes: discriminator
        // - 32 bytes: market (Pubkey)
        // - 32 bytes: owner/authority (Pubkey) - this is what we filter by
        // - 8 bytes: base_token_free (u64)
        // - 8 bytes: base_token_total (u64)
        // - 8 bytes: quote_token_free (u64)
        // - 8 bytes: quote_token_total (u64)
        // - ... other fields we don't need for empty detection
        
        // Safety check - ensure we have at least 96 bytes for the fields we need
        if decoded_data.len() < 96 {
            return Err(SolanaRecoverError::InternalError("Invalid OpenOrders account data length".to_string()));
        }

        // Extract base_token_free (bytes 72-80, after discriminator, market, owner)
        let base_token_free_bytes = &decoded_data[72..80];
        let base_token_free = u64::from_le_bytes(base_token_free_bytes.try_into()
            .map_err(|_| SolanaRecoverError::InternalError("Failed to parse base_token_free".to_string()))?);

        // Extract base_token_total (bytes 80-88)
        let base_token_total_bytes = &decoded_data[80..88];
        let base_token_total = u64::from_le_bytes(base_token_total_bytes.try_into()
            .map_err(|_| SolanaRecoverError::InternalError("Failed to parse base_token_total".to_string()))?);

        // Extract quote_token_free (bytes 88-96)
        let quote_token_free_bytes = &decoded_data[88..96];
        let quote_token_free = u64::from_le_bytes(quote_token_free_bytes.try_into()
            .map_err(|_| SolanaRecoverError::InternalError("Failed to parse quote_token_free".to_string()))?);

        // Extract quote_token_total (bytes 96-104)
        let quote_token_total_bytes = &decoded_data[96..104];
        let quote_token_total = u64::from_le_bytes(quote_token_total_bytes.try_into()
            .map_err(|_| SolanaRecoverError::InternalError("Failed to parse quote_token_total".to_string()))?);

        Ok(OpenOrdersAccountInfo {
            base_token_free,
            base_token_total,
            quote_token_free,
            quote_token_total,
        })
    }
}
