use std::sync::Arc;
use parking_lot::{RwLock, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{info, debug};
use serde::{Serialize, Deserialize};
use chrono::Utc;

use crate::core::types::{WalletInfo, EmptyAccount, ScanResult, BatchScanResult, RecoveryTransaction};
use super::memory_pool::{MemoryPool, MemoryManager};

/// Enhanced memory manager with specialized object pools for Solana Account Cleaner
#[derive(Debug)]
pub struct EnhancedMemoryManager {
    /// Core memory manager for basic operations
    base_manager: Arc<MemoryManager>,
    
    /// Specialized object pools for frequently used types
    wallet_info_pool: Arc<MemoryPool<WalletInfo>>,
    empty_account_pool: Arc<MemoryPool<EmptyAccount>>,
    scan_result_pool: Arc<MemoryPool<ScanResult>>,
    batch_scan_result_pool: Arc<MemoryPool<BatchScanResult>>,
    recovery_transaction_pool: Arc<MemoryPool<RecoveryTransaction>>,
    
    /// String pools for common string operations
    string_pool: Arc<MemoryPool<String>>,
    vec_string_pool: Arc<MemoryPool<Vec<String>>>,
    vec_u8_pool: Arc<MemoryPool<Vec<u8>>>,
    
    /// Memory monitoring and optimization
    memory_monitor: Arc<MemoryMonitor>,
    gc_scheduler: Arc<GcScheduler>,
    
    /// Configuration
    config: MemoryManagerConfig,
    
    /// Statistics
    stats: Arc<RwLock<EnhancedMemoryStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryManagerConfig {
    pub max_pool_sizes: PoolSizes,
    pub gc_config: GcConfig,
    pub monitoring_config: MonitoringConfig,
    pub enable_object_pooling: bool,
    pub enable_memory_monitoring: bool,
    pub enable_auto_optimization: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSizes {
    pub wallet_info_pool: usize,
    pub empty_account_pool: usize,
    pub scan_result_pool: usize,
    pub batch_scan_result_pool: usize,
    pub recovery_transaction_pool: usize,
    pub string_pool: usize,
    pub vec_string_pool: usize,
    pub vec_u8_pool: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    pub interval_seconds: u64,
    pub memory_threshold_percent: f64,
    pub force_gc_interval_seconds: u64,
    pub enable_adaptive_gc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub collection_interval_seconds: u64,
    pub enable_leak_detection: bool,
    pub leak_detection_threshold_seconds: u64,
    pub enable_memory_profiling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedMemoryStats {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub total_allocated_bytes: usize,
    pub peak_allocated_bytes: usize,
    pub pool_stats: HashMap<String, PoolStats>,
    pub gc_stats: GcStats,
    pub memory_pressure: f64,
    pub fragmentation_ratio: f64,
    pub leak_detection_stats: LeakDetectionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub name: String,
    pub current_size: usize,
    pub max_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub allocations: u64,
    pub deallocations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcStats {
    pub total_collections: u64,
    pub total_time_ms: u64,
    pub average_time_ms: f64,
    pub last_collection_time: Option<chrono::DateTime<chrono::Utc>>,
    pub memory_freed_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakDetectionStats {
    pub active_allocations: usize,
    pub potential_leaks: usize,
    pub total_leaked_bytes: usize,
    pub oldest_leak_age_seconds: Option<u64>,
}

/// Enhanced memory monitor with advanced features
#[derive(Debug)]
pub struct MemoryMonitor {
    #[allow(dead_code)]
    stats: Arc<RwLock<EnhancedMemoryStats>>,
    config: MonitoringConfig,
    leak_detector: Arc<LeakDetector>,
}

/// Garbage collection scheduler
#[derive(Debug)]
pub struct GcScheduler {
    config: GcConfig,
    last_gc: Arc<Mutex<Instant>>,
    gc_semaphore: Arc<Semaphore>,
}

/// Memory leak detector
#[derive(Debug)]
pub struct LeakDetector {
    allocations: Arc<RwLock<HashMap<String, AllocationInfo>>>,
    enabled: bool,
    threshold_seconds: u64,
}

#[derive(Debug, Clone)]
struct AllocationInfo {
    #[allow(dead_code)]
    pool_name: String,
    size: usize,
    timestamp: Instant,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            max_pool_sizes: PoolSizes {
                wallet_info_pool: 10000,
                empty_account_pool: 50000,
                scan_result_pool: 10000,
                batch_scan_result_pool: 1000,
                recovery_transaction_pool: 5000,
                string_pool: 100000,
                vec_string_pool: 20000,
                vec_u8_pool: 50000,
            },
            gc_config: GcConfig {
                interval_seconds: 60,
                memory_threshold_percent: 80.0,
                force_gc_interval_seconds: 300,
                enable_adaptive_gc: true,
            },
            monitoring_config: MonitoringConfig {
                collection_interval_seconds: 30,
                enable_leak_detection: true,
                leak_detection_threshold_seconds: 300,
                enable_memory_profiling: true,
            },
            enable_object_pooling: true,
            enable_memory_monitoring: true,
            enable_auto_optimization: true,
        }
    }
}

impl EnhancedMemoryManager {
    pub fn new() -> Arc<Self> {
        Self::with_config(MemoryManagerConfig::default())
    }
    
    pub fn with_config(config: MemoryManagerConfig) -> Arc<Self> {
        let base_manager = Arc::new(MemoryManager::with_config(
            (config.max_pool_sizes.wallet_info_pool * 1024) as usize, // Rough estimate
            Duration::from_secs(config.gc_config.interval_seconds),
        ));
        
        let manager = Arc::new(Self {
            base_manager,
            wallet_info_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.wallet_info_pool,
                || WalletInfo {
                    address: String::new(),
                    pubkey: solana_sdk::pubkey::Pubkey::default(),
                    total_accounts: 0,
                    empty_accounts: 0,
                    recoverable_lamports: 0,
                    recoverable_sol: 0.0,
                    empty_account_addresses: Vec::new(),
                    scan_time_ms: 0,
                }
            )),
            empty_account_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.empty_account_pool,
                || EmptyAccount {
                    address: String::new(),
                    lamports: 0,
                    owner: String::new(),
                    mint: None,
                }
            )),
            scan_result_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.scan_result_pool,
                || ScanResult {
                    id: uuid::Uuid::new_v4(),
                    wallet_address: String::new(),
                    status: crate::core::types::ScanStatus::Pending,
                    result: None,
                    empty_accounts_found: 0,
                    recoverable_sol: 0.0,
                    scan_time_ms: 0,
                    created_at: Utc::now(),
                    completed_at: None,
                    error_message: None,
                }
            )),
            batch_scan_result_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.batch_scan_result_pool,
                || BatchScanResult {
                    request_id: uuid::Uuid::new_v4(),
                    batch_id: None,
                    total_wallets: 0,
                    successful_scans: 0,
                    failed_scans: 0,
                    completed_wallets: 0,
                    failed_wallets: 0,
                    total_recoverable_sol: 0.0,
                    estimated_fee_sol: 0.0,
                    results: Vec::new(),
                    created_at: Utc::now(),
                    completed_at: None,
                    duration_ms: None,
                    scan_time_ms: 0,
                }
            )),
            recovery_transaction_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.recovery_transaction_pool,
                || RecoveryTransaction {
                    id: uuid::Uuid::new_v4(),
                    recovery_request_id: uuid::Uuid::new_v4(),
                    transaction_signature: String::new(),
                    transaction_data: Vec::new(),
                    accounts_recovered: Vec::new(),
                    lamports_recovered: 0,
                    fee_paid: 0,
                    status: crate::core::types::TransactionStatus::Pending,
                    created_at: Utc::now(),
                    signed_at: None,
                    confirmed_at: None,
                    error: None,
                }
            )),
            string_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.string_pool,
                || String::new()
            )),
            vec_string_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.vec_string_pool,
                || Vec::<String>::new()
            )),
            vec_u8_pool: Arc::new(MemoryPool::with_factory(
                config.max_pool_sizes.vec_u8_pool,
                || Vec::<u8>::new()
            )),
            memory_monitor: Arc::new(MemoryMonitor::new(config.monitoring_config.clone())),
            gc_scheduler: Arc::new(GcScheduler::new(config.gc_config.clone())),
            config,
            stats: Arc::new(RwLock::new(EnhancedMemoryStats::default())),
        });
        
        // Start background tasks
        manager.start_background_tasks();
        
        manager
    }
    
    fn start_background_tasks(self: &Arc<Self>) {
        if self.config.enable_memory_monitoring {
            self.start_memory_monitoring();
        }
        
        if self.config.enable_auto_optimization {
            self.start_auto_optimization();
        }
    }
    
    fn start_memory_monitoring(self: &Arc<Self>) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(manager.config.monitoring_config.collection_interval_seconds)
            );
            
            loop {
                interval.tick().await;
                manager.collect_memory_stats().await;
                
                // Check for memory pressure
                manager.collect_memory_stats().await;
                let stats = manager.get_memory_stats();
                if stats.memory_pressure > manager.config.gc_config.memory_threshold_percent {
                    debug!("Memory pressure detected: {:.1}%", stats.memory_pressure);
                    manager.gc_scheduler.trigger_gc_if_needed(&manager).await;
                }
            }
        });
    }
    
    fn start_auto_optimization(self: &Arc<Self>) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(manager.config.gc_config.force_gc_interval_seconds)
            );
            
            loop {
                interval.tick().await;
                manager.base_manager.optimize_memory();
            }
        });
    }
    
    // Object pool access methods
    pub fn acquire_wallet_info(&self) -> crate::utils::memory_pool::PooledItem<WalletInfo> {
        self.wallet_info_pool.acquire()
    }
    
    pub fn acquire_empty_account(&self) -> crate::utils::memory_pool::PooledItem<EmptyAccount> {
        self.empty_account_pool.acquire()
    }
    
    pub fn acquire_scan_result(&self) -> crate::utils::memory_pool::PooledItem<ScanResult> {
        self.scan_result_pool.acquire()
    }
    
    pub fn acquire_batch_scan_result(&self) -> crate::utils::memory_pool::PooledItem<BatchScanResult> {
        self.batch_scan_result_pool.acquire()
    }
    
    pub fn acquire_recovery_transaction(&self) -> crate::utils::memory_pool::PooledItem<RecoveryTransaction> {
        self.recovery_transaction_pool.acquire()
    }
    
    pub fn acquire_string(&self) -> crate::utils::memory_pool::PooledItem<String> {
        self.string_pool.acquire()
    }
    
    pub fn acquire_vec_string(&self) -> crate::utils::memory_pool::PooledItem<Vec<String>> {
        self.vec_string_pool.acquire()
    }
    
    pub fn acquire_vec_u8(&self) -> crate::utils::memory_pool::PooledItem<Vec<u8>> {
        self.vec_u8_pool.acquire()
    }
    
    // Memory management methods
    pub async fn collect_memory_stats(&self) {
        let timestamp = Utc::now();
        
        // Collect pool statistics
        let pool_stats = HashMap::from([
            ("wallet_info".to_string(), self.collect_pool_stats(&self.wallet_info_pool)),
            ("empty_account".to_string(), self.collect_pool_stats(&self.empty_account_pool)),
            ("scan_result".to_string(), self.collect_pool_stats(&self.scan_result_pool)),
            ("batch_scan_result".to_string(), self.collect_pool_stats(&self.batch_scan_result_pool)),
            ("recovery_transaction".to_string(), self.collect_pool_stats(&self.recovery_transaction_pool)),
            ("string".to_string(), self.collect_pool_stats(&self.string_pool)),
            ("vec_string".to_string(), self.collect_pool_stats(&self.vec_string_pool)),
            ("vec_u8".to_string(), self.collect_pool_stats(&self.vec_u8_pool)),
        ]);
        
        // Update memory pressure (simplified calculation)
        let total_allocated: usize = pool_stats.values()
            .map(|p| p.current_size * 64) // Rough estimate per object
            .sum();
        
        let peak_allocated = {
            let stats = self.stats.write();
            stats.peak_allocated_bytes.max(total_allocated)
        };
        
        // Calculate memory pressure (0-100%)
        let max_memory = 1024 * 1024 * 1024; // 1GB default
        let memory_pressure = (total_allocated as f64 / max_memory as f64) * 100.0;
        
        // Update leak detection stats
        let leak_detection_stats = if self.config.monitoring_config.enable_leak_detection {
            Some(self.memory_monitor.get_leak_stats().await)
        } else {
            None
        };
        
        // Now update stats without holding lock across await
        {
            let mut stats = self.stats.write();
            stats.timestamp = timestamp;
            stats.pool_stats = pool_stats;
            stats.total_allocated_bytes = total_allocated;
            stats.peak_allocated_bytes = peak_allocated;
            stats.memory_pressure = memory_pressure;
            if let Some(leak_stats) = leak_detection_stats {
                stats.leak_detection_stats = leak_stats;
            }
        }
    }
    
    fn collect_pool_stats<T>(&self, pool: &MemoryPool<T>) -> PoolStats 
    where 
        T: Default + Clone,
    {
        let pool_stats = pool.get_stats();
        let hit_rate = if pool_stats.hits + pool_stats.misses > 0 {
            pool_stats.hits as f64 / (pool_stats.hits + pool_stats.misses) as f64 * 100.0
        } else {
            0.0
        };
        
        PoolStats {
            name: "unknown".to_string(), // Will be overridden by caller
            current_size: pool_stats.current_size,
            max_size: 1000, // Default, will be set by caller if needed
            hits: pool_stats.hits,
            misses: pool_stats.misses,
            hit_rate,
            allocations: pool_stats.allocations,
            deallocations: pool_stats.deallocations,
        }
    }
    
    pub async fn optimize_memory(&self) {
        info!("Starting memory optimization");
        
        let start_time = Instant::now();
        
        // Trigger garbage collection
        self.base_manager.garbage_collect();
        
        // Optimize pools
        self.wallet_info_pool.shrink_to_fit();
        self.empty_account_pool.shrink_to_fit();
        self.scan_result_pool.shrink_to_fit();
        self.batch_scan_result_pool.shrink_to_fit();
        self.recovery_transaction_pool.shrink_to_fit();
        self.string_pool.shrink_to_fit();
        self.vec_string_pool.shrink_to_fit();
        self.vec_u8_pool.shrink_to_fit();
        
        // Update GC stats
        {
            let mut stats = self.stats.write();
            stats.gc_stats.total_collections += 1;
            let duration = start_time.elapsed();
            stats.gc_stats.total_time_ms += duration.as_millis() as u64;
            stats.gc_stats.average_time_ms = stats.gc_stats.total_time_ms as f64 / stats.gc_stats.total_collections as f64;
            stats.gc_stats.last_collection_time = Some(Utc::now());
        }
        
        info!("Memory optimization completed in {}ms", start_time.elapsed().as_millis());
    }
    
    pub fn get_memory_stats(&self) -> EnhancedMemoryStats {
        self.stats.read().clone()
    }
    
    pub fn get_config(&self) -> MemoryManagerConfig {
        self.config.clone()
    }
    
    pub async fn get_comprehensive_report(&self) -> serde_json::Value {
        let stats = self.get_memory_stats();
        
        serde_json::json!({
            "timestamp": stats.timestamp,
            "memory_stats": {
                "total_allocated_bytes": stats.total_allocated_bytes,
                "peak_allocated_bytes": stats.peak_allocated_bytes,
                "memory_pressure": stats.memory_pressure,
                "fragmentation_ratio": stats.fragmentation_ratio,
            },
            "pool_stats": stats.pool_stats,
            "gc_stats": stats.gc_stats,
            "leak_detection": stats.leak_detection_stats,
            "config": self.config,
            "recommendations": self.generate_recommendations(&stats)
        })
    }
    
    fn generate_recommendations(&self, stats: &EnhancedMemoryStats) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if stats.memory_pressure > 80.0 {
            recommendations.push("High memory pressure detected. Consider increasing pool sizes or optimizing object usage.".to_string());
        }
        
        for (name, pool_stats) in &stats.pool_stats {
            if pool_stats.hit_rate < 50.0 {
                recommendations.push(format!("Low hit rate for {} pool: {:.1}%. Consider increasing pool size.", name, pool_stats.hit_rate));
            }
        }
        
        if stats.leak_detection_stats.potential_leaks > 0 {
            recommendations.push(format!("Potential memory leaks detected: {} allocations. Review object lifecycle.", stats.leak_detection_stats.potential_leaks));
        }
        
        if stats.gc_stats.average_time_ms > 100.0 {
            recommendations.push("High GC pause times detected. Consider reducing allocation rate or increasing pool sizes.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("Memory usage appears optimal. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

impl Default for EnhancedMemoryStats {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            total_allocated_bytes: 0,
            peak_allocated_bytes: 0,
            pool_stats: HashMap::new(),
            gc_stats: GcStats::default(),
            memory_pressure: 0.0,
            fragmentation_ratio: 0.0,
            leak_detection_stats: LeakDetectionStats::default(),
        }
    }
}

impl Default for GcStats {
    fn default() -> Self {
        Self {
            total_collections: 0,
            total_time_ms: 0,
            average_time_ms: 0.0,
            last_collection_time: None,
            memory_freed_bytes: 0,
        }
    }
}

impl Default for LeakDetectionStats {
    fn default() -> Self {
        Self {
            active_allocations: 0,
            potential_leaks: 0,
            total_leaked_bytes: 0,
            oldest_leak_age_seconds: None,
        }
    }
}

impl MemoryMonitor {
    fn new(config: MonitoringConfig) -> Self {
        Self {
            stats: Arc::new(RwLock::new(EnhancedMemoryStats::default())),
            config: config.clone(),
            leak_detector: Arc::new(LeakDetector::new(
                config.enable_leak_detection,
                config.leak_detection_threshold_seconds,
            )),
        }
    }
    
    async fn get_leak_stats(&self) -> LeakDetectionStats {
        if self.config.enable_leak_detection {
            self.leak_detector.get_stats().await
        } else {
            LeakDetectionStats::default()
        }
    }
}

impl GcScheduler {
    fn new(config: GcConfig) -> Self {
        Self {
            config,
            last_gc: Arc::new(Mutex::new(Instant::now())),
            gc_semaphore: Arc::new(Semaphore::new(1)), // Only one GC at a time
        }
    }
    
    async fn trigger_gc_if_needed(&self, manager: &EnhancedMemoryManager) {
        let should_gc = {
            let last_gc = self.last_gc.lock();
            last_gc.elapsed() > Duration::from_secs(self.config.interval_seconds)
        };
        
        if should_gc {
            let _permit = self.gc_semaphore.acquire().await;
            manager.optimize_memory().await;
            *self.last_gc.lock() = Instant::now();
        }
    }
}

impl LeakDetector {
    fn new(enabled: bool, threshold_seconds: u64) -> Self {
        Self {
            allocations: Arc::new(RwLock::new(HashMap::new())),
            enabled,
            threshold_seconds,
        }
    }
    
    async fn get_stats(&self) -> LeakDetectionStats {
        if !self.enabled {
            return LeakDetectionStats::default();
        }
        
        let allocations = self.allocations.read();
        let now = Instant::now();
        let mut potential_leaks = 0;
        let mut total_leaked_bytes = 0;
        let mut oldest_leak_age = None;
        
        for info in allocations.values() {
            let age_seconds = now.duration_since(info.timestamp).as_secs();
            if age_seconds > self.threshold_seconds {
                potential_leaks += 1;
                total_leaked_bytes += info.size;
                oldest_leak_age = oldest_leak_age.map(|age: u64| age.max(age_seconds)).or(Some(age_seconds));
            }
        }
        
        LeakDetectionStats {
            active_allocations: allocations.len(),
            potential_leaks,
            total_leaked_bytes,
            oldest_leak_age_seconds: oldest_leak_age,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_enhanced_memory_manager_creation() {
        let manager = EnhancedMemoryManager::new();
        let stats = manager.get_memory_stats();
        
        assert_eq!(stats.pool_stats.len(), 8); // Should have 8 pools
        assert!(stats.memory_pressure >= 0.0);
    }
    
    #[tokio::test]
    async fn test_object_pools() {
        let manager = EnhancedMemoryManager::new();
        
        // Test wallet info pool
        let wallet_info = manager.acquire_wallet_info();
        assert!(wallet_info.address.is_empty());
        drop(wallet_info); // Should return to pool
        
        // Test string pool
        let string = manager.acquire_string();
        assert!(string.is_empty());
        drop(string);
        
        let stats = manager.get_memory_stats();
        let wallet_pool_stats = stats.pool_stats.get("wallet_info").unwrap();
        assert!(wallet_pool_stats.allocations > 0);
    }
    
    #[tokio::test]
    async fn test_memory_optimization() {
        let manager = EnhancedMemoryManager::new();
        
        // Acquire some objects
        let _wallet1 = manager.acquire_wallet_info();
        let _wallet2 = manager.acquire_wallet_info();
        let _string1 = manager.acquire_string();
        
        // Trigger optimization
        manager.optimize_memory().await;
        
        let stats = manager.get_memory_stats();
        assert!(stats.gc_stats.total_collections > 0);
    }
    
    #[tokio::test]
    async fn test_comprehensive_report() {
        let manager = EnhancedMemoryManager::new();
        
        let report = manager.get_comprehensive_report().await;
        
        assert!(report.get("timestamp").is_some());
        assert!(report.get("memory_stats").is_some());
        assert!(report.get("pool_stats").is_some());
        assert!(report.get("gc_stats").is_some());
        assert!(report.get("recommendations").is_some());
    }
}
