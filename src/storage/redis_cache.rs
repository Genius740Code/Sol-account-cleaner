use crate::core::{Result, SolanaRecoverError};
use crate::storage::CacheConfig;
use redis::{Client, AsyncCommands, cmd};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use std::sync::Arc;
use tracing::{debug, warn, error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub data: T,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub access_count: u64,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
}

impl<T> CacheEntry<T> {
    pub fn new(data: T, ttl_seconds: Option<u64>) -> Self {
        let now = chrono::Utc::now();
        Self {
            data,
            created_at: now,
            expires_at: ttl_seconds.map(|ttl| now + chrono::Duration::seconds(ttl as i64)),
            access_count: 0,
            last_accessed: now,
        }
    }
    
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now() > expires_at
        } else {
            false
        }
    }
    
    pub fn access(&mut self) -> &T {
        self.access_count += 1;
        self.last_accessed = chrono::Utc::now();
        &self.data
    }
}

pub struct RedisCacheManager {
    client: Arc<Client>,
    config: CacheConfig,
    metrics: Arc<RwLock<CacheMetrics>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub deletes: u64,
    pub errors: u64,
    pub total_connections: u64,
    pub active_connections: u64,
}

impl RedisCacheManager {
    pub async fn new(config: CacheConfig) -> Result<Self> {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());
        
        let client = Client::open(redis_url.as_str())
            .map_err(|e| SolanaRecoverError::DatabaseError(format!("Failed to create Redis client: {}", e)))?;
        
        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await
            .map_err(|e| SolanaRecoverError::DatabaseError(format!("Failed to connect to Redis: {}", e)))?;
        
        // Ping to verify connection
        let _: String = cmd("PING").query_async(&mut conn).await
            .map_err(|e| SolanaRecoverError::DatabaseError(format!("Redis ping failed: {}", e)))?;
        
        info!("Connected to Redis at {}", redis_url);
        
        Ok(Self {
            client: Arc::new(client),
            config,
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
        })
    }
    
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        let mut conn = self.get_connection().await?;
        
        let result: Option<String> = conn.get(key).await
            .map_err(|e| {
                error!("Redis get error for key '{}': {}", key, e);
                self.increment_errors();
                SolanaRecoverError::DatabaseError(format!("Redis get error: {}", e))
            })?;
        
        if let Some(data_str) = result.as_deref() {
            let entry: CacheEntry<T> = serde_json::from_str(&data_str)
                .map_err(|e| {
                    warn!("Failed to deserialize cache entry for key '{}': {}", key, e);
                    SolanaRecoverError::SerializationError(format!("Deserialization failed: {}", e))
                })?;
            
            if entry.is_expired() {
                debug!("Cache entry expired for key '{}'", key);
                // Delete expired entry
                let _: () = conn.del(key).await.unwrap_or(());
                self.increment_misses();
                return Ok(None);
            }
            
            debug!("Cache hit for key '{}'", key);
            self.increment_hits();
            Ok(Some(entry.data))
        } else {
            debug!("Cache miss for key '{}'", key);
            self.increment_misses();
            Ok(None)
        }
    }
    
    pub async fn set<T>(&self, key: &str, value: &T, ttl_seconds: Option<u64>) -> Result<()>
    where
        T: Serialize + Send + Sync + 'static,
    {
        let mut conn = self.get_connection().await?;
        
        let ttl = ttl_seconds.or(Some(self.config.ttl_seconds));
        let entry = CacheEntry::new(value, ttl);
        
        let data_str = serde_json::to_string(&entry)
            .map_err(|e| SolanaRecoverError::SerializationError(format!("Serialization failed: {}", e)))?;
        
        let _: () = conn.set(key, data_str).await
            .map_err(|e| {
                error!("Redis set error for key '{}': {}", key, e);
                self.increment_errors();
                SolanaRecoverError::DatabaseError(format!("Redis set error: {}", e))
            })?;
        
        // Set TTL if specified
        if let Some(ttl) = ttl {
            if let Err(e) = conn.expire::<_, ()>(key, ttl as i64).await {
                warn!("Failed to set TTL for key '{}': {}", key, e);
                // Non-critical error, don't return error
            }
        }
        
        debug!("Cache set for key '{}' with TTL {:?}", key, ttl);
        self.increment_sets();
        Ok(())
    }
    
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        
        let result: i32 = conn.del(key).await
            .map_err(|e| {
                error!("Redis delete error for key '{}': {}", key, e);
                self.increment_errors();
                SolanaRecoverError::DatabaseError(format!("Redis delete error: {}", e))
            })?;
        
        let deleted = result > 0;
        if deleted {
            debug!("Cache delete for key '{}'", key);
            self.increment_deletes();
        }
        
        Ok(deleted)
    }
    
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        
        let result: i32 = conn.exists(key).await
            .map_err(|e| {
                error!("Redis exists error for key '{}': {}", key, e);
                self.increment_errors();
                SolanaRecoverError::DatabaseError(format!("Redis exists error: {}", e))
            })?;
        
        Ok(result > 0)
    }
    
    pub async fn clear(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        
        let _: () = cmd("FLUSHDB").query_async(&mut conn).await
            .map_err(|e| {
                error!("Redis clear error: {}", e);
                self.increment_errors();
                SolanaRecoverError::DatabaseError(format!("Redis clear error: {}", e))
            })?;
        
        info!("Redis cache cleared");
        Ok(())
    }
    
    pub async fn get_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        
        let keys: Vec<String> = conn.keys(pattern).await
            .map_err(|e| {
                error!("Redis keys error for pattern '{}': {}", pattern, e);
                self.increment_errors();
                SolanaRecoverError::DatabaseError(format!("Redis keys error: {}", e))
            })?;
        
        Ok(keys)
    }
    
    pub async fn get_size(&self) -> Result<u64> {
        let mut conn = self.get_connection().await?;
        
        let size: u64 = cmd("DBSIZE").query_async(&mut conn).await
            .map_err(|e| {
                error!("Redis dbsize error: {}", e);
                self.increment_errors();
                SolanaRecoverError::DatabaseError(format!("Redis dbsize error: {}", e))
            })?;
        
        Ok(size)
    }
    
    pub async fn cleanup_expired(&self) -> Result<u64> {
        let keys = self.get_keys("*").await?;
        let mut cleaned = 0u64;
        
        for key in keys {
            if let Ok(Some(entry_str)) = self.get_raw(&key).await {
                if let Ok(entry) = serde_json::from_str::<CacheEntry<serde_json::Value>>(&entry_str) {
                    if entry.is_expired() {
                        if self.delete(&key).await.unwrap_or(false) {
                            cleaned += 1;
                        }
                    }
                }
            }
        }
        
        if cleaned > 0 {
            info!("Cleaned up {} expired cache entries", cleaned);
        }
        
        Ok(cleaned)
    }
    
    async fn get_raw(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        
        let result: Option<String> = conn.get(key).await
            .map_err(|e| SolanaRecoverError::DatabaseError(format!("Redis get error: {}", e)))?;
        
        Ok(result)
    }
    
    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection> {
        let conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| SolanaRecoverError::DatabaseError(format!("Failed to get Redis connection: {}", e)))?;
        
        self.increment_active_connections();
        Ok(conn)
    }
    
    fn increment_hits(&self) {
        let mut metrics = self.metrics.blocking_write();
        metrics.hits += 1;
    }
    
    fn increment_misses(&self) {
        let mut metrics = self.metrics.blocking_write();
        metrics.misses += 1;
    }
    
    fn increment_sets(&self) {
        let mut metrics = self.metrics.blocking_write();
        metrics.sets += 1;
    }
    
    fn increment_deletes(&self) {
        let mut metrics = self.metrics.blocking_write();
        metrics.deletes += 1;
    }
    
    fn increment_errors(&self) {
        let mut metrics = self.metrics.blocking_write();
        metrics.errors += 1;
    }
    
    fn increment_active_connections(&self) {
        let mut metrics = self.metrics.blocking_write();
        metrics.active_connections += 1;
        metrics.total_connections += 1;
    }
    
    pub fn get_metrics(&self) -> CacheMetrics {
        self.metrics.blocking_read().clone()
    }
    
    pub fn get_hit_rate(&self) -> f64 {
        let metrics = self.metrics.blocking_read();
        let total_requests = metrics.hits + metrics.misses;
        if total_requests == 0 {
            0.0
        } else {
            metrics.hits as f64 / total_requests as f64
        }
    }
}

impl Drop for RedisCacheManager {
    fn drop(&mut self) {
        // Note: Redis connections are automatically dropped when they go out of scope
        debug!("RedisCacheManager dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cache_set_get() {
        let config = CacheConfig {
            ttl_seconds: 60,
            max_size: 1000,
            cleanup_interval_seconds: 300,
            enable_hierarchical_cache: false,
            l1_cache_size: 100,
            l2_cache_size: 200,
            compression_threshold: 1024,
            enable_metrics: false,
        };
        
        // This test requires Redis to be running
        // In production, you'd use a test Redis instance
        if std::env::var("SKIP_REDIS_TESTS").is_ok() {
            return;
        }
        
        match RedisCacheManager::new(config).await {
            Ok(cache) => {
                let key = "test_key";
                let value = "test_value";
                
                // Set value
                cache.set(key, &value, Some(60)).await.unwrap();
                
                // Get value
                let retrieved: Option<String> = cache.get(key).await.unwrap();
                assert_eq!(retrieved, Some(value.to_string()));
                
                // Delete value
                let deleted = cache.delete(key).await.unwrap();
                assert!(deleted);
                
                // Verify deletion
                let retrieved: Option<String> = cache.get(key).await.unwrap();
                assert_eq!(retrieved, None);
            }
            Err(_) => {
                println!("Redis not available, skipping test");
            }
        }
    }
}
