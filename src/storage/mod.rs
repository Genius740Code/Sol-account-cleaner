pub mod cache;
pub mod persistence;
pub mod redis_cache;
pub mod hierarchical_cache;
pub mod multi_level_cache;

#[cfg(test)]
mod tests;

pub use cache::{CacheManager, CacheConfig};
pub use persistence::{PersistenceManager, DatabaseConfig, SqlitePersistenceManager};
pub use redis_cache::{RedisCacheManager, CacheEntry, CacheMetrics};
pub use hierarchical_cache::{
    HierarchicalCache, HierarchicalCacheConfig, CachedWalletInfo, 
    CompressionEngine, CacheWarmer, CacheMetrics as HierarchicalCacheMetrics
};
pub use multi_level_cache::{
    MultiLevelCache, CachedAccount, AccountData, CachePriority,
    CacheMetrics as MultiLevelCacheMetrics, EvictionPolicy
};
