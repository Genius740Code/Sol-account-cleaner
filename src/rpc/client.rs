use crate::core::{Result, SolanaRecoverError};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::{pubkey::Pubkey, commitment_config::CommitmentConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};
use moka::future::Cache;
use base64::Engine;

pub struct RpcClientWrapper {
    client: Arc<RpcClient>,
    rate_limiter: Arc<dyn RateLimiter>,
    request_timeout: Duration,
    rent_cache: Cache<usize, u64>, // Cache for rent exemption queries
}

impl std::fmt::Debug for RpcClientWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcClientWrapper")
            .field("request_timeout", &self.request_timeout)
            .finish()
    }
}

impl RpcClientWrapper {
    pub fn new(client: Arc<RpcClient>, rate_limiter: Arc<dyn RateLimiter>) -> Self {
        let rent_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
            .build();
        
        Self {
            client,
            rate_limiter,
            request_timeout: Duration::from_secs(30),
            rent_cache,
        }
    }
    
    pub fn new_with_url(url: &str, timeout_ms: u64) -> Result<Self> {
        let client = Arc::new(RpcClient::new_with_timeout(
            url.to_string(),
            Duration::from_millis(timeout_ms),
        ));
        let rate_limiter = Arc::new(TokenBucketRateLimiter::new(100));
        
        let rent_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
            .build();
        
        Ok(Self {
            client,
            rate_limiter,
            request_timeout: Duration::from_millis(timeout_ms),
            rent_cache,
        })
    }
    
    pub fn from_url(url: &str, timeout_ms: u64) -> Result<Self> {
        Self::new_with_url(url, timeout_ms)
    }

    pub async fn get_token_accounts(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        self.rate_limiter.acquire().await?;
        
        let token_program_id = spl_token::id();
        let token_2022_program_id = spl_token_2022::id();
        let client = self.client.clone();
        let pubkey = *pubkey;
        let timeout = self.request_timeout;
        
        // Fetch standard Token accounts
        let standard_accounts = tokio::time::timeout(timeout, tokio::task::spawn_blocking({
            let client = client.clone();
            move || {
                client.get_token_accounts_by_owner(
                    &pubkey,
                    TokenAccountsFilter::ProgramId(token_program_id),
                )
            }
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?;
        
        // Fetch Token-2022 accounts
        let token_2022_accounts = tokio::time::timeout(timeout, tokio::task::spawn_blocking({
            let client = client.clone();
            move || {
                client.get_token_accounts_by_owner(
                    &pubkey,
                    TokenAccountsFilter::ProgramId(token_2022_program_id),
                )
            }
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?;
        
        // Combine results from both programs
        let mut all_accounts = standard_accounts;
        all_accounts.extend(token_2022_accounts);
        
        Ok(all_accounts)
    }

    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let pubkey = *pubkey;
        let timeout = self.request_timeout;
        
        Ok(tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.get_balance_with_commitment(&pubkey, CommitmentConfig::confirmed())
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?
        .value)
    }
    
    pub async fn get_health(&self) -> Result<()> {
        let client = self.client.clone();
        let timeout = Duration::from_secs(5);
        
        let _ = tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.get_latest_blockhash()
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Health check timeout".to_string()))?
        .map_err(|_| SolanaRecoverError::NetworkError("Endpoint unhealthy".to_string()))?;
        
        Ok(())
    }
    
    pub async fn get_account_info(&self, pubkey: &Pubkey) -> Result<solana_account_decoder::UiAccount> {
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let pubkey = *pubkey;
        let timeout = self.request_timeout;
        
        tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.get_account_with_commitment(&pubkey, CommitmentConfig::confirmed())
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?
        .value
        .ok_or_else(|| SolanaRecoverError::InternalError("Account not found".to_string()))
        .map(|account| {
            // Create a simple UiAccount representation
            solana_account_decoder::UiAccount {
                lamports: account.lamports,
                data: solana_account_decoder::UiAccountData::Binary(
                    base64::engine::general_purpose::STANDARD.encode(&account.data),
                    solana_account_decoder::UiAccountEncoding::Base64
                ),
                owner: account.owner.to_string(),
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                space: Some(account.data.len() as u64),
            }
        })
    }
    
    pub async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<solana_account_decoder::UiAccount>>> {
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let pubkeys = pubkeys.to_vec();
        let timeout = self.request_timeout;
        
        Ok(tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.get_multiple_accounts_with_commitment(&pubkeys, CommitmentConfig::confirmed())
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?
        .value
        .into_iter()
        .map(|account_opt| account_opt.map(|account| {
            solana_account_decoder::UiAccount {
                lamports: account.lamports,
                data: solana_account_decoder::UiAccountData::Binary(
                    base64::engine::general_purpose::STANDARD.encode(&account.data),
                    solana_account_decoder::UiAccountEncoding::Base64
                ),
                owner: account.owner.to_string(),
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                space: Some(account.data.len() as u64),
            }
        }))
        .collect::<Vec<_>>())
    }
    
    pub async fn send_transaction(&self, transaction: &solana_sdk::transaction::Transaction) -> Result<String> {
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let transaction = transaction.clone();
        let timeout = self.request_timeout;
        
        let result = tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.send_and_confirm_transaction(&transaction)
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Transaction timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(format!("Task join error: {}", e)))?
        .map_err(|e| SolanaRecoverError::RpcClientError(e))?;
        
        Ok(result.to_string())
    }
    
    pub fn get_client(&self) -> Arc<RpcClient> {
        self.client.clone()
    }
    
    pub async fn get_signature_status_with_commitment(&self, signature: &str, commitment: CommitmentConfig) -> Result<Option<bool>> {
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let signature = signature.parse::<solana_sdk::signature::Signature>()
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid signature".to_string()))?;
        let timeout = self.request_timeout;
        
        let result = tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.get_signature_status_with_commitment(&signature, commitment)
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?;
        
        Ok(result.map(|status| status.is_ok()))
    }
    
    pub async fn get_minimum_balance_for_rent_exemption(&self, data_size: usize) -> Result<u64> {
        // Check cache first
        if let Some(cached_value) = self.rent_cache.get(&data_size).await {
            return Ok(cached_value);
        }
        
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let timeout = self.request_timeout;
        let cache = self.rent_cache.clone();
        
        let result = tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.get_minimum_balance_for_rent_exemption(data_size)
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?;
        
        // Cache the result
        cache.insert(data_size, result).await;
        
        Ok(result)
    }
}

#[async_trait::async_trait]
pub trait RateLimiter: Send + Sync {
    async fn acquire(&self) -> Result<()>;
}

pub struct TokenBucketRateLimiter {
    max_tokens: u32,
    tokens: Arc<tokio::sync::Mutex<u32>>,
    refill_interval: Duration,
    last_refill: Arc<tokio::sync::Mutex<Instant>>,
}

impl TokenBucketRateLimiter {
    pub fn new(rps: u32) -> Self {
        Self {
            max_tokens: rps,
            tokens: Arc::new(tokio::sync::Mutex::new(rps)),
            refill_interval: Duration::from_secs(1) / rps as u32,
            last_refill: Arc::new(tokio::sync::Mutex::new(Instant::now())),
        }
    }

    async fn refill_tokens(&self) {
        let mut tokens = self.tokens.lock().await;
        let mut last_refill = self.last_refill.lock().await;
        
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);
        
        if elapsed >= self.refill_interval {
            let tokens_to_add = (elapsed.as_millis() / self.refill_interval.as_millis()) as u32;
            *tokens = (*tokens + tokens_to_add).min(self.max_tokens);
            *last_refill = now;
        }
    }
}

#[async_trait::async_trait]
impl RateLimiter for TokenBucketRateLimiter {
    async fn acquire(&self) -> Result<()> {
        self.refill_tokens().await;
        
        let mut tokens = self.tokens.lock().await;
        if *tokens > 0 {
            *tokens -= 1;
            Ok(())
        } else {
            Err(SolanaRecoverError::RateLimitExceeded)
        }
    }
}
