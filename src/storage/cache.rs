use crate::core::{Result, SolanaRecoverError};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheConfig {
    pub ttl_seconds: u64,
    pub max_size: usize,
    pub cleanup_interval_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 300, // 5 minutes
            max_size: 10000,
            cleanup_interval_seconds: 60, // 1 minute
        }
    }
}

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    created_at: Instant,
    ttl: Duration,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

pub struct CacheManager {
    data: Arc<DashMap<String, CacheEntry<serde_json::Value>>>,
    config: CacheConfig,
}

impl CacheManager {
    pub fn new(config: CacheConfig) -> Self {
        let cache = Self {
            data: Arc::new(DashMap::new()),
            config,
        };

        // Start cleanup task
        let data_clone = cache.data.clone();
        let cleanup_interval = Duration::from_secs(cache.config.cleanup_interval_seconds);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                Self::cleanup_expired(&data_clone);
            }
        });

        cache
    }

    fn cleanup_expired(data: &DashMap<String, CacheEntry<serde_json::Value>>) {
        let expired_keys: Vec<String> = data
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone())
            .collect();

        for key in expired_keys {
            data.remove(&key);
        }
    }

    pub fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(entry) = self.data.get(key) {
            if entry.is_expired() {
                drop(entry);
                self.data.remove(key);
                return Ok(None);
            }

            let value: T = serde_json::from_value(entry.value().value.clone())
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to deserialize cache value: {}", e)
                ))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub fn set<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        // Check if we need to evict entries
        if self.data.len() >= self.config.max_size {
            self.evict_lru();
        }

        let json_value = serde_json::to_value(value)
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to serialize cache value: {}", e)
            ))?;

        let entry = CacheEntry::new(
            json_value,
            Duration::from_secs(self.config.ttl_seconds)
        );

        self.data.insert(key.to_string(), entry);
        Ok(())
    }

    pub fn delete(&self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    pub fn clear(&self) {
        self.data.clear();
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn stats(&self) -> CacheStats {
        let total_entries = self.data.len();
        let expired_entries = self.data
            .iter()
            .filter(|entry| entry.value().is_expired())
            .count();

        CacheStats {
            total_entries,
            expired_entries,
            max_size: self.config.max_size,
            ttl_seconds: self.config.ttl_seconds,
        }
    }

    fn evict_lru(&self) {
        // Simple LRU implementation: remove the oldest entries
        let mut entries: Vec<_> = self.data
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().created_at))
            .collect();

        entries.sort_by_key(|(_, created_at)| *created_at);

        // Remove oldest 10% of entries
        let remove_count = (self.config.max_size / 10).max(1);
        for (key, _) in entries.iter().take(remove_count) {
            self.data.remove(key);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub max_size: usize,
    pub ttl_seconds: u64,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}
