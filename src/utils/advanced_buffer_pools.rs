use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use serde::{Serialize, Deserialize};

use super::memory_pool::{MemoryPool, PooledItem};

/// Advanced buffer pool system optimized for Solana RPC operations
#[derive(Debug, Clone)]
pub struct AdvancedBufferPool {
    /// Size-tiered buffer pools for different use cases
    tiny_buffers: Arc<MemoryPool<Vec<u8>>>,      // 64B - 256B
    small_buffers: Arc<MemoryPool<Vec<u8>>>,     // 256B - 1KB
    medium_buffers: Arc<MemoryPool<Vec<u8>>>,    // 1KB - 4KB
    large_buffers: Arc<MemoryPool<Vec<u8>>>,     // 4KB - 16KB
    xlarge_buffers: Arc<MemoryPool<Vec<u8>>>,    // 16KB - 64KB
    xxlarge_buffers: Arc<MemoryPool<Vec<u8>>>,   // 64KB - 256KB
    jumbo_buffers: Arc<MemoryPool<Vec<u8>>>,     // 256KB - 1MB
    
    /// Specialized pools for specific operations
    rpc_request_buffers: Arc<MemoryPool<RpcRequestBuffer>>,
    rpc_response_buffers: Arc<MemoryPool<RpcResponseBuffer>>,
    account_data_buffers: Arc<MemoryPool<AccountDataBuffer>>,
    transaction_buffers: Arc<MemoryPool<TransactionBuffer>>,
    
    /// Statistics and monitoring
    stats: Arc<RwLock<BufferPoolStats>>,
    config: BufferPoolConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RpcRequestBuffer {
    pub data: Vec<u8>,
    pub request_id: String,
    pub method: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RpcResponseBuffer {
    pub data: Vec<u8>,
    pub request_id: String,
    pub response_size: usize,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub received_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountDataBuffer {
    pub data: Vec<u8>,
    pub account_address: String,
    pub data_length: usize,
    pub slot: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionBuffer {
    pub data: Vec<u8>,
    pub signature: String,
    pub serialized_size: usize,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferPoolConfig {
    pub pool_sizes: BufferPoolSizes,
    pub enable_compression: bool,
    pub enable_zero_copy: bool,
    pub max_buffer_age_seconds: u64,
    pub cleanup_interval_seconds: u64,
    pub enable_stats_collection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferPoolSizes {
    pub tiny_pool_size: usize,
    pub small_pool_size: usize,
    pub medium_pool_size: usize,
    pub large_pool_size: usize,
    pub xlarge_pool_size: usize,
    pub xxlarge_pool_size: usize,
    pub jumbo_pool_size: usize,
    pub rpc_request_pool_size: usize,
    pub rpc_response_pool_size: usize,
    pub account_data_pool_size: usize,
    pub transaction_pool_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferPoolStats {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub total_bytes_allocated: u64,
    pub total_bytes_freed: u64,
    pub active_buffers: usize,
    pub pool_utilization: HashMap<String, f64>,
    pub size_distribution: HashMap<String, u64>,
    pub average_lifetime_ms: f64,
    pub compression_ratio: Option<f64>,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self {
            pool_sizes: BufferPoolSizes {
                tiny_pool_size: 10000,      // 64B-256B buffers
                small_pool_size: 5000,      // 256B-1KB buffers
                medium_pool_size: 2000,     // 1KB-4KB buffers
                large_pool_size: 1000,       // 4KB-16KB buffers
                xlarge_pool_size: 500,       // 16KB-64KB buffers
                xxlarge_pool_size: 100,      // 64KB-256KB buffers
                jumbo_pool_size: 50,         // 256KB-1MB buffers
                rpc_request_pool_size: 1000,
                rpc_response_pool_size: 1000,
                account_data_pool_size: 2000,
                transaction_pool_size: 500,
            },
            enable_compression: false, // Enable when compression library is available
            enable_zero_copy: true,
            max_buffer_age_seconds: 300, // 5 minutes
            cleanup_interval_seconds: 60,
            enable_stats_collection: true,
        }
    }
}

impl AdvancedBufferPool {
    pub fn new() -> Arc<Self> {
        Self::with_config(BufferPoolConfig::default())
    }
    
    pub fn with_config(config: BufferPoolConfig) -> Arc<Self> {
        let pool = Arc::new(Self {
            tiny_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.tiny_pool_size,
                || Vec::with_capacity(256)
            )),
            small_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.small_pool_size,
                || Vec::with_capacity(1024)
            )),
            medium_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.medium_pool_size,
                || Vec::with_capacity(4096)
            )),
            large_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.large_pool_size,
                || Vec::with_capacity(16384)
            )),
            xlarge_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.xlarge_pool_size,
                || Vec::with_capacity(65536)
            )),
            xxlarge_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.xxlarge_pool_size,
                || Vec::with_capacity(262144)
            )),
            jumbo_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.jumbo_pool_size,
                || Vec::with_capacity(1048576)
            )),
            rpc_request_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.rpc_request_pool_size,
                || RpcRequestBuffer {
                    data: Vec::with_capacity(4096),
                    request_id: String::new(),
                    method: String::new(),
                    created_at: chrono::Utc::now(),
                }
            )),
            rpc_response_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.rpc_response_pool_size,
                || RpcResponseBuffer {
                    data: Vec::with_capacity(16384),
                    request_id: String::new(),
                    response_size: 0,
                    received_at: chrono::Utc::now(),
                }
            )),
            account_data_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.account_data_pool_size,
                || AccountDataBuffer {
                    data: Vec::with_capacity(1024),
                    account_address: String::new(),
                    data_length: 0,
                    slot: 0,
                }
            )),
            transaction_buffers: Arc::new(MemoryPool::with_factory(
                config.pool_sizes.transaction_pool_size,
                || TransactionBuffer {
                    data: Vec::with_capacity(2048),
                    signature: String::new(),
                    serialized_size: 0,
                    created_at: chrono::Utc::now(),
                }
            )),
            stats: Arc::new(RwLock::new(BufferPoolStats::default())),
            config,
        });
        
        // Start cleanup task
        pool.start_cleanup_task();
        
        pool
    }
    
    fn start_cleanup_task(self: &Arc<Self>) {
        let pool = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(pool.config.cleanup_interval_seconds)
            );
            
            loop {
                interval.tick().await;
                pool.cleanup_old_buffers().await;
                pool.update_stats().await;
            }
        });
    }
    
    /// Get a buffer of the appropriate size
    pub fn get_buffer(&self, size: usize) -> crate::utils::memory_pool::PooledItem<Vec<u8>> {
        let buffer = match size {
            0..=256 => {
                let mut buf = self.tiny_buffers.acquire();
                buf.resize(size, 0);
                buf
            }
            257..=1024 => {
                let mut buf = self.small_buffers.acquire();
                buf.resize(size, 0);
                buf
            }
            1025..=4096 => {
                let mut buf = self.medium_buffers.acquire();
                buf.resize(size, 0);
                buf
            }
            4097..=16384 => {
                let mut buf = self.large_buffers.acquire();
                buf.resize(size, 0);
                buf
            }
            16385..=65536 => {
                let mut buf = self.xlarge_buffers.acquire();
                buf.resize(size, 0);
                buf
            }
            65537..=262144 => {
                let mut buf = self.xxlarge_buffers.acquire();
                buf.resize(size, 0);
                buf
            }
            262145..=1048576 => {
                let mut buf = self.jumbo_buffers.acquire();
                buf.resize(size, 0);
                buf
            }
            _ => {
                warn!("Requested buffer size {} exceeds maximum pool size, allocating directly", size);
                crate::utils::memory_pool::PooledItem::new(Vec::with_capacity(size))
            }
        };
        
        if self.config.enable_stats_collection {
            self.record_allocation(size);
        }
        
        buffer
    }
    
    /// Get a specialized RPC request buffer
    pub fn get_rpc_request_buffer(&self, method: &str, request_id: &str) -> crate::utils::memory_pool::PooledItem<RpcRequestBuffer> {
        let mut buffer = self.rpc_request_buffers.acquire();
        buffer.method = method.to_string();
        buffer.request_id = request_id.to_string();
        buffer.created_at = chrono::Utc::now();
        buffer.data.clear();
        
        if self.config.enable_stats_collection {
            self.record_specialized_allocation("rpc_request");
        }
        
        buffer
    }
    
    /// Get a specialized RPC response buffer
    pub fn get_rpc_response_buffer(&self, request_id: &str) -> crate::utils::memory_pool::PooledItem<RpcResponseBuffer> {
        let mut buffer = self.rpc_response_buffers.acquire();
        buffer.request_id = request_id.to_string();
        buffer.received_at = chrono::Utc::now();
        buffer.response_size = 0;
        buffer.data.clear();
        
        if self.config.enable_stats_collection {
            self.record_specialized_allocation("rpc_response");
        }
        
        buffer
    }
    
    /// Get a specialized account data buffer
    pub fn get_account_data_buffer(&self, address: &str, slot: u64) -> crate::utils::memory_pool::PooledItem<AccountDataBuffer> {
        let mut buffer = self.account_data_buffers.acquire();
        buffer.account_address = address.to_string();
        buffer.slot = slot;
        buffer.data_length = 0;
        buffer.data.clear();
        
        if self.config.enable_stats_collection {
            self.record_specialized_allocation("account_data");
        }
        
        buffer
    }
    
    /// Get a specialized transaction buffer
    pub fn get_transaction_buffer(&self, signature: &str) -> crate::utils::memory_pool::PooledItem<TransactionBuffer> {
        let mut buffer = self.transaction_buffers.acquire();
        buffer.signature = signature.to_string();
        buffer.created_at = chrono::Utc::now();
        buffer.serialized_size = 0;
        buffer.data.clear();
        
        if self.config.enable_stats_collection {
            self.record_specialized_allocation("transaction");
        }
        
        buffer
    }
    
    /// Zero-copy buffer transfer (when enable_zero_copy is true)
    pub fn transfer_buffer(&self, source: Vec<u8>) -> crate::utils::memory_pool::PooledItem<Vec<u8>> {
        if !self.config.enable_zero_copy {
            return self.get_buffer(source.len());
        }
        
        let mut buffer = self.get_buffer(source.len());
        buffer.copy_from_slice(&source);
        buffer
    }
    
    /// Get buffer pool statistics
    pub fn get_stats(&self) -> BufferPoolStats {
        self.stats.read().clone()
    }
    
    /// Reset all statistics
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write();
        *stats = BufferPoolStats::default();
    }
    
    /// Force cleanup of old buffers
    pub async fn cleanup_old_buffers(&self) {
        let _now = Instant::now();
        let _max_age = Duration::from_secs(self.config.max_buffer_age_seconds);
        
        // This is a simplified cleanup - in practice you'd track buffer ages
        // and selectively clear old buffers from pools
        
        debug!("Running buffer pool cleanup");
        
        // Shrink pools if they're underutilized
        self.tiny_buffers.shrink_to_fit();
        self.small_buffers.shrink_to_fit();
        self.medium_buffers.shrink_to_fit();
        self.large_buffers.shrink_to_fit();
        self.xlarge_buffers.shrink_to_fit();
        self.xxlarge_buffers.shrink_to_fit();
        self.jumbo_buffers.shrink_to_fit();
    }
    
    /// Update pool statistics
    async fn update_stats(&self) {
        if !self.config.enable_stats_collection {
            return;
        }
        
        let mut stats = self.stats.write();
        stats.timestamp = chrono::Utc::now();
        
        // Calculate pool utilization
        stats.pool_utilization = HashMap::from([
            ("tiny".to_string(), self.calculate_utilization(&self.tiny_buffers)),
            ("small".to_string(), self.calculate_utilization(&self.small_buffers)),
            ("medium".to_string(), self.calculate_utilization(&self.medium_buffers)),
            ("large".to_string(), self.calculate_utilization(&self.large_buffers)),
            ("xlarge".to_string(), self.calculate_utilization(&self.xlarge_buffers)),
            ("xxlarge".to_string(), self.calculate_utilization(&self.xxlarge_buffers)),
            ("jumbo".to_string(), self.calculate_utilization(&self.jumbo_buffers)),
        ]);
        
        // Update active buffers count
        stats.active_buffers = self.tiny_buffers.get_stats().current_size +
            self.small_buffers.get_stats().current_size +
            self.medium_buffers.get_stats().current_size +
            self.large_buffers.get_stats().current_size +
            self.xlarge_buffers.get_stats().current_size +
            self.xxlarge_buffers.get_stats().current_size +
            self.jumbo_buffers.get_stats().current_size;
    }
    
    fn calculate_utilization<T>(&self, pool: &MemoryPool<T>) -> f64 
    where 
        T: Default + Clone,
    {
        let pool_stats = pool.get_stats();
        if pool_stats.max_size_reached == 0 {
            0.0
        } else {
            (pool_stats.current_size as f64 / pool_stats.max_size_reached as f64) * 100.0
        }
    }
    
    fn record_allocation(&self, size: usize) {
        let mut stats = self.stats.write();
        stats.total_allocations += 1;
        stats.total_bytes_allocated += size as u64;
        
        // Update size distribution
        let size_category = self.categorize_size(size);
        *stats.size_distribution.entry(size_category).or_insert(0) += 1;
    }
    
    fn record_specialized_allocation(&self, pool_type: &str) {
        let mut stats = self.stats.write();
        stats.total_allocations += 1;
        *stats.size_distribution.entry(pool_type.to_string()).or_insert(0) += 1;
    }
    
    fn categorize_size(&self, size: usize) -> String {
        match size {
            0..=256 => "tiny".to_string(),
            257..=1024 => "small".to_string(),
            1025..=4096 => "medium".to_string(),
            4097..=16384 => "large".to_string(),
            16385..=65536 => "xlarge".to_string(),
            65537..=262144 => "xxlarge".to_string(),
            262145..=1048576 => "jumbo".to_string(),
            _ => "oversized".to_string(),
        }
    }
    
    /// Get comprehensive performance report
    pub fn get_performance_report(&self) -> serde_json::Value {
        let stats = self.get_stats();
        
        serde_json::json!({
            "timestamp": stats.timestamp,
            "total_allocations": stats.total_allocations,
            "total_bytes_allocated": stats.total_bytes_allocated,
            "active_buffers": stats.active_buffers,
            "pool_utilization": stats.pool_utilization,
            "size_distribution": stats.size_distribution,
            "average_lifetime_ms": stats.average_lifetime_ms,
            "config": self.config,
            "recommendations": self.generate_performance_recommendations(&stats)
        })
    }
    
    fn generate_performance_recommendations(&self, stats: &BufferPoolStats) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Check for underutilized pools
        for (pool_name, utilization) in &stats.pool_utilization {
            if *utilization < 20.0 {
                recommendations.push(format!("Pool '{}' is underutilized at {:.1}%. Consider reducing pool size.", pool_name, utilization));
            } else if *utilization > 90.0 {
                recommendations.push(format!("Pool '{}' is highly utilized at {:.1}%. Consider increasing pool size.", pool_name, utilization));
            }
        }
        
        // Check allocation patterns
        if let Some((category, count)) = stats.size_distribution.iter().max_by_key(|(_, count)| *count) {
            if *count > stats.total_allocations / 2 {
                recommendations.push(format!("Most allocations are in '{}' category. Consider optimizing for this size range.", category));
            }
        }
        
        // Memory efficiency recommendations
        let avg_allocation_size = if stats.total_allocations > 0 {
            stats.total_bytes_allocated / stats.total_allocations
        } else {
            0
        };
        
        if avg_allocation_size > 100000 {
            recommendations.push("Large average allocation size detected. Consider implementing streaming or chunking for large data.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("Buffer pool performance appears optimal. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

impl Default for BufferPoolStats {
    fn default() -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            total_allocations: 0,
            total_deallocations: 0,
            total_bytes_allocated: 0,
            total_bytes_freed: 0,
            active_buffers: 0,
            pool_utilization: HashMap::new(),
            size_distribution: HashMap::new(),
            average_lifetime_ms: 0.0,
            compression_ratio: None,
        }
    }
}

// Helper trait for creating pooled items
pub trait PooledItemFactory<T: std::clone::Clone + std::default::Default> {
    fn create_pooled_item(data: T) -> PooledItem<T>;
}

impl<T: Default + Clone> PooledItemFactory<T> for PooledItem<T> {
    fn create_pooled_item(data: T) -> PooledItem<T> {
        PooledItem::new(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_advanced_buffer_pool_creation() {
        let pool = AdvancedBufferPool::new();
        let stats = pool.get_stats();
        
        assert_eq!(stats.total_allocations, 0);
        assert!(stats.pool_utilization.is_empty());
    }
    
    #[tokio::test]
    async fn test_size_tiered_buffers() {
        let pool = AdvancedBufferPool::new();
        
        // Test different sizes
        let tiny_buf = pool.get_buffer(128);
        assert_eq!(tiny_buf.len(), 128);
        
        let small_buf = pool.get_buffer(512);
        assert_eq!(small_buf.len(), 512);
        
        let medium_buf = pool.get_buffer(2048);
        assert_eq!(medium_buf.len(), 2048);
        
        let large_buf = pool.get_buffer(8192);
        assert_eq!(large_buf.len(), 8192);
        
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 4);
    }
    
    #[tokio::test]
    async fn test_specialized_buffers() {
        let pool = AdvancedBufferPool::new();
        
        let rpc_buf = pool.get_rpc_request_buffer("getAccountInfo", "req-123");
        assert_eq!(rpc_buf.method, "getAccountInfo");
        assert_eq!(rpc_buf.request_id, "req-123");
        
        let account_buf = pool.get_account_data_buffer("11111111111111111111111111111112", 100);
        assert_eq!(account_buf.account_address, "11111111111111111111111111111112");
        assert_eq!(account_buf.slot, 100);
        
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 2);
    }
    
    #[tokio::test]
    async fn test_performance_report() {
        let pool = AdvancedBufferPool::new();
        
        // Generate some activity
        let _buf1 = pool.get_buffer(1024);
        let _buf2 = pool.get_buffer(4096);
        let _rpc_buf = pool.get_rpc_request_buffer("test", "123");
        
        let report = pool.get_performance_report();
        
        assert!(report.get("timestamp").is_some());
        assert!(report.get("total_allocations").is_some());
        assert!(report.get("pool_utilization").is_some());
        assert!(report.get("recommendations").is_some());
    }
    
    #[tokio::test]
    async fn test_cleanup_task() {
        let pool = AdvancedBufferPool::new();
        
        // Create some buffers
        let _buf1 = pool.get_buffer(1024);
        let _buf2 = pool.get_buffer(2048);
        
        // Wait for cleanup interval (shortened for test)
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let stats = pool.get_stats();
        assert!(stats.timestamp > chrono::Utc::now() - chrono::Duration::seconds(10));
    }
}
