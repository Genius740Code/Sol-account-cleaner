use crate::core::{Result, SolanaRecoverError};
use crate::storage::{HierarchicalCache, HierarchicalCacheConfig};
use crate::config::ProgramIds;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_client::rpc_filter::{RpcFilterType, Memcmp, MemcmpEncodedBytes};
use solana_client::rpc_config::{RpcProgramAccountsConfig, RpcAccountInfoConfig};
use solana_sdk::{pubkey::Pubkey, commitment_config::CommitmentConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use moka::future::Cache;
use base64::Engine;
use tracing::{debug, warn};
use async_trait::async_trait;
use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;

pub struct RpcClientWrapper {
    pub client: Arc<RpcClient>,
    pub async_client: Arc<AsyncRpcClient>,
    rate_limiter: Arc<dyn RateLimiter>,
    request_timeout: Duration,
    rent_cache: Cache<usize, u64>, // Cache for rent exemption queries
    hierarchical_cache: Option<Arc<HierarchicalCache>>, // Enhanced multi-tier cache
    program_ids: Arc<ProgramIds>, // Secure program ID configuration
}

impl std::fmt::Debug for RpcClientWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcClientWrapper")
            .field("request_timeout", &self.request_timeout)
            .finish()
    }
}

impl RpcClientWrapper {
    pub fn new(client: Arc<RpcClient>, rate_limiter: Arc<dyn RateLimiter>) -> Result<Self> {
        let program_ids = Arc::new(ProgramIds::default());
        let rent_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
            .build();
        
        // Create async client from the sync client's URL
        let async_client = Arc::new(AsyncRpcClient::new(client.url()));
        
        Ok(Self {
            client,
            async_client,
            rate_limiter,
            request_timeout: Duration::from_secs(30),
            rent_cache,
            hierarchical_cache: None,
            program_ids,
        })
    }
    
    pub fn new_with_url(url: &str, timeout_ms: u64) -> Result<Self> {
        let client = Arc::new(RpcClient::new_with_timeout(
            url.to_string(),
            Duration::from_millis(timeout_ms),
        ));
        let async_client = Arc::new(AsyncRpcClient::new(url.to_string()));
        let rate_limiter = Arc::new(TokenBucketRateLimiter::new(100));
        let program_ids = Arc::new(ProgramIds::default());
        
        let rent_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
            .build();
        
        Ok(Self {
            client,
            async_client,
            rate_limiter,
            request_timeout: Duration::from_millis(timeout_ms),
            rent_cache,
            hierarchical_cache: None,
            program_ids,
        })
    }
    
    pub fn from_url(url: &str, timeout_ms: u64) -> Result<Self> {
        Self::new_with_url(url, timeout_ms)
    }
    
    pub async fn with_hierarchical_cache(mut self, cache_config: HierarchicalCacheConfig) -> Result<Self> {
        let cache = HierarchicalCache::new(cache_config).await?;
        self.hierarchical_cache = Some(Arc::new(cache));
        Ok(self)
    }
    
    pub fn set_hierarchical_cache(&mut self, cache: Arc<HierarchicalCache>) {
        self.hierarchical_cache = Some(cache);
    }

    /// Generate efficient cache key without string allocation
    fn generate_cache_key(&self, prefix: &str, pubkey: &Pubkey, extra: Option<u64>) -> u64 {
        let mut hasher = DefaultHasher::new();
        prefix.hash(&mut hasher);
        pubkey.hash(&mut hasher);
        if let Some(extra) = extra {
            hasher.write_u64(extra);
        }
        hasher.finish()
    }

    /// Ultra-fast token accounts retrieval with optimized batching
    pub async fn get_token_accounts_by_owner_ultra_fast(
        &self,
        pubkey: &Pubkey,
        batch_size: usize,
    ) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        let cache_key = self.generate_cache_key("token_accounts_ultra", pubkey, Some(batch_size as u64));
        
        // Try cache first
        if let Some(ref cache) = self.hierarchical_cache {
            if let Ok(Some(cached_accounts)) = cache.get::<Vec<solana_client::rpc_response::RpcKeyedAccount>>(&cache_key.to_string()).await {
                debug!("Cache hit for ultra-fast token accounts of {}", pubkey);
                return Ok(cached_accounts);
            }
        }
        
        // Use optimized config for ultra-fast retrieval with async client
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                    0,
                    pubkey.to_string().as_bytes().to_vec(),
                )),
            ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                data_slice: None,
                commitment: Some(solana_sdk::commitment_config::CommitmentConfig::confirmed()),
                min_context_slot: None,
            },
            with_context: Some(false),
        };
        
        // Rate limit check
        self.rate_limiter.acquire().await?;
        
        // Ultra-fast async RPC call with timeout
        let start_time = Instant::now();
        let accounts = tokio::time::timeout(self.request_timeout, 
            self.async_client.get_program_accounts_with_config(
                &spl_token::id(),
                config,
            )
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?;
        
        let response_time = start_time.elapsed();
        debug!("Retrieved {} token accounts for {} in {:?}", accounts.len(), pubkey, response_time);
        
        // Convert to RpcKeyedAccount format
        let keyed_accounts: Vec<solana_client::rpc_response::RpcKeyedAccount> = accounts.into_iter()
            .map(|(pubkey, account)| solana_client::rpc_response::RpcKeyedAccount {
                pubkey: pubkey.to_string(),
                account: solana_account_decoder::UiAccount::encode(
                    &pubkey,
                    &account,
                    solana_account_decoder::UiAccountEncoding::Binary,
                    None,
                    None,
                ),
            })
            .collect();
        
        // Cache the result
        if let Some(ref cache) = self.hierarchical_cache {
            let _ = cache.set(&cache_key.to_string(), &keyed_accounts).await;
        }
        
        Ok(keyed_accounts)
    }

    pub async fn get_all_recoverable_accounts(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        let cache_key = self.generate_cache_key("recoverable_accounts", pubkey, None);
        
        // Try hierarchical cache first
        if let Some(ref cache) = self.hierarchical_cache {
            if let Ok(Some(cached_accounts)) = cache.get::<Vec<solana_client::rpc_response::RpcKeyedAccount>>(&cache_key.to_string()).await {
                debug!("Cache hit for recoverable accounts of {}", pubkey);
                return Ok(cached_accounts);
            }
        }
        
        // Cache miss - fetch from RPC
        let mut all_accounts = self.get_token_accounts(pubkey).await?;
        
        // Add OpenBook accounts
        let openbook_accounts = self.get_openbook_accounts(pubkey).await?;
        all_accounts.extend(openbook_accounts);
        
        // Cache the result
        if let Some(ref cache) = self.hierarchical_cache {
            if let Err(e) = cache.set(&cache_key.to_string(), &all_accounts).await {
                warn!("Failed to cache recoverable accounts for {}: {}", pubkey, e);
            } else {
                debug!("Cached recoverable accounts for {}", pubkey);
            }
        }
        
        Ok(all_accounts)
    }

    pub async fn get_openbook_accounts(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        self.rate_limiter.acquire().await?;
        
        // Use validated program IDs from configuration
        let openbook_v2_id = self.program_ids.openbook_v2;
        let serum_dex_id = self.program_ids.serum_dex;
        
        // Fetch OpenBook V2 accounts asynchronously
        let openbook_v2_accounts = {
            self.rate_limiter.acquire().await?;
            
            let memcmp_filter = Memcmp::new(
                45, // Offset where authority sits in OpenOrders account data
                MemcmpEncodedBytes::Bytes(pubkey.to_bytes().to_vec()),
            );
            
            let config = RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(memcmp_filter)]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                    data_slice: None,
                    commitment: Some(CommitmentConfig::confirmed()),
                    min_context_slot: None,
                },
                with_context: None,
            };
            
            tokio::time::timeout(self.request_timeout,
                self.async_client.get_program_accounts_with_config(&openbook_v2_id, config)
            ).await
            .map_err(|_| SolanaRecoverError::NetworkError("OpenBook V2 request timeout".to_string()))?
            .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?
        };
        
        // Fetch Serum DEX accounts asynchronously
        let serum_dex_accounts = {
            self.rate_limiter.acquire().await?;
            
            let memcmp_filter = Memcmp::new(
                45, // Offset where authority sits in OpenOrders account data
                MemcmpEncodedBytes::Bytes(pubkey.to_bytes().to_vec()),
            );
            
            let config = RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(memcmp_filter)]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                    data_slice: None,
                    commitment: Some(CommitmentConfig::confirmed()),
                    min_context_slot: None,
                },
                with_context: None,
            };
            
            tokio::time::timeout(self.request_timeout,
                self.async_client.get_program_accounts_with_config(&serum_dex_id, config)
            ).await
            .map_err(|_| SolanaRecoverError::NetworkError("Serum DEX request timeout".to_string()))?
            .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?
        };
        
        // Convert program accounts to RpcKeyedAccount format
        let mut all_openbook_accounts = Vec::new();
        
        // Process OpenBook V2 accounts
        for (pubkey, account) in openbook_v2_accounts {
            let ui_account = solana_account_decoder::UiAccount {
                lamports: account.lamports,
                data: solana_account_decoder::UiAccountData::Binary(
                    base64::engine::general_purpose::STANDARD.encode(&account.data),
                    solana_account_decoder::UiAccountEncoding::Base64
                ),
                owner: openbook_v2_id.to_string(),
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                space: Some(account.data.len() as u64),
            };
            
            // Convert to the format expected by the scanner
            all_openbook_accounts.push(solana_client::rpc_response::RpcKeyedAccount {
                pubkey: pubkey.to_string(),
                account: ui_account,
            });
        }
        
        // Process Serum DEX accounts
        for (pubkey, account) in serum_dex_accounts {
            let ui_account = solana_account_decoder::UiAccount {
                lamports: account.lamports,
                data: solana_account_decoder::UiAccountData::Binary(
                    base64::engine::general_purpose::STANDARD.encode(&account.data),
                    solana_account_decoder::UiAccountEncoding::Base64
                ),
                owner: serum_dex_id.to_string(),
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                space: Some(account.data.len() as u64),
            };
            
            all_openbook_accounts.push(solana_client::rpc_response::RpcKeyedAccount {
                pubkey: pubkey.to_string(),
                account: ui_account,
            });
        }
        
        Ok(all_openbook_accounts)
    }

    pub async fn get_token_accounts(&self, pubkey: &Pubkey) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        let cache_key = self.generate_cache_key("token_accounts", pubkey, None);
        
        // Try hierarchical cache first
        if let Some(ref cache) = self.hierarchical_cache {
            if let Ok(Some(cached_accounts)) = cache.get::<Vec<solana_client::rpc_response::RpcKeyedAccount>>(&cache_key.to_string()).await {
                debug!("Cache hit for token accounts of {}", pubkey);
                return Ok(cached_accounts);
            }
        }
        
        self.rate_limiter.acquire().await?;
        
        let token_program_id = spl_token::id();
        let token_2022_program_id = spl_token_2022::id();
        
        // Fetch standard Token accounts asynchronously
        let standard_accounts = {
            tokio::time::timeout(self.request_timeout,
                self.async_client.get_token_accounts_by_owner(
                    pubkey,
                    TokenAccountsFilter::ProgramId(token_program_id),
                )
            ).await
            .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
            .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?
        };
        
        // Fetch Token-2022 accounts asynchronously
        let token_2022_accounts = {
            self.rate_limiter.acquire().await?;
            tokio::time::timeout(self.request_timeout,
                self.async_client.get_token_accounts_by_owner(
                    pubkey,
                    TokenAccountsFilter::ProgramId(token_2022_program_id),
                )
            ).await
            .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
            .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?
        };
        
        // Combine results from both programs
        let mut all_accounts = standard_accounts;
        all_accounts.extend(token_2022_accounts);
        
        // Cache the result
        if let Some(ref cache) = self.hierarchical_cache {
            if let Err(e) = cache.set(&cache_key.to_string(), &all_accounts).await {
                warn!("Failed to cache token accounts for {}: {}", pubkey, e);
            } else {
                debug!("Cached token accounts for {}", pubkey);
            }
        }
        
        Ok(all_accounts)
    }

    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        let cache_key = self.generate_cache_key("balance", pubkey, None);
        
        // Try hierarchical cache first (with shorter TTL for balance)
        if let Some(ref cache) = self.hierarchical_cache {
            if let Ok(Some(cached_balance)) = cache.get::<u64>(&cache_key.to_string()).await {
                debug!("Cache hit for balance of {}", pubkey);
                return Ok(cached_balance);
            }
        }
        
        self.rate_limiter.acquire().await?;
        
        let balance = tokio::time::timeout(self.request_timeout,
            self.async_client.get_balance_with_commitment(pubkey, CommitmentConfig::confirmed())
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?
        .value;
        
        // Cache the result
        if let Some(ref cache) = self.hierarchical_cache {
            if let Err(e) = cache.set(&cache_key.to_string(), &balance).await {
                warn!("Failed to cache balance for {}: {}", pubkey, e);
            } else {
                debug!("Cached balance for {}", pubkey);
            }
        }
        
        Ok(balance)
    }
    
    pub async fn get_health(&self) -> Result<()> {
        let _ = tokio::time::timeout(Duration::from_secs(5),
            self.async_client.get_latest_blockhash()
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Health check timeout".to_string()))?
        .map_err(|_| SolanaRecoverError::NetworkError("Endpoint unhealthy".to_string()))?;
        
        Ok(())
    }
    
    pub async fn get_account_info(&self, pubkey: &Pubkey) -> Result<solana_account_decoder::UiAccount> {
        let cache_key = self.generate_cache_key("account_info", pubkey, None);
        
        // Try hierarchical cache first
        if let Some(ref cache) = self.hierarchical_cache {
            if let Ok(Some(cached_account)) = cache.get::<solana_account_decoder::UiAccount>(&cache_key.to_string()).await {
                debug!("Cache hit for account info of {}", pubkey);
                return Ok(cached_account);
            }
        }
        
        self.rate_limiter.acquire().await?;
        
        let account = tokio::time::timeout(self.request_timeout,
            self.async_client.get_account_with_commitment(pubkey, CommitmentConfig::confirmed())
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?
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
        })?;
        
        // Cache the result
        if let Some(ref cache) = self.hierarchical_cache {
            if let Err(e) = cache.set(&cache_key.to_string(), &account).await {
                warn!("Failed to cache account info for {}: {}", pubkey, e);
            } else {
                debug!("Cached account info for {}", pubkey);
            }
        }
        
        Ok(account)
    }
    
    pub async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<solana_account_decoder::UiAccount>>> {
        self.rate_limiter.acquire().await?;
        
        Ok(tokio::time::timeout(self.request_timeout,
            self.async_client.get_multiple_accounts_with_commitment(pubkeys, CommitmentConfig::confirmed())
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?
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
        
        let result = tokio::time::timeout(self.request_timeout,
            self.async_client.send_and_confirm_transaction(transaction)
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Transaction timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?;
        
        Ok(result.to_string())
    }
    
    pub fn get_client(&self) -> Arc<RpcClient> {
        self.client.clone()
    }
    
    pub async fn get_signature_status_with_commitment(&self, signature: &str, commitment: CommitmentConfig) -> Result<Option<bool>> {
        self.rate_limiter.acquire().await?;
        
        let signature = signature.parse::<solana_sdk::signature::Signature>()
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid signature".to_string()))?;
        let result = tokio::time::timeout(self.request_timeout,
            self.async_client.get_signature_status_with_commitment(&signature, commitment)
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?;
        
        Ok(result.map(|status| status.is_ok()))
    }
    
    pub async fn get_minimum_balance_for_rent_exemption(&self, data_size: usize) -> Result<u64> {
        // Check cache first
        if let Some(cached_value) = self.rent_cache.get(&data_size).await {
            return Ok(cached_value);
        }
        
        self.rate_limiter.acquire().await?;
        
        let result = tokio::time::timeout(self.request_timeout,
            self.async_client.get_minimum_balance_for_rent_exemption(data_size)
        ).await
        .map_err(|_| SolanaRecoverError::NetworkError("Request timeout".to_string()))?
        .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?;
        
        // Cache the result
        self.rent_cache.insert(data_size, result).await;
        
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
            Err(SolanaRecoverError::RateLimitExceeded("Rate limit exceeded".to_string()))
        }
    }
}
