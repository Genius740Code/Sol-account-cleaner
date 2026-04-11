use crate::core::{Result, SolanaRecoverError};
use crate::storage::{CacheManager, RedisCacheManager, CacheConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use flate2::{write::GzEncoder, read::GzDecoder};
use std::io::{Read, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedWalletInfo {
    pub wallet_address: String,
    pub empty_accounts: Vec<EmptyAccount>,
    pub total_accounts: u64,
    pub recoverable_sol: f64,
    pub cached_at: chrono::DateTime<chrono::Utc>,
    pub ttl: chrono::Duration,
    pub compression_type: CompressionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyAccount {
    pub address: String,
    pub balance: f64,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct HierarchicalCache {
    l1_cache: Arc<CacheManager>,      // Hot data (1 minute TTL)
    l2_cache: Arc<CacheManager>,      // Warm data (15 minute TTL)
    l3_cache: Option<Arc<RedisCacheManager>>, // Cold data (1 hour TTL)
    compression: Arc<CompressionEngine>,
    cache_warmer: Arc<CacheWarmer>,
    config: HierarchicalCacheConfig,
    metrics: Arc<RwLock<CacheMetrics>>,
}

#[derive(Debug, Clone)]
pub struct HierarchicalCacheConfig {
    pub l1_ttl_seconds: u64,
    pub l1_max_size: usize,
    pub l2_ttl_seconds: u64,
    pub l2_max_size: usize,
    pub l3_ttl_seconds: u64,
    pub enable_compression: bool,
    pub compression_threshold: usize,
    pub enable_cache_warming: bool,
    pub enable_metrics: bool,
    pub redis_url: Option<String>,
}

impl Default for HierarchicalCacheConfig {
    fn default() -> Self {
        Self {
            l1_ttl_seconds: 60,
            l1_max_size: 100000,
            l2_ttl_seconds: 900,
            l2_max_size: 1000000,
            l3_ttl_seconds: 3600,
            enable_compression: true,
            compression_threshold: 1024,
            enable_cache_warming: true,
            enable_metrics: true,
            redis_url: std::env::var("REDIS_URL").ok(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheMetrics {
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub l3_hits: u64,
    pub l1_misses: u64,
    pub l2_misses: u64,
    pub l3_misses: u64,
    pub compressions: u64,
    pub decompressions: u64,
    pub promotions: u64,
    pub total_requests: u64,
    pub cache_warmups: u64,
}

pub struct CompressionEngine {
    enable_compression: bool,
    threshold: usize,
}

impl CompressionEngine {
    pub fn new(enable_compression: bool, threshold: usize) -> Self {
        Self {
            enable_compression,
            threshold,
        }
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.enable_compression || data.len() < self.threshold {
            return Ok(data.to_vec());
        }

        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data)
            .map_err(|e| SolanaRecoverError::SerializationError(
                format!("Compression failed: {}", e)
            ))?;
        encoder.finish()
            .map_err(|e| SolanaRecoverError::SerializationError(
                format!("Compression finish failed: {}", e)
            ))
    }

    pub fn decompress(&self, compressed_data: &[u8]) -> Result<Vec<u8>> {
        // Check if data is compressed by looking for gzip magic number
        if compressed_data.len() < 2 || (compressed_data[0] != 0x1f || compressed_data[1] != 0x8b) {
            return Ok(compressed_data.to_vec());
        }

        let mut decoder = GzDecoder::new(compressed_data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| SolanaRecoverError::SerializationError(
                format!("Decompression failed: {}", e)
            ))?;
        Ok(decompressed)
    }
}

pub struct CacheWarmer {
    l1_cache: Arc<CacheManager>,
    l2_cache: Arc<CacheManager>,
    l3_cache: Option<Arc<RedisCacheManager>>,
    enable_warming: bool,
}

impl CacheWarmer {
    pub fn new(
        l1_cache: Arc<CacheManager>,
        l2_cache: Arc<CacheManager>,
        l3_cache: Option<Arc<RedisCacheManager>>,
        enable_warming: bool,
    ) -> Self {
        Self {
            l1_cache,
            l2_cache,
            l3_cache,
            enable_warming,
        }
    }

    pub async fn warm_wallet_cache(&self, wallet_addresses: Vec<String>) -> Result<usize> {
        if !self.enable_warming {
            return Ok(0);
        }

        let mut warmed_count = 0;
        
        for address in wallet_addresses {
            // Check if already in L1 or L2
            if let Ok(Some(_)) = self.l1_cache.get::<CachedWalletInfo>(&address).await {
                continue;
            }
            
            if let Ok(Some(_)) = self.l2_cache.get::<CachedWalletInfo>(&address).await {
                continue;
            }

            // Try to get from L3 and promote to L2
            if let Some(l3_cache) = &self.l3_cache {
                if let Ok(Some(wallet_info)) = l3_cache.get::<CachedWalletInfo>(&address).await {
                    // Promote to L2 cache
                    if let Ok(()) = self.l2_cache.set(&address, &wallet_info).await {
                        warmed_count += 1;
                        debug!("Warmed cache for wallet: {}", address);
                    }
                }
            }
        }

        info!("Cache warming completed: {} wallets warmed", warmed_count);
        Ok(warmed_count)
    }

    pub async fn periodic_warmup(&self, frequent_wallets: Vec<String>) -> Result<()> {
        info!("Starting periodic cache warmup for {} wallets", frequent_wallets.len());
        self.warm_wallet_cache(frequent_wallets).await?;
        Ok(())
    }
}

impl HierarchicalCache {
    pub async fn new(config: HierarchicalCacheConfig) -> Result<Self> {
        // Create L1 cache configuration
        let l1_config = CacheConfig {
            ttl_seconds: config.l1_ttl_seconds,
            max_size: config.l1_max_size,
            cleanup_interval_seconds: 60,
            enable_hierarchical_cache: false, // L1 is standalone
            l1_cache_size: config.l1_max_size,
            l2_cache_size: 0,
            compression_threshold: config.compression_threshold,
            enable_metrics: config.enable_metrics,
        };

        // Create L2 cache configuration
        let l2_config = CacheConfig {
            ttl_seconds: config.l2_ttl_seconds,
            max_size: config.l2_max_size,
            cleanup_interval_seconds: 300,
            enable_hierarchical_cache: false, // L2 is standalone
            l1_cache_size: 0,
            l2_cache_size: config.l2_max_size,
            compression_threshold: config.compression_threshold,
            enable_metrics: config.enable_metrics,
        };

        let l1_cache = Arc::new(CacheManager::new(l1_config));
        let l2_cache = Arc::new(CacheManager::new(l2_config));

        // Create L3 cache (Redis) if configured
        let l3_cache = if let Some(ref redis_url) = config.redis_url {
            std::env::set_var("REDIS_URL", redis_url.clone());
            let redis_config = CacheConfig {
                ttl_seconds: config.l3_ttl_seconds,
                max_size: 10000000, // Large size for Redis
                cleanup_interval_seconds: 600,
                enable_hierarchical_cache: false,
                l1_cache_size: 0,
                l2_cache_size: 0,
                compression_threshold: config.compression_threshold,
                enable_metrics: config.enable_metrics,
            };

            match RedisCacheManager::new(redis_config).await {
                Ok(cache) => Some(Arc::new(cache)),
                Err(e) => {
                    warn!("Failed to initialize Redis cache: {}. Continuing without L3 cache.", e);
                    None
                }
            }
        } else {
            None
        };

        let compression = Arc::new(CompressionEngine::new(
            config.enable_compression,
            config.compression_threshold,
        ));

        let cache_warmer = Arc::new(CacheWarmer::new(
            l1_cache.clone(),
            l2_cache.clone(),
            l3_cache.clone(),
            config.enable_cache_warming,
        ));

        Ok(Self {
            l1_cache,
            l2_cache,
            l3_cache,
            compression,
            cache_warmer,
            config,
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
        })
    }

    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de> + Clone + Send + Sync + 'static + serde::Serialize,
    {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        drop(metrics);

        // Try L1 cache first (hot data)
        if let Ok(Some(value)) = self.l1_cache.get::<T>(key).await {
            let mut metrics = self.metrics.write().await;
            metrics.l1_hits += 1;
            drop(metrics);
            debug!("L1 cache hit for key: {}", key);
            return Ok(Some(value));
        }

        let mut metrics = self.metrics.write().await;
        metrics.l1_misses += 1;
        drop(metrics);

        // Try L2 cache (warm data)
        if let Ok(Some(value)) = self.l2_cache.get::<T>(key).await {
            let mut metrics = self.metrics.write().await;
            metrics.l2_hits += 1;
            metrics.promotions += 1;
            drop(metrics);

            // Promote to L1 cache
            if let Err(e) = self.l1_cache.set(key, &value).await {
                warn!("Failed to promote key {} to L1 cache: {}", key, e);
            }

            debug!("L2 cache hit and promoted to L1 for key: {}", key);
            return Ok(Some(value));
        }

        let mut metrics = self.metrics.write().await;
        metrics.l2_misses += 1;
        drop(metrics);

        // Try L3 cache (cold data)
        if let Some(l3_cache) = &self.l3_cache {
            if let Ok(Some(value)) = l3_cache.get::<T>(key).await {
                let mut metrics = self.metrics.write().await;
                metrics.l3_hits += 1;
                metrics.promotions += 1;
                drop(metrics);

                // Promote to L2 cache
                if let Err(e) = self.l2_cache.set(key, &value).await {
                    warn!("Failed to promote key {} to L2 cache: {}", key, e);
                }

                debug!("L3 cache hit and promoted to L2 for key: {}", key);
                return Ok(Some(value));
            }
        }

        let mut metrics = self.metrics.write().await;
        metrics.l3_misses += 1;
        drop(metrics);

        debug!("Cache miss for key: {}", key);
        Ok(None)
    }

    pub async fn set<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: serde::Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
    {
        // Store in L1 cache (hot data)
        self.l1_cache.set(key, value).await?;

        // Also store in L2 cache (warm data)
        self.l2_cache.set(key, value).await?;

        // Store in L3 cache if available
        if let Some(l3_cache) = &self.l3_cache {
            l3_cache.set(key, value, Some(self.config.l3_ttl_seconds)).await?;
        }

        debug!("Set cache entry for key: {} in all available levels", key);
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<bool> {
        let l1_deleted = self.l1_cache.delete(key).await;
        let l2_deleted = self.l2_cache.delete(key).await;
        let l3_deleted = if let Some(l3_cache) = &self.l3_cache {
            l3_cache.delete(key).await.unwrap_or(false)
        } else {
            false
        };

        let deleted = l1_deleted || l2_deleted || l3_deleted;
        if deleted {
            debug!("Deleted cache entry for key: {} from all levels", key);
        }
        Ok(deleted)
    }

    pub async fn clear_all(&self) -> Result<()> {
        self.l1_cache.clear();
        self.l2_cache.clear();

        if let Some(l3_cache) = &self.l3_cache {
            l3_cache.clear().await?;
        }

        info!("Cleared all cache levels");
        Ok(())
    }

    pub async fn warm_cache(&self, wallet_addresses: Vec<String>) -> Result<usize> {
        let warmed = self.cache_warmer.warm_wallet_cache(wallet_addresses).await?;
        let mut metrics = self.metrics.write().await;
        metrics.cache_warmups += 1;
        drop(metrics);
        Ok(warmed)
    }

    pub async fn get_metrics(&self) -> CacheMetrics {
        self.metrics.read().await.clone()
    }

    pub async fn get_stats(&self) -> HierarchicalCacheStats {
        let l1_stats = self.l1_cache.stats();
        let l2_stats = self.l2_cache.stats();
        let l3_size = if let Some(l3_cache) = &self.l3_cache {
            l3_cache.get_size().await.unwrap_or(0)
        } else {
            0
        };

        let metrics = self.metrics.read().await.clone();

        HierarchicalCacheStats {
            l1_entries: l1_stats.total_entries,
            l2_entries: l2_stats.total_entries,
            l3_entries: l3_size,
            l1_max_size: self.config.l1_max_size as u64,
            l2_max_size: self.config.l2_max_size as u64,
            l3_max_size: 10000000, // Redis max size
            metrics,
        }
    }

    pub async fn optimize_cache(&self) -> Result<()> {
        // Trigger cleanup in L2 cache
        self.l2_cache.cleanup_expired().await?;

        // Cleanup expired entries in L3 if available
        if let Some(l3_cache) = &self.l3_cache {
            l3_cache.cleanup_expired().await?;
        }

        info!("Cache optimization completed");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HierarchicalCacheStats {
    pub l1_entries: u64,
    pub l2_entries: u64,
    pub l3_entries: u64,
    pub l1_max_size: u64,
    pub l2_max_size: u64,
    pub l3_max_size: u64,
    pub metrics: CacheMetrics,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hierarchical_cache_basic() {
        let config = HierarchicalCacheConfig::default();
        let cache = HierarchicalCache::new(config).await.unwrap();

        let key = "test_key";
        let value = "test_value";

        // Test set and get
        cache.set(key, &value).await.unwrap();
        let retrieved: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        // Test delete
        let deleted = cache.delete(key).await.unwrap();
        assert!(deleted);

        let retrieved: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(retrieved, None);
    }

    #[tokio::test]
    async fn test_compression_engine() {
        let engine = CompressionEngine::new(true, 10);
        
        let data = b"This is a test string that should be compressed because it's longer than the threshold";
        let compressed = engine.compress(data).unwrap();
        let decompressed = engine.decompress(&compressed).unwrap();
        
        assert_eq!(data.to_vec(), decompressed);
        assert!(compressed.len() < data.len()); // Should be compressed
    }
}
