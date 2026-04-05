use crate::core::{Result, SolanaRecoverError};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::{pubkey::Pubkey, commitment_config::CommitmentConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RpcClientWrapper {
    client: Arc<RpcClient>,
    rate_limiter: Arc<dyn RateLimiter>,
    request_timeout: Duration,
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
        Self {
            client,
            rate_limiter,
            request_timeout: Duration::from_secs(30),
        }
    }
    
    pub fn new_with_url(url: &str, timeout_ms: u64) -> Result<Self> {
        let client = Arc::new(RpcClient::new_with_timeout(
            url.to_string(),
            Duration::from_millis(timeout_ms),
        ));
        let rate_limiter = Arc::new(TokenBucketRateLimiter::new(100));
        
        Ok(Self {
            client,
            rate_limiter,
            request_timeout: Duration::from_millis(timeout_ms),
        })
    }
    
    pub fn from_url(url: &str, timeout_ms: u64) -> Result<Self> {
        Self::new_with_url(url, timeout_ms)
    }

    pub async fn get_token_accounts(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        self.rate_limiter.acquire().await?;
        
        let token_program_id = spl_token::id();
        let client = self.client.clone();
        let pubkey = *pubkey;
        let timeout = self.request_timeout;
        
        tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            client.get_token_accounts_by_owner(
                &pubkey,
                TokenAccountsFilter::ProgramId(token_program_id),
            )
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)
    }

    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let pubkey = *pubkey;
        let timeout = self.request_timeout;
        
        tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            Ok(client.get_balance_with_commitment(&pubkey, CommitmentConfig::confirmed())
            .map_err(SolanaRecoverError::RpcClientError)?
            .value)
        })).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
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
    
    pub async fn get_account_info(&self, pubkey: &Pubkey) -> Result<solana_sdk::account::Account> {
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
        .ok_or_else(|| SolanaRecoverError::RpcClientError(
            std::io::Error::new(std::io::ErrorKind::Other, "Account not found".to_string()).into()
        ))
    }
    
    pub async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<solana_sdk::account::Account>>> {
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
        .value)
    }
    
    pub async fn send_transaction(&self, _transaction_data: &[u8]) -> Result<String> {
        // TODO: Implement proper transaction sending
        Err(SolanaRecoverError::InternalError("Transaction sending not yet implemented".to_string()))
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
