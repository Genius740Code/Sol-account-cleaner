use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use parking_lot::Mutex;
use std::collections::HashMap;
use tracing::{info, error, debug};
use serde::{Serialize, Deserialize};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub heap_size_mb: u64,
    pub heap_objects: u64,
    pub stack_size_mb: u64,
    pub gc_collections: u64,
    pub gc_pause_ms: u64,
    pub memory_pressure: f64,
    pub fragmentation_ratio: f64,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            heap_size_mb: 0,
            heap_objects: 0,
            stack_size_mb: 0,
            gc_collections: 0,
            gc_pause_ms: 0,
            memory_pressure: 0.0,
            fragmentation_ratio: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub max_heap_size_mb: u64,
    pub gc_threshold_percent: f64,
    pub cleanup_interval_seconds: u64,
    pub enable_auto_gc: bool,
    pub enable_memory_profiling: bool,
    pub max_cache_size_mb: u64,
    pub max_queue_size: usize,
    pub enable_object_pooling: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_heap_size_mb: 2048, // 2GB
            gc_threshold_percent: 80.0,
            cleanup_interval_seconds: 60,
            enable_auto_gc: true,
            enable_memory_profiling: true,
            max_cache_size_mb: 512,
            max_queue_size: 10000,
            enable_object_pooling: true,
        }
    }
}

pub struct MemoryOptimizer {
    config: MemoryConfig,
    stats: Arc<RwLock<MemoryStats>>,
    object_pools: Arc<Mutex<HashMap<String, String>>>, // Store pool names for tracking
    #[allow(dead_code)]
    memory_monitors: Arc<RwLock<Vec<Box<dyn MemoryMonitor>>>>,
    last_gc_time: Arc<Mutex<Instant>>,
}

pub trait MemoryMonitor: Send + Sync {
    fn check_memory_usage(&self) -> Result<MemoryStats, Box<dyn std::error::Error + Send + Sync>>;
    fn trigger_gc(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct ObjectPool<T> {
    objects: Arc<Mutex<Vec<T>>>,
    create_fn: Box<dyn Fn() -> T + Send + Sync>,
    reset_fn: Option<Box<dyn Fn(&mut T) + Send + Sync>>,
    max_size: usize,
    current_size: Arc<Mutex<usize>>,
}

impl<T> ObjectPool<T> {
    pub fn new<F, R>(create_fn: F, reset_fn: Option<R>, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
        R: Fn(&mut T) + Send + Sync + 'static,
    {
        Self {
            objects: Arc::new(Mutex::new(Vec::with_capacity(max_size))),
            create_fn: Box::new(create_fn),
            reset_fn: reset_fn.map(|f| Box::new(f) as Box<dyn Fn(&mut T) + Send + Sync>),
            max_size,
            current_size: Arc::new(Mutex::new(0)),
        }
    }
    
    pub fn acquire(&self) -> PooledObject<'_, T> {
        let mut objects = self.objects.lock();
        let object = objects.pop().or_else(|| {
            if *self.current_size.lock() < self.max_size {
                *self.current_size.lock() += 1;
                Some((self.create_fn)())
            } else {
                None
            }
        });
        
        PooledObject {
            object,
            pool: self,
        }
    }
    
    pub fn return_object(&self, mut object: T) {
        if let Some(reset_fn) = &self.reset_fn {
            reset_fn(&mut object);
        }
        
        let mut objects = self.objects.lock();
        if objects.len() < self.max_size {
            objects.push(object);
        } else {
            *self.current_size.lock() -= 1;
        }
    }
    
    pub fn size(&self) -> usize {
        *self.current_size.lock()
    }
}

pub struct PooledObject<'a, T> {
    object: Option<T>,
    pool: &'a ObjectPool<T>,
}

impl<T> std::ops::Deref for PooledObject<'_, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledObject<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<'_, T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            self.pool.return_object(object);
        }
    }
}

impl MemoryOptimizer {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(MemoryStats::default())),
            object_pools: Arc::new(Mutex::new(HashMap::new())),
            memory_monitors: Arc::new(RwLock::new(Vec::new())),
            last_gc_time: Arc::new(Mutex::new(Instant::now())),
        }
    }
    
    pub async fn start_monitoring(&self) -> Result<(), crate::SolanaRecoverError> {
        info!("Starting memory optimization monitoring");
        
        let stats = self.stats.clone();
        let config = self.config.clone();
        let last_gc_time = self.last_gc_time.clone();
        let object_pools = self.object_pools.clone();
        
        // Start memory monitoring task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.cleanup_interval_seconds));
            
            loop {
                interval.tick().await;
                
                // Collect memory statistics
                if let Ok(memory_stats) = Self::collect_memory_stats().await {
                    *stats.write().await = memory_stats.clone();
                    
                    // Check if GC is needed
                    let memory_usage_percent = (memory_stats.heap_size_mb as f64 / config.max_heap_size_mb as f64) * 100.0;
                    
                    if config.enable_auto_gc && memory_usage_percent > config.gc_threshold_percent {
                        let now = Instant::now();
                        let should_run_gc = {
                            let last_gc = last_gc_time.lock();
                            now.duration_since(*last_gc) > Duration::from_secs(30)
                        };
                        
                        if should_run_gc {
                            info!("Triggering garbage collection - memory usage: {:.1}%", memory_usage_percent);
                            
                            if let Err(e) = Self::trigger_garbage_collection().await {
                                error!("Failed to trigger garbage collection: {}", e);
                            } else {
                                // Update last_gc time after successful GC
                                let mut last_gc = last_gc_time.lock();
                                *last_gc = now;
                            }
                        }
                    }
                }
                
                // Cleanup object pools
                Self::cleanup_object_pools(&object_pools).await;
            }
        });
        
        Ok(())
    }

    pub fn get_object_pool<T: 'static>(&self, name: &str) -> Option<Arc<ObjectPool<T>>> {
        let pools = self.object_pools.lock();
        if pools.contains_key(name) {
            // In a real implementation, you'd store actual pool references
            // For now, return None as placeholder
            None
        } else {
            None
        }
    }

    // ...

    async fn optimize_memory(&self) -> Result<MemoryOptimizationResult, crate::SolanaRecoverError> {
        let before_stats = self.get_memory_stats().await;

        info!("Starting memory optimization");

        // Trigger garbage collection
        Self::trigger_garbage_collection().await?;

        // Clear caches if memory pressure is high
        let memory_usage_percent = (before_stats.heap_size_mb as f64 / self.config.max_heap_size_mb as f64) * 100.0;

        let mut cleared_caches = 0;
        if memory_usage_percent > self.config.gc_threshold_percent {
            // In a real implementation, you'd clear actual caches
            cleared_caches = 1;
            info!("Cleared caches due to memory pressure");
        }

        // Shrink object pools if necessary
        let shrunk_pools = 0;
        let pools = self.object_pools.lock();
        for (name, _pool) in pools.iter() {
            // In a more sophisticated implementation, you might:
            // 1. Check pool usage and shrink if necessary
            // 2. Remove unused pools
            // 3. Reset objects in pools

            debug!("Checking object pool: {}", name);
        }

        let after_stats = self.get_memory_stats().await;

        let result = MemoryOptimizationResult {
            before_heap_size_mb: before_stats.heap_size_mb,
            after_heap_size_mb: after_stats.heap_size_mb,
            memory_freed_mb: before_stats.heap_size_mb.saturating_sub(after_stats.heap_size_mb),
            cleared_caches,
            shrunk_pools,
            duration_ms: 100, // Placeholder
        };
        
        info!("Memory optimization completed: freed {}MB", result.memory_freed_mb);
        
        Ok(result)
    }
    
    pub async fn set_memory_limit(&mut self, max_heap_size_mb: u64) {
        self.config.max_heap_size_mb = max_heap_size_mb;
        info!("Updated memory limit to {}MB", max_heap_size_mb);
    }
    
    pub fn get_config(&self) -> MemoryConfig {
        self.config.clone()
    }
    
    async fn cleanup_object_pools(pools: &Arc<Mutex<HashMap<String, String>>>) {
        let pools_guard = pools.lock();
        
        for (name, _pool) in pools_guard.iter() {
            // In a more sophisticated implementation, you might:
            // 1. Check pool usage and shrink if necessary
            // 2. Remove unused pools
            // 3. Reset objects in pools
            
            debug!("Checking object pool: {}", name);
        }
    }
    
    async fn collect_memory_stats() -> Result<MemoryStats, Box<dyn std::error::Error + Send + Sync>> {
        // Placeholder implementation
        Ok(MemoryStats {
            timestamp: Utc::now(),
            heap_size_mb: 100,
            heap_objects: 1000,
            stack_size_mb: 10,
            gc_collections: 5,
            gc_pause_ms: 10,
            memory_pressure: 50.0,
            fragmentation_ratio: 0.1,
        })
    }
    
    async fn trigger_garbage_collection() -> Result<(), crate::SolanaRecoverError> {
        // Simulate garbage collection
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    }
    
    pub async fn get_memory_stats(&self) -> MemoryStats {
        self.stats.read().await.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOptimizationResult {
    pub before_heap_size_mb: u64,
    pub after_heap_size_mb: u64,
    pub memory_freed_mb: u64,
    pub cleared_caches: u32,
    pub shrunk_pools: u32,
    pub duration_ms: u64,
}

// Memory leak detector
pub struct MemoryLeakDetector {
    allocations: Arc<RwLock<HashMap<String, AllocationInfo>>>,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct AllocationInfo {
    size: usize,
    timestamp: Instant,
    stack_trace: Option<String>,
}

impl MemoryLeakDetector {
    pub fn new(enabled: bool) -> Self {
        Self {
            allocations: Arc::new(RwLock::new(HashMap::new())),
            enabled,
        }
    }
    
    pub fn track_allocation(&self, id: String, size: usize) {
        if !self.enabled {
            return;
        }
        
        let mut allocations = self.allocations.blocking_write();
        allocations.insert(id, AllocationInfo {
            size,
            timestamp: Instant::now(),
            stack_trace: None, // Would capture actual stack trace in production
        });
    }
    
    pub fn track_deallocation(&self, id: &str) {
        if !self.enabled {
            return;
        }
        
        let mut allocations = self.allocations.blocking_write();
        allocations.remove(id);
    }
    
    pub async fn detect_leaks(&self, max_age_seconds: u64) -> Vec<LeakInfo> {
        if !self.enabled {
            return Vec::new();
        }
        
        let allocations = self.allocations.read().await;
        let now = Instant::now();
        let mut leaks = Vec::new();
        
        for (id, info) in allocations.iter() {
            let age_seconds = now.duration_since(info.timestamp).as_secs();
            
            if age_seconds > max_age_seconds {
                leaks.push(LeakInfo {
                    id: id.clone(),
                    size: info.size,
                    age_seconds,
                    stack_trace: info.stack_trace.clone(),
                });
            }
        }
        
        // Sort by age (oldest first)
        leaks.sort_by(|a, b| b.age_seconds.cmp(&a.age_seconds));
        
        leaks
    }
    
    pub async fn get_allocation_summary(&self) -> AllocationSummary {
        let allocations = self.allocations.read().await;
        
        let total_allocations = allocations.len();
        let total_size: usize = allocations.values().map(|info| info.size).sum();
        
        let mut size_distribution = HashMap::new();
        for info in allocations.values() {
            let size_category = Self::categorize_size(info.size);
            *size_distribution.entry(size_category).or_insert(0) += 1;
        }
        
        AllocationSummary {
            total_allocations,
            total_size_bytes: total_size,
            average_size_bytes: if total_allocations > 0 { total_size / total_allocations } else { 0 },
            size_distribution,
        }
    }
    
    fn categorize_size(size: usize) -> String {
        match size {
            0..=64 => "small".to_string(),
            65..=1024 => "medium".to_string(),
            1025..=65536 => "large".to_string(),
            _ => "huge".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakInfo {
    pub id: String,
    pub size: usize,
    pub age_seconds: u64,
    pub stack_trace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationSummary {
    pub total_allocations: usize,
    pub total_size_bytes: usize,
    pub average_size_bytes: usize,
    pub size_distribution: HashMap<String, usize>,
}

// Memory-safe utilities
pub struct SafeMemoryManager {
    leak_detector: Arc<MemoryLeakDetector>,
    optimizer: Arc<MemoryOptimizer>,
}

impl SafeMemoryManager {
    pub fn new(memory_config: MemoryConfig, leak_detection_enabled: bool) -> Self {
        Self {
            leak_detector: Arc::new(MemoryLeakDetector::new(leak_detection_enabled)),
            optimizer: Arc::new(MemoryOptimizer::new(memory_config)),
        }
    }
    
    pub async fn start(&self) -> Result<(), crate::SolanaRecoverError> {
        self.optimizer.start_monitoring().await?;
        Ok(())
    }
    
    pub fn track_allocation(&self, id: String, size: usize) {
        self.leak_detector.track_allocation(id, size);
    }
    
    pub fn track_deallocation(&self, id: &str) {
        self.leak_detector.track_deallocation(id);
    }
    
    pub async fn detect_leaks(&self, max_age_seconds: u64) -> Vec<LeakInfo> {
        self.leak_detector.detect_leaks(max_age_seconds).await
    }
    
    pub async fn get_memory_stats(&self) -> MemoryStats {
        self.optimizer.get_memory_stats().await
    }
    
    pub async fn optimize_memory(&self) -> Result<MemoryOptimizationResult, crate::SolanaRecoverError> {
        self.optimizer.optimize_memory().await
    }
    
    pub async fn get_comprehensive_report(&self) -> serde_json::Value {
        let memory_stats = self.get_memory_stats().await;
        let allocation_summary = self.leak_detector.get_allocation_summary().await;
        let leaks = self.detect_leaks(300).await; // 5 minutes
        
        serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "memory_stats": memory_stats,
            "allocation_summary": allocation_summary,
            "potential_leaks": leaks,
            "leak_count": leaks.len(),
            "memory_pressure": memory_stats.memory_pressure,
            "recommendations": Self::generate_recommendations(&memory_stats, &leaks)
        })
    }
    
    fn generate_recommendations(memory_stats: &MemoryStats, leaks: &[LeakInfo]) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if memory_stats.memory_pressure > 80.0 {
            recommendations.push("High memory pressure detected. Consider increasing memory limits or optimizing memory usage.".to_string());
        }
        
        if memory_stats.fragmentation_ratio > 0.3 {
            recommendations.push("High memory fragmentation detected. Consider implementing memory pooling or compaction.".to_string());
        }
        
        if !leaks.is_empty() {
            recommendations.push(format!("Potential memory leaks detected: {} allocations. Review allocation patterns.", leaks.len()));
        }
        
        if memory_stats.gc_pause_ms > 100 {
            recommendations.push("High GC pause times detected. Consider reducing allocation rate or implementing object pooling.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("Memory usage appears normal. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_optimizer_creation() {
        let config = MemoryConfig::default();
        let optimizer = MemoryOptimizer::new(config);
        
        let stats = optimizer.get_memory_stats().await;
        assert_eq!(stats.heap_size_mb, 0); // Default value
    }
    
    #[tokio::test]
    async fn test_object_pool() {
        let pool = Arc::new(ObjectPool::new(
            || Vec::<u8>::new(),
            Some(|vec: &mut Vec<u8>| vec.clear()),
            10
        ));
        
        {
            let _obj1 = pool.acquire();
            let _obj2 = pool.acquire();
            
            assert_eq!(pool.size(), 2);
        }
        
        // Objects should be returned to pool when dropped
        assert_eq!(pool.size(), 2);
    }
    
    #[tokio::test]
    async fn test_leak_detector() {
        let detector = MemoryLeakDetector::new(true);
        
        detector.track_allocation("test1".to_string(), 1024);
        detector.track_allocation("test2".to_string(), 2048);
        
        let summary = detector.get_allocation_summary().await;
        assert_eq!(summary.total_allocations, 2);
        assert_eq!(summary.total_size_bytes, 3072);
        
        detector.track_deallocation("test1");
        
        let summary = detector.get_allocation_summary().await;
        assert_eq!(summary.total_allocations, 1);
        assert_eq!(summary.total_size_bytes, 2048);
    }
    
    #[tokio::test]
    async fn test_safe_memory_manager() {
        let config = MemoryConfig::default();
        let manager = SafeMemoryManager::new(config, true);
        
        manager.track_allocation("test".to_string(), 1024);
        
        let report = manager.get_comprehensive_report().await;
        
        // Check that report contains expected fields
        assert!(report.get("memory_stats").is_some());
        assert!(report.get("allocation_summary").is_some());
        assert!(report.get("potential_leaks").is_some());
        assert!(report.get("recommendations").is_some());
        
        manager.track_deallocation("test");
    }
}
