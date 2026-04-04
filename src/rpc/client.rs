use crate::core::{Result, SolanaRecoverError};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::{pubkey::Pubkey, commitment_config::CommitmentConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct RpcClientWrapper {
    client: Arc<RpcClient>,
    rate_limiter: Arc<dyn RateLimiter>,
}

impl RpcClientWrapper {
    pub fn new(client: Arc<RpcClient>, rate_limiter: Arc<dyn RateLimiter>) -> Self {
        Self {
            client,
            rate_limiter,
        }
    }

    pub async fn get_token_accounts(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        self.rate_limiter.acquire().await?;
        
        let token_program_id = spl_token::id();
        let client = self.client.clone();
        let pubkey = *pubkey;
        
        tokio::task::spawn_blocking(move || {
            client.get_token_accounts_by_owner(
                &pubkey,
                TokenAccountsFilter::ProgramId(token_program_id),
            )
        }).await
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)
    }

    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.rate_limiter.acquire().await?;
        
        let client = self.client.clone();
        let pubkey = *pubkey;
        
        Ok(tokio::task::spawn_blocking(move || {
            client.get_balance_with_commitment(&pubkey, CommitmentConfig::confirmed())
        }).await
        .map_err(|e| SolanaRecoverError::InternalError(e.to_string()))?
        .map_err(SolanaRecoverError::RpcClientError)?
        .value)
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
            Err(SolanaRecoverError::RateLimitExceeded("Rate limit exceeded".to_string()))
        }
    }
}
