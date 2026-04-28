use crate::core::{Result, SolanaRecoverError};
use crate::rpc::{ConnectionPoolTrait, RpcClientWrapper};
use solana_account_decoder::UiAccount;
use base64::Engine;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use futures::stream::{self, StreamExt};
use serde::{Serialize, Deserialize};

/// Batch RPC client for optimized account operations
pub struct BatchRpcClient {
    connection_pool: Arc<dyn ConnectionPoolTrait>,
    config: BatchConfig,
    semaphore: Arc<Semaphore>,
    metrics: Arc<tokio::sync::RwLock<BatchMetrics>>,
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub max_batch_size: usize,
    pub max_concurrent_batches: usize,
    pub batch_timeout: Duration,
    pub retry_policy: RetryPolicy,
    pub enable_compression: bool,
    pub enable_multiplexing: bool,
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct BatchMetrics {
    pub total_batches: u64,
    pub successful_batches: u64,
    pub failed_batches: u64,
    pub total_requests: u64,
    pub avg_batch_size: f64,
    pub avg_response_time_ms: f64,
    pub avg_efficiency_ratio: f64, // requests_per_rpc
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAccountInfo {
    pub pubkey: String,
    pub account: Option<UiAccount>,
    pub lamports: Option<u64>,
    pub owner: Option<String>,
    pub executable: Option<bool>,
    pub space: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct BatchRentExemptionResult {
    pub account_sizes: HashMap<usize, u64>,
    pub total_requests: usize,
    pub response_time_ms: u64,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            max_concurrent_batches: 10,
            batch_timeout: Duration::from_secs(30),
            retry_policy: RetryPolicy::default(),
            enable_compression: true,
            enable_multiplexing: true,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl BatchRpcClient {
    pub fn new(connection_pool: Arc<dyn ConnectionPoolTrait>, config: BatchConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_batches));
        
        Self {
            connection_pool,
            config,
            semaphore,
            metrics: Arc::new(tokio::sync::RwLock::new(BatchMetrics::default())),
        }
    }

    /// Batch get multiple accounts efficiently
    pub async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> Result<Vec<Option<UiAccount>>> {
        if pubkeys.is_empty() {
            return Ok(Vec::new());
        }

        let start_time = Instant::now();
        
        // Split into batches if necessary
        let batches = self.create_batches(pubkeys, self.config.max_batch_size);
        let mut all_results = Vec::with_capacity(pubkeys.len());
        
        // Process batches concurrently with semaphore control
        let batch_futures = batches.into_iter().map(|batch| {
            let semaphore = self.semaphore.clone();
            let connection_pool = self.connection_pool.clone();
            let config = self.config.clone();
            
            async move {
                let _permit = semaphore.acquire().await
                    .map_err(|_| SolanaRecoverError::ConnectionPoolExhausted)?;
                
                Self::process_account_batch(connection_pool, &batch, config).await
            }
        });

        let batch_results = stream::iter(batch_futures)
            .buffer_unordered(self.config.max_concurrent_batches)
            .collect::<Vec<_>>()
            .await;

        // Collect results and handle errors
        for batch_result in batch_results {
            match batch_result {
                Ok(results) => all_results.extend(results),
                Err(e) => {
                    tracing::error!("Batch account request failed: {}", e);
                    // Continue with other batches even if one fails
                }
            }
        }

        // Update metrics
        let response_time = start_time.elapsed();
        self.update_metrics(pubkeys.len(), 1, response_time.as_millis() as f64, true).await;

        Ok(all_results)
    }

    /// Batch get rent exemption for multiple account sizes
    pub async fn get_multiple_rent_exemptions(
        &self,
        account_sizes: &[usize],
    ) -> Result<BatchRentExemptionResult> {
        if account_sizes.is_empty() {
            return Ok(BatchRentExemptionResult {
                account_sizes: HashMap::new(),
                total_requests: 0,
                response_time_ms: 0,
            });
        }

        let start_time = Instant::now();
        let mut results = HashMap::new();
        
        // Group unique sizes to avoid duplicate requests
        let unique_sizes: Vec<usize> = account_sizes.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Split into batches
        let batches = self.create_size_batches(&unique_sizes, self.config.max_batch_size);
        
        // Process batches concurrently
        let batch_futures = batches.into_iter().map(|batch| {
            let semaphore = self.semaphore.clone();
            let connection_pool = self.connection_pool.clone();
            let config = self.config.clone();
            
            async move {
                let _permit = semaphore.acquire().await
                    .map_err(|_| SolanaRecoverError::ConnectionPoolExhausted)?;
                
                Self::process_rent_exemption_batch(connection_pool, &batch, config).await
            }
        });

        let batch_results = stream::iter(batch_futures)
            .buffer_unordered(self.config.max_concurrent_batches)
            .collect::<Vec<_>>()
            .await;

        // Collect results
        for batch_result in batch_results {
            match batch_result {
                Ok(batch_results_map) => {
                    results.extend(batch_results_map);
                }
                Err(e) => {
                    tracing::error!("Batch rent exemption request failed: {}", e);
                }
            }
        }

        let response_time = start_time.elapsed();
        
        // Fill in original request order
        let mut ordered_results = HashMap::new();
        for &size in account_sizes {
            if let Some(&rent_exemption) = results.get(&size) {
                ordered_results.insert(size, rent_exemption);
            }
        }

        Ok(BatchRentExemptionResult {
            account_sizes: ordered_results,
            total_requests: account_sizes.len(),
            response_time_ms: response_time.as_millis() as u64,
        })
    }

    /// Optimized batch operation for wallet scanning
    pub async fn scan_wallet_accounts_optimized(
        &self,
        wallet_pubkey: &Pubkey,
    ) -> Result<Vec<solana_client::rpc_response::RpcKeyedAccount>> {
        let start_time = Instant::now();
        
        // Get token accounts with larger page size for efficiency
        let client = self.connection_pool.get_client().await?;
        
        // Use getProgramAccounts with optimized filters
        let token_accounts = client.get_token_accounts_with_config(wallet_pubkey).await?;
        
        // Get system accounts
        let system_accounts = client.get_system_accounts_with_config(wallet_pubkey).await?;
        
        // Combine results
        let mut all_accounts = Vec::with_capacity(token_accounts.len() + system_accounts.len());
        all_accounts.extend(token_accounts);
        all_accounts.extend(system_accounts);
        
        tracing::debug!(
            "Retrieved {} accounts for wallet in {}ms",
            all_accounts.len(),
            start_time.elapsed().as_millis()
        );
        
        Ok(all_accounts)
    }

    /// Create batches from pubkeys
    fn create_batches(&self, items: &[Pubkey], batch_size: usize) -> Vec<Vec<Pubkey>> {
        items.chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect()
    }

    /// Create batches from account sizes
    fn create_size_batches(&self, items: &[usize], batch_size: usize) -> Vec<Vec<usize>> {
        items.chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect()
    }

    /// Process a single batch of account requests
    async fn process_account_batch(
        connection_pool: Arc<dyn ConnectionPoolTrait>,
        pubkeys: &[Pubkey],
        config: BatchConfig,
    ) -> Result<Vec<Option<UiAccount>>> {
        let client = connection_pool.get_client().await?;
        
        // Convert pubkeys to strings for the RPC call
        let _pubkey_strings: Vec<String> = pubkeys
            .iter()
            .map(|pk| pk.to_string())
            .collect();

        // Execute batch request with retry logic
        let mut attempt = 0;
        loop {
            match client.get_multiple_accounts(&pubkeys).await {
                Ok(accounts) => return Ok(accounts),
                Err(e) if attempt < config.retry_policy.max_retries => {
                    attempt += 1;
                    let delay = Self::calculate_retry_delay(attempt, &config.retry_policy);
                    tracing::warn!(
                        "Batch account request failed (attempt {}/{}), retrying in {:?}: {}",
                        attempt,
                        config.retry_policy.max_retries,
                        delay,
                        e
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Process a single batch of rent exemption requests
    async fn process_rent_exemption_batch(
        connection_pool: Arc<dyn ConnectionPoolTrait>,
        account_sizes: &[usize],
        config: BatchConfig,
    ) -> Result<HashMap<usize, u64>> {
        let client = connection_pool.get_client().await?;
        let mut results = HashMap::new();
        
        // Process each size in the batch
        for &size in account_sizes {
            let mut attempt = 0;
            loop {
                match client.get_minimum_balance_for_rent_exemption(size).await {
                    Ok(rent_exemption) => {
                        results.insert(size, rent_exemption);
                        break;
                    }
                    Err(e) if attempt < config.retry_policy.max_retries => {
                        attempt += 1;
                        let delay = Self::calculate_retry_delay(attempt, &config.retry_policy);
                        tracing::warn!(
                            "Rent exemption request failed for size {} (attempt {}/{}), retrying in {:?}: {}",
                            size,
                            attempt,
                            config.retry_policy.max_retries,
                            delay,
                            e
                        );
                        tokio::time::sleep(delay).await;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        
        Ok(results)
    }

    /// Calculate retry delay with exponential backoff and jitter
    fn calculate_retry_delay(attempt: u32, policy: &RetryPolicy) -> Duration {
        let base_delay = policy.base_delay.as_millis() as f64;
        let multiplier = policy.backoff_multiplier.powi(attempt as i32);
        let delay_ms = base_delay * multiplier;
        
        // Apply jitter if enabled
        let final_delay_ms = if policy.jitter {
            delay_ms * (0.5 + rand::random::<f64>() * 0.5) // 50% to 100% of base delay
        } else {
            delay_ms
        };
        
        let delay = Duration::from_millis(final_delay_ms as u64);
        std::cmp::min(delay, policy.max_delay)
    }

    /// Update batch metrics
    async fn update_metrics(
        &self,
        total_requests: usize,
        batch_count: usize,
        response_time_ms: f64,
        success: bool,
    ) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_requests += total_requests as u64;
        metrics.total_batches += batch_count as u64;
        
        if success {
            metrics.successful_batches += batch_count as u64;
        } else {
            metrics.failed_batches += batch_count as u64;
        }
        
        // Update average batch size
        let total_batches = metrics.total_batches;
        metrics.avg_batch_size = 
            (metrics.avg_batch_size * (total_batches - 1) as f64 + total_requests as f64) 
            / total_batches as f64;
        
        // Update average response time
        metrics.avg_response_time_ms = 
            (metrics.avg_response_time_ms * (total_batches - 1) as f64 + response_time_ms) 
            / total_batches as f64;
        
        // Calculate efficiency ratio (requests per RPC call)
        metrics.avg_efficiency_ratio = total_requests as f64 / batch_count as f64;
    }

    /// Get current batch metrics
    pub async fn get_metrics(&self) -> BatchMetrics {
        let metrics = self.metrics.read().await;
        BatchMetrics {
            total_batches: metrics.total_batches,
            successful_batches: metrics.successful_batches,
            failed_batches: metrics.failed_batches,
            total_requests: metrics.total_requests,
            avg_batch_size: metrics.avg_batch_size,
            avg_response_time_ms: metrics.avg_response_time_ms,
            avg_efficiency_ratio: metrics.avg_efficiency_ratio,
        }
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = BatchMetrics::default();
    }
}

/// Extension trait for RpcClientWrapper to support batch operations
pub trait BatchRpcOperations {
    fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> impl std::future::Future<Output = Result<Vec<Option<UiAccount>>>> + Send;
    
    fn get_token_accounts_with_config(
        &self,
        pubkey: &Pubkey,
    ) -> impl std::future::Future<Output = Result<Vec<solana_client::rpc_response::RpcKeyedAccount>>> + Send;
    
    fn get_system_accounts_with_config(
        &self,
        pubkey: &Pubkey,
    ) -> impl std::future::Future<Output = Result<Vec<solana_client::rpc_response::RpcKeyedAccount>>> + Send;
}

impl BatchRpcOperations for RpcClientWrapper {
    fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> impl std::future::Future<Output = Result<Vec<Option<UiAccount>>>> + Send {
        async move {
        // Convert pubkeys to strings for the RPC call
        let pubkey_strings: Vec<String> = pubkeys
            .iter()
            .map(|pk| pk.to_string())
            .collect();
        
        // Convert string pubkeys to Pubkey structs
        let pubkeys: std::result::Result<Vec<Pubkey>, _> = pubkey_strings
            .iter()
            .map(|s| s.parse::<Pubkey>())
            .collect();
        
        let pubkeys = pubkeys.map_err(|_| SolanaRecoverError::InvalidInput("Invalid pubkey format".to_string()))?;
        
        // Use the underlying Solana client for batch operations
        let accounts = self.client.get_multiple_accounts(&pubkeys)
            .map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?;
        
        // Convert Account results to UiAccount results
        let ui_accounts: Vec<Option<UiAccount>> = accounts
            .into_iter()
            .map(|account_opt| account_opt.map(|account| UiAccount {
                lamports: account.lamports,
                data: solana_account_decoder::UiAccountData::Binary(base64::engine::general_purpose::STANDARD.encode(&account.data), solana_account_decoder::UiAccountEncoding::Base64),
                owner: account.owner.to_string(),
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                space: Some(account.data.len() as u64),
            }))
            .collect();
        
        Ok(ui_accounts)
        }
    }

    fn get_token_accounts_with_config(
        &self,
        pubkey: &Pubkey,
    ) -> impl std::future::Future<Output = Result<Vec<solana_client::rpc_response::RpcKeyedAccount>>> + Send {
        async move {
        // Get all token accounts owned by this wallet
        let token_mints = vec![
            spl_token::id().to_string(),
            spl_token_2022::id().to_string(),
        ];

        let mut all_token_accounts = Vec::new();
        
        for mint_str in token_mints {
            let mint = mint_str.parse::<Pubkey>()
                .map_err(|_| SolanaRecoverError::InvalidInput("Invalid mint address".to_string()))?;
            let accounts = self.client.get_program_accounts_with_config(
                &mint,
                solana_client::rpc_config::RpcProgramAccountsConfig {
                    filters: Some(vec![
                        solana_client::rpc_filter::RpcFilterType::Memcmp(
                            solana_client::rpc_filter::Memcmp::new_raw_bytes(
                                32, // owner offset in token account
                                pubkey.as_ref().to_vec(),
                            ),
                        ),
                    ]),
                    account_config: solana_client::rpc_config::RpcAccountInfoConfig {
                        encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                        data_slice: None,
                        commitment: Some(solana_sdk::commitment_config::CommitmentConfig::confirmed()),
                        min_context_slot: None,
                    },
                    with_context: None,
                },
            ).map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?;
            
            // Convert (Pubkey, Account) to RpcKeyedAccount
            let rpc_accounts: Vec<solana_client::rpc_response::RpcKeyedAccount> = accounts
                .into_iter()
                .map(|(pubkey, account)| solana_client::rpc_response::RpcKeyedAccount {
                    pubkey: pubkey.to_string(),
                    account: UiAccount {
                        lamports: account.lamports,
                        data: solana_account_decoder::UiAccountData::Binary(base64::engine::general_purpose::STANDARD.encode(&account.data), solana_account_decoder::UiAccountEncoding::Base64),
                        owner: account.owner.to_string(),
                        executable: account.executable,
                        rent_epoch: account.rent_epoch,
                        space: Some(account.data.len() as u64),
                    },
                })
                .collect();
            
            all_token_accounts.extend(rpc_accounts);
        }
        
        Ok(all_token_accounts)
        }
    }

    fn get_system_accounts_with_config(
        &self,
        pubkey: &Pubkey,
    ) -> impl std::future::Future<Output = Result<Vec<solana_client::rpc_response::RpcKeyedAccount>>> + Send {
        async move {
        // Get system accounts owned by this wallet
        let accounts = self.client.get_program_accounts_with_config(
            &solana_program::system_program::id(),
            solana_client::rpc_config::RpcProgramAccountsConfig {
                filters: Some(vec![
                    solana_client::rpc_filter::RpcFilterType::Memcmp(
                        solana_client::rpc_filter::Memcmp::new_raw_bytes(
                            0, // owner offset in system account
                            pubkey.as_ref().to_vec(),
                        ),
                    ),
                ]),
                account_config: solana_client::rpc_config::RpcAccountInfoConfig {
                    encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                    data_slice: None,
                    commitment: Some(solana_sdk::commitment_config::CommitmentConfig::confirmed()),
                    min_context_slot: None,
                },
                with_context: None,
            },
        ).map_err(|e| SolanaRecoverError::RpcError(e.to_string()))?;
        
        // Convert (Pubkey, Account) to RpcKeyedAccount
        let rpc_accounts: Vec<solana_client::rpc_response::RpcKeyedAccount> = accounts
            .into_iter()
            .map(|(pubkey, account)| solana_client::rpc_response::RpcKeyedAccount {
                pubkey: pubkey.to_string(),
                account: UiAccount {
                    lamports: account.lamports,
                    data: solana_account_decoder::UiAccountData::Binary(base64::engine::general_purpose::STANDARD.encode(&account.data), solana_account_decoder::UiAccountEncoding::Base64),
                    owner: account.owner.to_string(),
                    executable: account.executable,
                    rent_epoch: account.rent_epoch,
                    space: Some(account.data.len() as u64),
                },
            })
            .collect();
        
        Ok(rpc_accounts)
        }
    }
}
