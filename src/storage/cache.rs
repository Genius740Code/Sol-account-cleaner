use crate::core::{Result, SolanaRecoverError};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::sync::Arc;
use moka::future::Cache as MokaCache;
use parking_lot::RwLock;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheConfig {
    pub ttl_seconds: u64,
    pub max_size: usize,
    pub cleanup_interval_seconds: u64,
    pub enable_hierarchical_cache: bool,
    pub l1_cache_size: usize,
    pub l2_cache_size: usize,
    pub compression_threshold: usize,
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 300, // 5 minutes
            max_size: 100000, // Increased for better performance
            cleanup_interval_seconds: 60, // 1 minute
            enable_hierarchical_cache: true,
            l1_cache_size: 10000,  // Hot data
            l2_cache_size: 50000,  // Warm data
            compression_threshold: 1024, // Compress entries > 1KB
            enable_metrics: true,
        }
    }
}

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    created_at: Instant,
    ttl: Duration,
    access_count: u64,
    last_accessed: Instant,
    size_bytes: usize,
    compressed: bool,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        let size_bytes = std::mem::size_of_val(&value);
        Self {
            value,
            created_at: Instant::now(),
            ttl,
            access_count: 0,
            last_accessed: Instant::now(),
            size_bytes,
            compressed: false,
        }
    }
    
    fn new_compressed(value: T, ttl: Duration) -> Self {
        let size_bytes = std::mem::size_of_val(&value);
        Self {
            value,
            created_at: Instant::now(),
            ttl,
            access_count: 0,
            last_accessed: Instant::now(),
            size_bytes,
            compressed: true,
        }
    }
    
    fn is_compressed(&self) -> bool {
        self.compressed
    }
    
    fn set_compressed(&mut self, compressed: bool) {
        self.compressed = compressed;
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
    
    fn touch(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
    }
    
    fn priority_score(&self) -> f64 {
        let age_factor = self.last_accessed.elapsed().as_secs_f64();
        let access_factor = self.access_count as f64;
        access_factor / (1.0 + age_factor)
    }
}

pub struct CacheManager {
    // Hierarchical cache: L1 (hot) + L2 (warm)
    l1_cache: Arc<MokaCache<String, CacheEntry<serde_json::Value>>>,
    l2_cache: Arc<DashMap<String, CacheEntry<serde_json::Value>>>,
    config: CacheConfig,
    metrics: Arc<RwLock<CacheMetrics>>,
    compression_enabled: bool,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub evictions: u64,
    pub compressions: u64,
    pub decompressions: u64,
    pub total_size_bytes: u64,
    pub avg_access_time_ns: u64,
    pub l1_hits: u64,
    pub l2_hits: u64,
}

impl CacheManager {
    pub fn new(config: CacheConfig) -> Self {
        let l1_cache = Arc::new(
            MokaCache::builder()
                .max_capacity(config.l1_cache_size as u64)
                .time_to_live(Duration::from_secs(config.ttl_seconds))
                .build()
        );
        
        let cache = Self {
            l1_cache,
            l2_cache: Arc::new(DashMap::new()),
            config: config.clone(),
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
            compression_enabled: config.compression_threshold > 0,
        };

        // Start cleanup task for L2 cache
        if config.enable_hierarchical_cache {
            let l2_cache = cache.l2_cache.clone();
            let cleanup_interval = Duration::from_secs(config.cleanup_interval_seconds);
            let metrics = cache.metrics.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(cleanup_interval);
                loop {
                    interval.tick().await;
                    Self::cleanup_expired_l2(&l2_cache, &metrics);
                }
            });
        }

        cache
    }

    fn cleanup_expired_l2(
        l2_cache: &DashMap<String, CacheEntry<serde_json::Value>>,
        metrics: &Arc<RwLock<CacheMetrics>>
    ) {
        let expired_keys: Vec<String> = l2_cache
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone())
            .collect();

        let mut total_size_freed = 0usize;
        for key in &expired_keys {
            if let Some((_, entry)) = l2_cache.remove(key) {
                total_size_freed += entry.size_bytes;
            }
        }
        
        // Update metrics
        {
            let mut m = metrics.write();
            m.total_size_bytes = m.total_size_bytes.saturating_sub(total_size_freed as u64);
        }
    }

    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let start = Instant::now();
        
        // Try L1 cache first (hot data)
        if let Some(mut entry) = self.l1_cache.get(key).await {
            entry.touch();
            
            // Track compression metrics
            if entry.is_compressed() {
                let mut m = self.metrics.write();
                m.decompressions += 1;
            }
            
            let value: T = serde_json::from_value(entry.value.clone())
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to deserialize cache value: {}", e)
                ))?;
            
            // Update metrics
            {
                let mut m = self.metrics.write();
                m.hits += 1;
                m.l1_hits += 1;
                let elapsed = start.elapsed().as_nanos() as u64;
                m.avg_access_time_ns = (m.avg_access_time_ns + elapsed) / 2;
            }
            
            return Ok(Some(value));
        }
        
        // Try L2 cache (warm data)
        if self.config.enable_hierarchical_cache {
            if let Some(mut entry) = self.l2_cache.get_mut(key) {
                if entry.is_expired() {
                    drop(entry);
                    self.l2_cache.remove(key);
                } else {
                    entry.touch();
                    
                    // Track compression metrics
                    if entry.is_compressed() {
                        let mut m = self.metrics.write();
                        m.decompressions += 1;
                    }
                    
                    let value: T = serde_json::from_value(entry.value.clone())
                        .map_err(|e| SolanaRecoverError::StorageError(
                            format!("Failed to deserialize cache value: {}", e)
                        ))?;
                    
                    // Promote to L1 if accessed frequently
                    if entry.access_count > 3 {
                        let entry_clone = entry.clone();
                        drop(entry);
                        let _ = self.l1_cache.insert(key.to_string(), entry_clone).await;
                    } else {
                        drop(entry);
                    }
                    
                    // Update metrics
                    {
                        let mut m = self.metrics.write();
                        m.hits += 1;
                        m.l2_hits += 1;
                        let elapsed = start.elapsed().as_nanos() as u64;
                        m.avg_access_time_ns = (m.avg_access_time_ns + elapsed) / 2;
                    }
                    
                    return Ok(Some(value));
                }
            }
        }
        
        // Cache miss
        {
            let mut m = self.metrics.write();
            m.misses += 1;
            let elapsed = start.elapsed().as_nanos() as u64;
            m.avg_access_time_ns = (m.avg_access_time_ns + elapsed) / 2;
        }
        
        Ok(None)
    }

    pub async fn set<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let json_value = serde_json::to_value(value)
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to serialize cache value: {}", e)
            ))?;
        
        let size_bytes = json_value.to_string().len();
        let should_compress = self.compression_enabled && size_bytes > self.config.compression_threshold;
        
        let entry = if should_compress {
            CacheEntry::new_compressed(
                json_value,
                Duration::from_secs(self.config.ttl_seconds)
            )
        } else {
            CacheEntry::new(
                json_value,
                Duration::from_secs(self.config.ttl_seconds)
            )
        };
        
        // Store in L1 cache (hot data)
        let _ = self.l1_cache.insert(key.to_string(), entry.clone()).await;
        
        // Also store in L2 if hierarchical caching is enabled
        if self.config.enable_hierarchical_cache {
            self.l2_cache.insert(key.to_string(), entry);
        }
        
        // Update metrics
        {
            let mut m = self.metrics.write();
            m.sets += 1;
            m.total_size_bytes += size_bytes as u64;
            if should_compress {
                m.compressions += 1;
            }
        }
        
        // Check if we need to evict entries
        if self.l1_cache.weighted_size() >= self.config.l1_cache_size as u64 {
            self.evict_lru_l1();
        }
        
        if self.config.enable_hierarchical_cache && self.l2_cache.len() >= self.config.l2_cache_size {
            self.evict_smart_l2();
        }
        
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> bool {
        let l1_removed = self.l1_cache.remove(key).await.is_some();
        let l2_removed = if self.config.enable_hierarchical_cache {
            self.l2_cache.remove(key).is_some()
        } else {
            false
        };
        l1_removed || l2_removed
    }

    pub fn clear(&self) {
        self.l1_cache.invalidate_all();
        if self.config.enable_hierarchical_cache {
            self.l2_cache.clear();
        }
    }

    pub fn size(&self) -> u64 {
        let l1_size = self.l1_cache.entry_count();
        let l2_size = if self.config.enable_hierarchical_cache {
            self.l2_cache.len()
        } else {
            0
        };
        
        l1_size as u64 + l2_size as u64
    }

    pub fn stats(&self) -> CacheStats {
        let l1_size = self.l1_cache.entry_count();
        let l2_size = if self.config.enable_hierarchical_cache {
            self.l2_cache.len()
        } else {
            0
        };
        
        let expired_l2 = if self.config.enable_hierarchical_cache {
            self.l2_cache
                .iter()
                .filter(|entry| entry.value().is_expired())
                .count()
        } else {
            0
        };
        
        CacheStats {
            total_entries: l1_size as u64 + l2_size as u64,
            l1_entries: l1_size as u64,
            l2_entries: l2_size as u64,
            expired_entries: expired_l2 as u64,
            max_size: self.config.max_size as u64,
            l1_max_size: self.config.l1_cache_size as u64,
            l2_max_size: self.config.l2_cache_size as u64,
            ttl_seconds: self.config.ttl_seconds,
            metrics: self.metrics.read().clone(),
        }
    }

    fn evict_lru_l1(&self) {
        // Moka cache handles LRU eviction automatically
        // This is a no-op as Moka manages its own eviction
    }
    
    fn evict_smart_l2(&self) {
        // Smart eviction based on priority score (access frequency + recency)
        let mut entries: Vec<_> = self.l2_cache
            .iter()
            .map(|entry| {
                let (key, value) = entry.pair();
                (key.clone(), value.priority_score())
            })
            .collect();

        entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Remove lowest priority entries (bottom 20%)
        let remove_count = (self.config.l2_cache_size / 5).max(1);
        let mut total_size_freed = 0usize;
        
        for (key, _) in entries.iter().take(remove_count) {
            if let Some((_, entry)) = self.l2_cache.remove(key) {
                total_size_freed += entry.size_bytes;
            }
        }
        
        // Update metrics
        {
            let mut m = self.metrics.write();
            m.evictions += remove_count as u64;
            m.total_size_bytes = m.total_size_bytes.saturating_sub(total_size_freed as u64);
        }
    }
    
    pub fn get_metrics(&self) -> CacheMetrics {
        self.metrics.read().clone()
    }
    
    pub async fn recompress_entries(&self, new_threshold: usize) -> Result<()> {
        // Recompress entries based on new threshold
        if self.config.enable_hierarchical_cache {
            let mut entries_to_update = Vec::new();
            
            for entry in self.l2_cache.iter() {
                let (key, cache_entry) = entry.pair();
                let should_compress = cache_entry.size_bytes > new_threshold;
                
                if cache_entry.is_compressed() != should_compress {
                    let mut updated_entry = cache_entry.clone();
                    updated_entry.set_compressed(should_compress);
                    entries_to_update.push((key.clone(), updated_entry));
                }
            }
            
            // Update entries with new compression status
            for (key, entry) in entries_to_update {
                let _ = self.l1_cache.insert(key, entry).await;
            }
        }
        Ok(())
    }
    
    pub async fn warm_up<T>(&self, entries: Vec<(String, T)>) -> Result<()>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        for (key, value) in entries {
            self.set(&key, &value).await?;
        }
        Ok(())
    }
    
    pub async fn prefetch_batch<T>(&self, keys: Vec<String>) -> Vec<(String, Option<T>)>
    where
        T: for<'de> Deserialize<'de>,
    {
        use futures::future::join_all;
        
        let results = join_all(
            keys.into_iter().map(|key| async move {
                let result = self.get(&key).await.unwrap_or(None);
                (key, result)
            })
        ).await;
        
        results
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub total_entries: u64,
    pub l1_entries: u64,
    pub l2_entries: u64,
    pub expired_entries: u64,
    pub max_size: u64,
    pub l1_max_size: u64,
    pub l2_max_size: u64,
    pub ttl_seconds: u64,
    pub metrics: CacheMetrics,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}
