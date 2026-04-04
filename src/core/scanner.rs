use crate::core::{WalletInfo, EmptyAccount, Result, SolanaRecoverError, ScanResult, ScanStatus};
use crate::rpc::{ConnectionPool, RpcClientWrapper};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use chrono::Utc;

const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

#[derive(Clone)]
pub struct WalletScanner {
    connection_pool: Arc<ConnectionPool>,
}

impl WalletScanner {
    pub fn new(connection_pool: Arc<ConnectionPool>) -> Self {
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
                let _scan_time = start_time.elapsed().as_millis() as u64;
                let mut result = scan_result;
                result.status = ScanStatus::Completed;
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

    async fn scan_wallet_internal(&self, wallet_address: &str) -> Result<WalletInfo> {
        let pubkey = Pubkey::from_str(wallet_address)
            .map_err(|_| SolanaRecoverError::InvalidWalletAddress(wallet_address.to_string()))?;

        let client = self.connection_pool.get_client().await?;
        let rate_limiter = std::sync::Arc::new(
            crate::rpc::TokenBucketRateLimiter::new(10)
        );
        let wrapper = RpcClientWrapper::new(client, rate_limiter);

        let token_accounts = wrapper.get_token_accounts(&pubkey).await?;
        let total_accounts = token_accounts.len();

        let mut empty_accounts: Vec<EmptyAccount> = Vec::new();
        let mut total_recoverable_lamports: u64 = 0;

        for keyed_account in token_accounts {
            if let Some(empty_account) = self.check_empty_account(&keyed_account).await? {
                total_recoverable_lamports += empty_account.lamports;
                empty_accounts.push(empty_account);
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

    async fn check_empty_account(&self, keyed_account: &solana_client::rpc_response::RpcKeyedAccount) -> Result<Option<EmptyAccount>> {
        let account_data = &keyed_account.account.data;

        if let solana_account_decoder::UiAccountData::Json(parsed) = account_data {
            if let Some(info) = parsed.parsed.get("info") {
                if let Some(token_amount) = info.get("tokenAmount") {
                    if let Some(amount_str) = token_amount.get("amount") {
                        let amount: u64 = amount_str
                            .as_str()
                            .unwrap_or("1")
                            .parse()
                            .unwrap_or(1);

                        if amount == 0 {
                            let owner = info.get("owner")
                                .and_then(|o| o.as_str())
                                .unwrap_or("unknown")
                                .to_string();

                            let mint = info.get("mint")
                                .and_then(|m| m.as_str())
                                .map(|m| m.to_string());

                            return Ok(Some(EmptyAccount {
                                address: keyed_account.pubkey.to_string(),
                                lamports: keyed_account.account.lamports,
                                owner,
                                mint,
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}
