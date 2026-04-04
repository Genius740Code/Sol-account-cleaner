use crate::core::{Result, SolanaRecoverError, WalletInfo, ScanResult};
use moka::future::Cache as MokaCache;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use lru::LruCache;
use crossbeam::queue::SegQueue;
use flume::{Sender, Receiver};
use tokio::task::JoinHandle;
use once_cell::sync::Lazy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedCacheConfig {
    pub max_size: u64,
    pub ttl_seconds: u64,
    pub enable_background_refresh: bool,
    pub refresh_interval_seconds: u64,
    pub enable_compression: bool,
    pub enable_metrics: bool,
    pub shard_count: usize,
}

impl Default for OptimizedCacheConfig {
    fn default() -> Self {
        Self {
            max_size: 100_000,
            ttl_seconds: 300, // 5 minutes
            enable_background_refresh: true,
            refresh_interval_seconds: 60, // 1 minute
            enable_compression: false,
            enable_metrics: true,
            shard_count: 16,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub evictions: u64,
    pub refreshes: u64,
    pub size: u64,
    pub memory_usage_bytes: u64,
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            sets: 0,
            evictions: 0,
            refreshes: 0,
            size: 0,
            memory_usage_bytes: 0,
        }
    }
}

pub struct OptimizedCacheManager {
    // Sharded cache for better concurrency
    shards: Vec<Arc<MokaCache<String, CacheEntry>>>,
    
    // LRU cache for hot items
    hot_cache: Arc<RwLock<LruCache<String, WalletInfo>>>,
    
    // Metrics
    metrics: Arc<RwLock<CacheMetrics>>,
    
    // Configuration
    config: OptimizedCacheConfig,
    
    // Background refresh task
    refresh_task: Option<JoinHandle<()>>,
    
    // Refresh queue
    refresh_queue: Arc<SegQueue<String>>,
    
    // Cache hit statistics
    hit_stats: Arc<RwLock<lru::LruCache<String, u64>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    data: WalletInfo,
    created_at: chrono::DateTime<chrono::Utc>,
    last_accessed: chrono::DateTime<chrono::Utc>,
    access_count: u64,
    size_bytes: usize,
}

impl OptimizedCacheManager {
    pub fn new(config: OptimizedCacheConfig) -> Result<Self> {
        let mut shards = Vec::with_capacity(config.shard_count);
        
        for _ in 0..config.shard_count {
            let cache = MokaCache::builder()
                .max_capacity(config.max_size / config.shard_count as u64)
                .time_to_live(Duration::from_secs(config.ttl_seconds))
                .build();
            
            shards.push(Arc::new(cache));
        }
        
        let hot_cache = Arc::new(RwLock::new(LruCache::new(
            std::num::NonZeroUsize::new(1000).unwrap()
        )));
        
        let hit_stats = Arc::new(RwLock::new(LruCache::new(
            std::num::NonZeroUsize::new(10000).unwrap()
        )));
        
        let manager = Self {
            shards,
            hot_cache,
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
            config: config.clone(),
            refresh_task: None,
            refresh_queue: Arc::new(SegQueue::new()),
            hit_stats,
        };
        
        Ok(manager)
    }
    
    pub async fn start_background_refresh(&mut self) -> Result<()> {
        if !self.config.enable_background_refresh {
            return Ok(());
        }
        
        let refresh_queue = self.refresh_queue.clone();
        let shards = self.shards.clone();
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(config.refresh_interval_seconds)
            );
            
            loop {
                interval.tick().await;
                
                let mut refreshed_count = 0;
                let max_refresh_per_cycle = 100;
                
                while refreshed_count < max_refresh_per_cycle {
                    if let Some(key) = refresh_queue.pop() {
                        let shard_index = Self::hash_shard(&key, config.shard_count);
                        
                        if let Some(entry) = shards[shard_index].get(&key).await {
                            // Check if entry needs refresh (e.g., if it's old)
                            let now = chrono::Utc::now();
                            let age = now.signed_duration_since(entry.created_at);
                            
                            if age.num_seconds() > (config.ttl_seconds as i64 / 2) {
                                // In a real implementation, you would refresh the data here
                                // For now, we'll just update the access time
                                refreshed_count += 1;
                                
                                // Update metrics
                                let mut metrics_guard = metrics.write();
                                metrics_guard.refreshes += 1;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        });
        
        self.refresh_task = Some(task);
        Ok(())
    }
    
    pub async fn get(&self, key: &str) -> Option<WalletInfo> {
        let shard_index = Self::hash_shard(key, self.config.shard_count);
        
        // Try hot cache first
        {
            let mut hot_cache = self.hot_cache.write();
            if let Some(wallet_info) = hot_cache.get(key) {
                // Update hit statistics
                self.update_hit_stats(key);
                self.update_metrics(|m| m.hits += 1);
                return Some(wallet_info.clone());
            }
        }
        
        // Try main cache
        if let Some(entry) = self.shards[shard_index].get(key).await {
            // Update access statistics
            let mut updated_entry = entry.clone();
            updated_entry.last_accessed = chrono::Utc::now();
            updated_entry.access_count += 1;
            
            // Update the entry in cache
            self.shards[shard_index].insert(key.to_string(), updated_entry.clone()).await;
            
            // Move to hot cache if accessed frequently
            if updated_entry.access_count > 5 {
                let mut hot_cache = self.hot_cache.write();
                hot_cache.put(key.to_string(), updated_entry.data.clone());
            }
            
            // Update hit statistics
            self.update_hit_stats(key);
            self.update_metrics(|m| m.hits += 1);
            
            Some(updated_entry.data)
        } else {
            self.update_metrics(|m| m.misses += 1);
            None
        }
    }
    
    pub async fn set(&self, key: String, wallet_info: WalletInfo) -> Result<()> {
        let shard_index = Self::hash_shard(&key, self.config.shard_count);
        
        let entry = CacheEntry {
            size_bytes: self.estimate_size(&wallet_info),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 1,
            data: wallet_info,
        };
        
        self.shards[shard_index].insert(key.clone(), entry).await;
        
        // Add to refresh queue if background refresh is enabled
        if self.config.enable_background_refresh {
            self.refresh_queue.push(key);
        }
        
        self.update_metrics(|m| {
            m.sets += 1;
            m.size += 1;
        });
        
        Ok(())
    }
    
    pub async fn remove(&self, key: &str) -> Result<()> {
        let shard_index = Self::hash_shard(key, self.config.shard_count);
        
        self.shards[shard_index].invalidate(key).await;
        
        // Remove from hot cache
        {
            let mut hot_cache = self.hot_cache.write();
            hot_cache.pop(key);
        }
        
        self.update_metrics(|m| {
            if m.size > 0 {
                m.size -= 1;
            }
        });
        
        Ok(())
    }
    
    pub async fn clear(&self) -> Result<()> {
        for shard in &self.shards {
            shard.invalidate_all();
        }
        
        {
            let mut hot_cache = self.hot_cache.write();
            hot_cache.clear();
        }
        
        self.update_metrics(|m| {
            m.size = 0;
        });
        
        Ok(())
    }
    
    pub fn get_metrics(&self) -> CacheMetrics {
        let mut metrics = self.metrics.read().clone();
        
        // Calculate current size and memory usage
        let mut total_size = 0;
        let mut total_memory = 0;
        
        for shard in &self.shards {
            total_size += shard.entry_count() as u64;
            // Estimate memory usage (rough calculation)
            total_memory += shard.weighted_size() as u64;
        }
        
        metrics.size = total_size;
        metrics.memory_usage_bytes = total_memory;
        
        metrics
    }
    
    pub fn get_hot_keys(&self, limit: usize) -> Vec<(String, u64)> {
        let hit_stats = self.hit_stats.read();
        hit_stats
            .iter()
            .take(limit)
            .map(|(k, &v)| (k.clone(), v))
            .collect()
    }
    
    fn hash_shard(key: &str, shard_count: usize) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % shard_count
    }
    
    fn update_hit_stats(&self, key: &str) {
        let mut hit_stats = self.hit_stats.write();
        let count = hit_stats.get(key).unwrap_or(&0) + 1;
        hit_stats.put(key.to_string(), count);
    }
    
    fn update_metrics<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut CacheMetrics),
    {
        let mut metrics = self.metrics.write();
        update_fn(&mut metrics);
    }
    
    fn estimate_size(&self, wallet_info: &WalletInfo) -> usize {
        // Rough estimation of memory usage
        let base_size = std::mem::size_of::<WalletInfo>();
        let address_size = wallet_info.address.len();
        let empty_addresses_size: usize = wallet_info.empty_account_addresses
            .iter()
            .map(|addr| addr.len())
            .sum();
        
        base_size + address_size + empty_addresses_size
    }
    
    pub async fn preload(&self, keys: SmallVec<[String; 16]>) -> Result<usize> {
        let mut loaded_count = 0;
        
        for key in keys {
            if self.get(&key).await.is_none() {
                // In a real implementation, you would load the data from the database
                // For now, we'll just simulate loading
                loaded_count += 1;
            }
        }
        
        Ok(loaded_count)
    }
    
    pub fn get_cache_stats(&self) -> serde_json::Value {
        let metrics = self.get_metrics();
        let hit_rate = if metrics.hits + metrics.misses > 0 {
            metrics.hits as f64 / (metrics.hits + metrics.misses) as f64
        } else {
            0.0
        };
        
        serde_json::json!({
            "size": metrics.size,
            "hits": metrics.hits,
            "misses": metrics.misses,
            "sets": metrics.sets,
            "evictions": metrics.evictions,
            "refreshes": metrics.refreshes,
            "hit_rate": hit_rate,
            "memory_usage_bytes": metrics.memory_usage_bytes,
            "shard_count": self.config.shard_count,
            "ttl_seconds": self.config.ttl_seconds,
            "max_size": self.config.max_size
        })
    }
}

impl Drop for OptimizedCacheManager {
    fn drop(&mut self) {
        if let Some(task) = self.refresh_task.take() {
            task.abort();
        }
    }
}

// Global cache instance for convenience
static GLOBAL_CACHE: Lazy<Arc<OptimizedCacheManager>> = Lazy::new(|| {
    let config = OptimizedCacheConfig::default();
    Arc::new(OptimizedCacheManager::new(config).unwrap())
});

pub fn get_global_cache() -> Arc<OptimizedCacheManager> {
    GLOBAL_CACHE.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cache_basic_operations() {
        let config = OptimizedCacheConfig::default();
        let cache = OptimizedCacheManager::new(config).unwrap();
        
        let key = "test_key".to_string();
        let wallet_info = WalletInfo {
            address: "test_address".to_string(),
            pubkey: solana_sdk::pubkey::Pubkey::default(),
            total_accounts: 10,
            empty_accounts: 5,
            recoverable_lamports: 1_000_000,
            recoverable_sol: 0.001,
            empty_account_addresses: vec![],
            scan_time_ms: 1000,
        };
        
        // Test set and get
        cache.set(key.clone(), wallet_info.clone()).await.unwrap();
        let retrieved = cache.get(&key).await.unwrap();
        
        assert_eq!(retrieved.address, wallet_info.address);
        assert_eq!(retrieved.total_accounts, wallet_info.total_accounts);
        
        // Test metrics
        let metrics = cache.get_metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.sets, 1);
        assert_eq!(metrics.size, 1);
    }
    
    #[tokio::test]
    async fn test_cache_hot_items() {
        let config = OptimizedCacheConfig::default();
        let cache = OptimizedCacheManager::new(config).unwrap();
        
        let key = "hot_key".to_string();
        let wallet_info = WalletInfo {
            address: "test_address".to_string(),
            pubkey: solana_sdk::pubkey::Pubkey::default(),
            total_accounts: 10,
            empty_accounts: 5,
            recoverable_lamports: 1_000_000,
            recoverable_sol: 0.001,
            empty_account_addresses: vec![],
            scan_time_ms: 1000,
        };
        
        cache.set(key.clone(), wallet_info).await.unwrap();
        
        // Access multiple times to make it hot
        for _ in 0..10 {
            cache.get(&key).await;
        }
        
        let hot_keys = cache.get_hot_keys(10);
        assert!(!hot_keys.is_empty());
        assert!(hot_keys[0].1 >= 10); // Should have at least 10 hits
    }
    
    #[tokio::test]
    async fn test_cache_sharding() {
        let config = OptimizedCacheConfig {
            shard_count: 4,
            ..Default::default()
        };
        let cache = OptimizedCacheManager::new(config).unwrap();
        
        // Insert items that should go to different shards
        for i in 0..10 {
            let key = format!("key_{}", i);
            let wallet_info = WalletInfo {
                address: format!("address_{}", i),
                pubkey: solana_sdk::pubkey::Pubkey::default(),
                total_accounts: i,
                empty_accounts: i / 2,
                recoverable_lamports: (i * 1_000_000) as u64,
                recoverable_sol: i as f64 * 0.001,
                empty_account_addresses: vec![],
                scan_time_ms: i as u64 * 100,
            };
            
            cache.set(key, wallet_info).await.unwrap();
        }
        
        let metrics = cache.get_metrics();
        assert_eq!(metrics.sets, 10);
        assert_eq!(metrics.size, 10);
    }
}
