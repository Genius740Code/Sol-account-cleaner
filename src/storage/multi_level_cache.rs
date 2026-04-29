use crate::core::{Result, SolanaRecoverError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use moka::future::Cache as MokaCache;
use dashmap::DashMap;
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use std::collections::HashMap;
use flate2::{Compress, Decompress, Compression};

/// Multi-level cache architecture with L1 (hot), L2 (warm), and L3 (cold) tiers
pub struct MultiLevelCache {
    l1_cache: Arc<MokaCache<String, CachedAccount>>,
    l2_cache: Arc<DashMap<String, CachedAccount>>,
    l3_cache: Arc<PersistentCache>,
    config: CacheConfig,
    metrics: Arc<RwLock<CacheMetrics>>,
    eviction_policy: EvictionPolicy,
}

#[derive(Debug)]
pub struct CachedAccount {
    pub data: AccountData,
    pub timestamp: Instant,
    pub access_count: std::sync::atomic::AtomicU64,
    pub priority: CachePriority,
    pub size_bytes: usize,
    pub compressed: bool,
}

impl Clone for CachedAccount {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            timestamp: self.timestamp,
            access_count: std::sync::atomic::AtomicU64::new(
                self.access_count.load(std::sync::atomic::Ordering::Relaxed)
            ),
            priority: self.priority,
            size_bytes: self.size_bytes,
            compressed: self.compressed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableCachedAccount {
    data: AccountData,
    access_count: u64,
    priority: CachePriority,
    size_bytes: usize,
    compressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountData {
    AccountInfo(solana_sdk::account::Account),
    RentExemption(u64),
    TokenAccount(TokenAccountInfo),
    BatchAccounts(Vec<solana_client::rpc_response::RpcKeyedAccount>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAccountInfo {
    pub mint: String,
    pub amount: u64,
    pub owner: String,
    pub lamports: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum CachePriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    // L1 Cache (Hot) - In-memory with Moka
    pub l1_max_capacity: usize,
    pub l1_ttl: Duration,
    pub l1_tti: Duration, // Time to idle
    
    // L2 Cache (Warm) - DashMap for concurrent access
    pub l2_max_capacity: usize,
    pub l2_ttl: Duration,
    
    // L3 Cache (Cold) - Persistent storage
    pub l3_max_size_bytes: usize,
    pub l3_ttl: Duration,
    pub l3_compression_threshold: usize,
    
    // Global settings
    pub enable_compression: bool,
    pub enable_metrics: bool,
    pub cleanup_interval: Duration,
    pub warming_enabled: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct CacheMetrics {
    // L1 Metrics
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l1_evictions: u64,
    
    // L2 Metrics
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l2_evictions: u64,
    
    // L3 Metrics
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub l3_evictions: u64,
    
    // Global Metrics
    pub total_requests: u64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub total_evictions: u64,
    pub compression_ratio: f64,
    pub avg_access_time_ns: u64,
    pub last_cleanup: Option<chrono::DateTime<chrono::Utc>>,
    
    // Size metrics
    pub l1_size_bytes: usize,
    pub l2_size_bytes: usize,
    pub l3_size_bytes: usize,
    pub total_size_bytes: usize,
}

#[derive(Debug, Clone)]
pub enum EvictionPolicy {
    LRU,            // Least Recently Used
    LFU,            // Least Frequently Used
    PriorityBased,  // Priority-based eviction
    Adaptive,       // Adaptive based on access patterns
}

/// Persistent cache implementation for L3 tier
pub struct PersistentCache {
    storage: Arc<DashMap<String, CompressedCachedAccount>>,
    max_size_bytes: usize,
    current_size_bytes: std::sync::atomic::AtomicUsize,
    ttl: Duration,
    compression_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct CompressedCachedAccount {
    data: Vec<u8>, // Compressed serialized data
    timestamp: Instant,
    compressed_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            // L1: 100MB, 5 minutes TTL, 2 minutes TTI
            l1_max_capacity: 100 * 1024 * 1024,
            l1_ttl: Duration::from_secs(300),
            l1_tti: Duration::from_secs(120),
            
            // L2: 500MB, 1 hour TTL
            l2_max_capacity: 500 * 1024 * 1024,
            l2_ttl: Duration::from_secs(3600),
            
            // L3: 2GB, 24 hours TTL, compress items > 1KB
            l3_max_size_bytes: 2 * 1024 * 1024 * 1024,
            l3_ttl: Duration::from_secs(86400),
            l3_compression_threshold: 1024,
            
            enable_compression: true,
            enable_metrics: true,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            warming_enabled: true,
        }
    }
}

impl MultiLevelCache {
    pub fn new(config: CacheConfig) -> Result<Self> {
        // Initialize L1 cache with Moka
        let l1_cache = Arc::new(
            MokaCache::builder()
                .max_capacity(config.l1_max_capacity as u64)
                .time_to_live(config.l1_ttl)
                .time_to_idle(config.l1_tti)
                .build()
        );
        
        // Initialize L2 cache with DashMap
        let l2_cache = Arc::new(DashMap::new());
        
        // Initialize L3 persistent cache
        let l3_cache = Arc::new(PersistentCache::new(
            config.l3_max_size_bytes,
            config.l3_ttl,
            config.enable_compression,
            config.l3_compression_threshold,
        )?);
        
        let cache = Self {
            l1_cache,
            l2_cache,
            l3_cache,
            config: config.clone(),
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
            eviction_policy: EvictionPolicy::Adaptive,
        };
        
        // Start background cleanup task
        if config.cleanup_interval > Duration::ZERO {
            cache.start_cleanup_task();
        }
        
        Ok(cache)
    }

    /// Get data from cache, checking L1 -> L2 -> L3 in order
    pub async fn get(&self, key: &str) -> Result<Option<CachedAccount>> {
        let start_time = std::time::Instant::now();
        
        // Update total requests
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
        }
        
        // Try L1 cache first (hottest data)
        if let Some(account) = self.l1_cache.get(key).await {
            if self.config.enable_metrics {
                let mut metrics = self.metrics.write().await;
                metrics.l1_hits += 1;
                metrics.total_hits += 1;
                self.update_access_time_metrics(&mut metrics, start_time).await;
            }
            
            // Update access count
            account.access_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            // Promote to higher priority if accessed frequently
            self.maybe_promote_account(key, &account).await;
            
            return Ok(Some(account.clone()));
        }
        
        // Try L2 cache (warm data)
        if let Some(account) = self.l2_cache.get(key) {
            if self.config.enable_metrics {
                let mut metrics = self.metrics.write().await;
                metrics.l2_hits += 1;
                metrics.total_hits += 1;
                self.update_access_time_metrics(&mut metrics, start_time).await;
            }
            
            // Update access count
            account.access_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            // Promote to L1 cache
            let account_clone = account.clone();
            self.l1_cache.insert(key.to_string(), account_clone.clone()).await;
            
            return Ok(Some(account_clone));
        }
        
        // Try L3 cache (cold data)
        if let Some(compressed_account) = self.l3_cache.get(key).await? {
            if self.config.enable_metrics {
                let mut metrics = self.metrics.write().await;
                metrics.l3_hits += 1;
                metrics.total_hits += 1;
                self.update_access_time_metrics(&mut metrics, start_time).await;
            }
            
            // Decompress and deserialize
            let account = self.decompress_account(&compressed_account)?;
            
            // Promote to L2 and potentially L1
            self.l2_cache.insert(key.to_string(), account.clone());
            if account.priority >= CachePriority::High {
                self.l1_cache.insert(key.to_string(), account.clone()).await;
            }
            
            return Ok(Some(account));
        }
        
        // Cache miss
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.l1_misses += 1;
            metrics.l2_misses += 1;
            metrics.l3_misses += 1;
            metrics.total_misses += 1;
            self.update_access_time_metrics(&mut metrics, start_time).await;
        }
        
        Ok(None)
    }

    /// Put data into cache with intelligent tier placement
    pub async fn put(&self, key: String, account: CachedAccount) -> Result<()> {
        let _start_time = std::time::Instant::now();
        
        // Determine initial cache tier based on priority and access patterns
        match account.priority {
            CachePriority::Critical | CachePriority::High => {
                // High priority items go to L1 immediately
                self.l1_cache.insert(key.clone(), account.clone()).await;
                self.l2_cache.insert(key, account);
            }
            CachePriority::Medium => {
                // Medium priority items start in L2
                self.l2_cache.insert(key, account);
            }
            CachePriority::Low => {
                // Low priority items go to L3 (with compression if beneficial)
                if self.should_compress(&account) {
                    let compressed = self.compress_account(&account)?;
                    self.l3_cache.put(key, compressed).await?;
                } else {
                    self.l2_cache.insert(key, account);
                }
            }
        }
        
        // Update metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            self.update_size_metrics(&mut metrics).await;
        }
        
        Ok(())
    }

    /// Batch get operation for multiple keys
    pub async fn get_multiple(&self, keys: &[String]) -> Result<HashMap<String, CachedAccount>> {
        let mut results = HashMap::new();
        
        // Process keys in parallel for better performance
        let get_futures = keys.iter().map(|key| async {
            let key = key.clone();
            match self.get(&key).await {
                Ok(Some(account)) => Some((key, account)),
                _ => None,
            }
        });
        
        let futures_results = futures::future::join_all(get_futures).await;
        
        for result in futures_results {
            if let Some((key, account)) = result {
                results.insert(key, account);
            }
        }
        
        Ok(results)
    }

    /// Batch put operation for multiple accounts
    pub async fn put_multiple(&self, items: Vec<(String, CachedAccount)>) -> Result<()> {
        // Process in parallel with controlled concurrency
        let semaphore = Arc::new(tokio::sync::Semaphore::new(10));
        
        let put_futures = items.into_iter().map(|(key, account)| {
            let semaphore = semaphore.clone();
            async move {
                let _permit = semaphore.acquire().await
                    .map_err(|_| SolanaRecoverError::InternalError("Semaphore acquisition failed".to_string()))?;
                
                self.put(key, account).await
            }
        });
        
        let results = futures::future::join_all(put_futures).await;
        
        // Check for any errors
        for result in results {
            result?; // Propagate first error encountered
        }
        
        Ok(())
    }

    /// Pre-warm cache with frequently accessed data
    pub async fn warm_cache(&self, keys: Vec<String>, priority: CachePriority) -> Result<()> {
        if !self.config.warming_enabled {
            return Ok(());
        }
        
        tracing::info!("Warming cache with {} keys", keys.len());
        
        // For cache warming, we'd typically fetch data from the source
        // This is a placeholder implementation
        for key in keys {
            // In a real implementation, you'd fetch the actual data
            // For now, we'll just create placeholder entries
            let placeholder = CachedAccount {
                data: AccountData::RentExemption(0), // Placeholder
                timestamp: Instant::now(),
                access_count: std::sync::atomic::AtomicU64::new(0),
                priority,
                size_bytes: 64, // Placeholder size
                compressed: false,
            };
            
            self.put(key, placeholder).await?;
        }
        
        Ok(())
    }

    /// Invalidate cache entries
    pub async fn invalidate(&self, key: &str) -> Result<bool> {
        let mut invalidated = false;
        
        // Remove from all tiers
        if self.l1_cache.remove(key).await.is_some() {
            invalidated = true;
        }
        
        if self.l2_cache.remove(key).is_some() {
            invalidated = true;
        }
        
        if self.l3_cache.remove(key).await? {
            invalidated = true;
        }
        
        Ok(invalidated)
    }

    /// Clear all cache tiers
    pub async fn clear(&self) -> Result<()> {
        self.l1_cache.invalidate_all();
        self.l2_cache.clear();
        self.l3_cache.clear().await?;
        
        // Reset metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            *metrics = CacheMetrics::default();
        }
        
        Ok(())
    }

    /// Get comprehensive cache metrics
    pub async fn get_metrics(&self) -> CacheMetrics {
        if !self.config.enable_metrics {
            return CacheMetrics::default();
        }
        
        let mut metrics = self.metrics.write().await;
        self.update_size_metrics(&mut metrics).await;
        
        CacheMetrics {
            l1_hits: metrics.l1_hits,
            l1_misses: metrics.l1_misses,
            l1_evictions: metrics.l1_evictions,
            l2_hits: metrics.l2_hits,
            l2_misses: metrics.l2_misses,
            l2_evictions: metrics.l2_evictions,
            l3_hits: metrics.l3_hits,
            l3_misses: metrics.l3_misses,
            l3_evictions: metrics.l3_evictions,
            total_requests: metrics.total_requests,
            total_hits: metrics.total_hits,
            total_misses: metrics.total_misses,
            total_evictions: metrics.total_evictions,
            compression_ratio: metrics.compression_ratio,
            avg_access_time_ns: metrics.avg_access_time_ns,
            last_cleanup: metrics.last_cleanup,
            l1_size_bytes: metrics.l1_size_bytes,
            l2_size_bytes: metrics.l2_size_bytes,
            l3_size_bytes: metrics.l3_size_bytes,
            total_size_bytes: metrics.total_size_bytes,
        }
    }

    // Helper methods
    
    async fn maybe_promote_account(&self, key: &str, account: &CachedAccount) {
        let access_count = account.access_count.load(std::sync::atomic::Ordering::Relaxed);
        
        // Promote to higher priority if accessed frequently
        if access_count > 10 && account.priority == CachePriority::Medium {
            // This would require updating the account priority
            // For now, just ensure it's in L1
            self.l1_cache.insert(key.to_string(), account.clone()).await;
        }
    }

    fn should_compress(&self, account: &CachedAccount) -> bool {
        self.config.enable_compression && account.size_bytes > self.config.l3_compression_threshold
    }

    fn compress_account(&self, account: &CachedAccount) -> Result<CompressedCachedAccount> {
        // Create a serializable version without Instant
        let serializable = SerializableCachedAccount {
            data: account.data.clone(),
            access_count: account.access_count.load(std::sync::atomic::Ordering::Relaxed),
            priority: account.priority,
            size_bytes: account.size_bytes,
            compressed: account.compressed,
        };
        
        let serialized = bincode::serialize(&serializable)
            .map_err(|e| SolanaRecoverError::InternalError(format!("Serialization failed: {}", e)))?;
        
        let mut compressor = Compress::new(Compression::default(), false);
        let mut compressed = Vec::new();
        
        compressor.compress_vec(&serialized, &mut compressed, flate2::FlushCompress::Finish)
            .map_err(|e| SolanaRecoverError::InternalError(format!("Compression failed: {}", e)))?;
        
        Ok(CompressedCachedAccount {
            data: compressed.clone(),
            timestamp: account.timestamp,
            compressed_size: compressed.len(),
        })
    }

    fn decompress_account(&self, compressed: &CompressedCachedAccount) -> Result<CachedAccount> {
        let mut decompressor = Decompress::new(false);
        let mut decompressed = Vec::new();
        
        decompressor.decompress_vec(&compressed.data, &mut decompressed, flate2::FlushDecompress::Finish)
            .map_err(|e| SolanaRecoverError::InternalError(format!("Decompression failed: {}", e)))?;
        
        let serializable: SerializableCachedAccount = bincode::deserialize(&decompressed)
            .map_err(|e| SolanaRecoverError::InternalError(format!("Deserialization failed: {}", e)))?;
        
        let account = CachedAccount {
            data: serializable.data,
            timestamp: Instant::now(), // Use current time as approximation
            access_count: std::sync::atomic::AtomicU64::new(serializable.access_count),
            priority: serializable.priority,
            size_bytes: serializable.size_bytes,
            compressed: serializable.compressed,
        };
        
        Ok(account)
    }

    async fn update_access_time_metrics(&self, metrics: &mut CacheMetrics, start_time: Instant) {
        let access_time_ns = start_time.elapsed().as_nanos() as u64;
        let total_requests = metrics.total_requests;
        
        if total_requests > 0 {
            metrics.avg_access_time_ns = 
                (metrics.avg_access_time_ns * (total_requests - 1) + access_time_ns) / total_requests;
        }
    }

    async fn update_size_metrics(&self, metrics: &mut CacheMetrics) {
        // Update size metrics (simplified - in production would track actual memory usage)
        metrics.l1_size_bytes = (self.l1_cache.entry_count() as usize) * 1024; // Estimate
        metrics.l2_size_bytes = self.l2_cache.len() * 2048; // Estimate
        metrics.l3_size_bytes = self.l3_cache.current_size_bytes.load(std::sync::atomic::Ordering::Relaxed);
        metrics.total_size_bytes = metrics.l1_size_bytes + metrics.l2_size_bytes + metrics.l3_size_bytes;
    }

    fn start_cleanup_task(&self) {
        let cache = self.clone();
        let cleanup_interval = self.config.cleanup_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = cache.perform_cleanup().await {
                    tracing::error!("Cache cleanup failed: {}", e);
                }
                
                // Update last cleanup time
                if cache.config.enable_metrics {
                    let mut metrics = cache.metrics.write().await;
                    metrics.last_cleanup = Some(chrono::Utc::now());
                }
            }
        });
    }

    async fn perform_cleanup(&self) -> Result<()> {
        // Cleanup expired entries from L2 cache
        let now = Instant::now();
        let mut keys_to_remove = Vec::new();
        
        for entry in self.l2_cache.iter() {
            if now.duration_since(entry.timestamp) > self.config.l2_ttl {
                keys_to_remove.push(entry.key().clone());
            }
        }
        
        for key in keys_to_remove {
            self.l2_cache.remove(&key);
        }
        
        // Cleanup L3 cache
        self.l3_cache.cleanup_expired().await?;
        
        Ok(())
    }
}

impl Clone for MultiLevelCache {
    fn clone(&self) -> Self {
        Self {
            l1_cache: self.l1_cache.clone(),
            l2_cache: self.l2_cache.clone(),
            l3_cache: self.l3_cache.clone(),
            config: self.config.clone(),
            metrics: self.metrics.clone(),
            eviction_policy: self.eviction_policy.clone(),
        }
    }
}

impl PersistentCache {
    pub fn new(max_size_bytes: usize, ttl: Duration, compression_enabled: bool, _compression_threshold: usize) -> Result<Self> {
        Ok(Self {
            storage: Arc::new(DashMap::new()),
            max_size_bytes,
            current_size_bytes: std::sync::atomic::AtomicUsize::new(0),
            ttl,
            compression_enabled,
        })
    }

    pub async fn get(&self, key: &str) -> Result<Option<CompressedCachedAccount>> {
        if let Some(compressed) = self.storage.get(key) {
            // Check TTL
            if compressed.timestamp.elapsed() < self.ttl {
                return Ok(Some(compressed.clone()));
            } else {
                // Expired, remove it
                drop(compressed);
                self.storage.remove(key);
            }
        }
        
        Ok(None)
    }

    pub async fn put(&self, key: String, account: CompressedCachedAccount) -> Result<()> {
        // Store compressed_size before moving account
        let compressed_size = account.compressed_size;
        
        // Check if we need to evict to make space
        let current_size = self.current_size_bytes.load(std::sync::atomic::Ordering::Relaxed);
        let new_size = current_size.saturating_add(compressed_size);
        
        if new_size > self.max_size_bytes {
            self.evict_lru(compressed_size).await?;
        }
        
        // Insert the new item
        let old_size = if let Some(old) = self.storage.insert(key, account) {
            old.compressed_size
        } else {
            0
        };
        
        // Update current size
        let size_diff = compressed_size.saturating_sub(old_size);
        self.current_size_bytes.fetch_add(size_diff, std::sync::atomic::Ordering::Relaxed);
        
        Ok(())
    }

    pub async fn remove(&self, key: &str) -> Result<bool> {
        if let Some((_key, removed)) = self.storage.remove(key) {
            self.current_size_bytes.fetch_sub(removed.compressed_size, std::sync::atomic::Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn clear(&self) -> Result<()> {
        self.storage.clear();
        self.current_size_bytes.store(0, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn cleanup_expired(&self) -> Result<()> {
        let now = Instant::now();
        let mut keys_to_remove = Vec::new();
        
        for entry in self.storage.iter() {
            if now.duration_since(entry.timestamp) > self.ttl {
                keys_to_remove.push(entry.key().clone());
            }
        }
        
        for key in keys_to_remove {
            self.remove(&key).await?;
        }
        
        Ok(())
    }

    async fn evict_lru(&self, needed_space: usize) -> Result<()> {
        let mut freed_space = 0;
        let mut items_to_remove = Vec::new();
        
        // Sort by timestamp (oldest first) and remove until we have enough space
        let mut items: Vec<_> = self.storage.iter().collect();
        items.sort_by_key(|entry| entry.timestamp);
        
        for entry in items {
            if freed_space >= needed_space {
                break;
            }
            
            freed_space += entry.compressed_size;
            items_to_remove.push(entry.key().clone());
        }
        
        for key in items_to_remove {
            self.remove(&key).await?;
        }
        
        Ok(())
    }
}

impl Clone for PersistentCache {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            max_size_bytes: self.max_size_bytes,
            current_size_bytes: std::sync::atomic::AtomicUsize::new(
                self.current_size_bytes.load(std::sync::atomic::Ordering::Relaxed)
            ),
            ttl: self.ttl,
            compression_enabled: self.compression_enabled,
        }
    }
}
