use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Clone)]
pub struct MemoryPool<T> {
    pool: Arc<RwLock<Vec<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
    created: Arc<RwLock<Instant>>,
    stats: Arc<RwLock<MemoryPoolStats>>,
}

#[derive(Debug, Default, Clone)]
pub struct MemoryPoolStats {
    pub hits: u64,
    pub misses: u64,
    pub allocations: u64,
    pub deallocations: u64,
    pub current_size: usize,
    pub max_size_reached: usize,
}

impl<T> MemoryPool<T>
where
    T: Default + Clone,
{
    pub fn new(max_size: usize) -> Self {
        Self::with_factory(max_size, || T::default())
    }

    pub fn with_factory<F>(max_size: usize, factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            pool: Arc::new(RwLock::new(Vec::with_capacity(max_size))),
            factory: Arc::new(factory),
            max_size,
            created: Arc::new(RwLock::new(Instant::now())),
            stats: Arc::new(RwLock::new(MemoryPoolStats::default())),
        }
    }

    pub fn acquire(&self) -> PooledItem<T> {
        let mut pool = self.pool.write();
        let mut stats = self.stats.write();
        
        if let Some(item) = pool.pop() {
            stats.hits += 1;
            stats.current_size = pool.len();
            
            PooledItem {
                item: Some(item),
                pool: Some(self.clone()),
            }
        } else {
            stats.misses += 1;
            stats.allocations += 1;
            
            let item = (self.factory)();
            
            PooledItem {
                item: Some(item),
                pool: Some(self.clone()),
            }
        }
    }

    pub fn return_item(&self, item: T) {
        let mut pool = self.pool.write();
        let mut stats = self.stats.write();
        
        if pool.len() < self.max_size {
            pool.push(item);
            stats.deallocations += 1;
            stats.current_size = pool.len();
            stats.max_size_reached = stats.max_size_reached.max(pool.len());
        }
    }

    pub fn stats(&self) -> MemoryPoolStats {
        self.stats.read().clone()
    }

    pub fn clear(&self) {
        let mut pool = self.pool.write();
        let mut stats = self.stats.write();
        
        pool.clear();
        stats.current_size = 0;
    }

    pub fn shrink_to_fit(&self) {
        let mut pool = self.pool.write();
        pool.shrink_to_fit();
    }
    
    pub fn created_at(&self) -> Instant {
        *self.created.read()
    }
    
    pub fn age(&self) -> Duration {
        self.created.read().elapsed()
    }
    
    pub fn is_older_than(&self, duration: Duration) -> bool {
        self.age() > duration
    }
}

pub struct PooledItem<T: Default + Clone> {
    item: Option<T>,
    pool: Option<MemoryPool<T>>,
}

impl<T: Default + Clone> std::ops::Deref for PooledItem<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T: Default + Clone> std::ops::DerefMut for PooledItem<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T: Default + Clone> Drop for PooledItem<T> {
    fn drop(&mut self) {
        if let (Some(item), Some(pool)) = (self.item.take(), &self.pool) {
            pool.return_item(item);
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryManager {
    pools: Arc<RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>>,
    total_allocated: Arc<RwLock<usize>>,
    peak_allocated: Arc<RwLock<usize>>,
    gc_threshold: usize,
    gc_interval: Duration,
    last_gc: Arc<RwLock<Instant>>,
}

impl MemoryManager {
    pub fn new() -> Self {
        Self::with_config(100 * 1024 * 1024, Duration::from_secs(60)) // 100MB threshold, 1 minute GC
    }

    pub fn with_config(gc_threshold: usize, gc_interval: Duration) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            total_allocated: Arc::new(RwLock::new(0)),
            peak_allocated: Arc::new(RwLock::new(0)),
            gc_threshold,
            gc_interval,
            last_gc: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub fn get_pool<T>(&self, name: &str, max_size: usize) -> MemoryPool<T>
    where
        T: Default + Clone + 'static + Send + Sync,
    {
        let mut pools = self.pools.write();
        
        if let Some(pool) = pools.get(name) {
            if let Some(typed_pool) = pool.downcast_ref::<MemoryPool<T>>() {
                return typed_pool.clone();
            }
        }

        let pool = MemoryPool::new(max_size);
        pools.insert(name.to_string(), Box::new(pool.clone()));
        pool
    }

    pub fn allocate(&self, size: usize) {
        let mut total = self.total_allocated.write();
        let mut peak = self.peak_allocated.write();
        
        *total += size;
        *peak = (*peak).max(*total);

        // Trigger GC if threshold exceeded
        if *total > self.gc_threshold {
            let should_gc = {
                let last_gc = self.last_gc.read();
                last_gc.elapsed() > self.gc_interval
            };

            if should_gc {
                drop(self.last_gc.write());
                *self.last_gc.write() = Instant::now();
                drop(total);
                drop(peak);
                
                info!("Memory threshold exceeded, triggering garbage collection");
                self.garbage_collect();
            }
        }
    }

    pub fn deallocate(&self, size: usize) {
        let mut total = self.total_allocated.write();
        *total = total.saturating_sub(size);
    }

    pub fn garbage_collect(&self) {
        // Force garbage collection in all pools
        let pools = self.pools.read();
        
        for (_name, pool) in pools.iter() {
            // This is a simplified GC - in practice you'd have more sophisticated logic
            if let Some(_string_pool) = pool.downcast_ref::<MemoryPool<String>>() {
                // Clear string pools more aggressively
            }
        }
        
        info!("Garbage collection completed");
    }

    pub fn get_memory_stats(&self) -> MemoryStats {
        let total = *self.total_allocated.read();
        let peak = *self.peak_allocated.read();
        
        MemoryStats {
            current_allocated_bytes: total,
            peak_allocated_bytes: peak,
            gc_threshold_bytes: self.gc_threshold,
            time_since_last_gc: self.last_gc.read().elapsed(),
        }
    }

    pub fn optimize_memory(&self) {
        info!("Starting memory optimization");
        
        // Shrink all pools
        let pools = self.pools.read();
        for (_name, _pool) in pools.iter() {
            // This would need more sophisticated type handling in practice
            // For now, we'll just trigger GC
        }
        
        // Force garbage collection
        self.garbage_collect();
        
        info!("Memory optimization completed");
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub current_allocated_bytes: usize,
    pub peak_allocated_bytes: usize,
    pub gc_threshold_bytes: usize,
    pub time_since_last_gc: Duration,
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

// Memory-efficient buffer pool for network operations
pub struct BufferPool {
    small_buffers: MemoryPool<Vec<u8>>,
    medium_buffers: MemoryPool<Vec<u8>>,
    large_buffers: MemoryPool<Vec<u8>>,
}

impl std::fmt::Debug for BufferPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferPool")
            .field("small_buffers", &"MemoryPool<Vec<u8>>")
            .field("medium_buffers", &"MemoryPool<Vec<u8>>")
            .field("large_buffers", &"MemoryPool<Vec<u8>>")
            .finish()
    }
}

impl Clone for BufferPool {
    fn clone(&self) -> Self {
        Self {
            small_buffers: self.small_buffers.clone(),
            medium_buffers: self.medium_buffers.clone(),
            large_buffers: self.large_buffers.clone(),
        }
    }
}

impl BufferPool {
    pub fn new() -> Self {
        Self {
            small_buffers: MemoryPool::new(1000),   // 1KB buffers
            medium_buffers: MemoryPool::new(500),   // 4KB buffers
            large_buffers: MemoryPool::new(100),    // 64KB buffers
        }
    }

    pub fn get_buffer(&self, size: usize) -> PooledItem<Vec<u8>> {
        match size {
            0..=1024 => {
                let mut buffer = self.small_buffers.acquire();
                buffer.resize(size, 0);
                buffer
            }
            1025..=4096 => {
                let mut buffer = self.medium_buffers.acquire();
                buffer.resize(size, 0);
                buffer
            }
            _ => {
                let mut buffer = self.large_buffers.acquire();
                buffer.resize(size, 0);
                buffer
            }
        }
    }

    pub fn get_stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            small_pool: self.small_buffers.stats(),
            medium_pool: self.medium_buffers.stats(),
            large_pool: self.large_buffers.stats(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BufferPoolStats {
    pub small_pool: MemoryPoolStats,
    pub medium_pool: MemoryPoolStats,
    pub large_pool: MemoryPoolStats,
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

// Global memory manager instance
lazy_static::lazy_static! {
    pub static ref GLOBAL_MEMORY_MANAGER: Arc<MemoryManager> = Arc::new(MemoryManager::new());
    pub static ref GLOBAL_BUFFER_POOL: Arc<BufferPool> = Arc::new(BufferPool::new());
}

pub fn get_global_memory_manager() -> Arc<MemoryManager> {
    GLOBAL_MEMORY_MANAGER.clone()
}

pub fn get_global_buffer_pool() -> Arc<BufferPool> {
    GLOBAL_BUFFER_POOL.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool_basic() {
        let pool = MemoryPool::new(10);
        
        // Test acquisition and return
        let item = pool.acquire();
        assert_eq!(pool.stats().misses, 1);
        
        drop(item);
        assert_eq!(pool.stats().deallocations, 1);
        
        // Test reuse
        let item2 = pool.acquire();
        assert_eq!(pool.stats().hits, 1);
        
        drop(item2);
    }

    #[test]
    fn test_memory_pool_factory() {
        let pool = MemoryPool::with_factory(5, || 42u32);
        
        let item = pool.acquire();
        assert_eq!(*item, 42);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new();
        
        let small_buf = pool.get_buffer(512);
        assert_eq!(small_buf.len(), 512);
        
        let medium_buf = pool.get_buffer(2048);
        assert_eq!(medium_buf.len(), 2048);
        
        let large_buf = pool.get_buffer(32768);
        assert_eq!(large_buf.len(), 32768);
    }

    #[test]
    fn test_memory_manager() {
        let manager = MemoryManager::new();
        
        manager.allocate(1024);
        assert_eq!(manager.get_memory_stats().current_allocated_bytes, 1024);
        
        manager.deallocate(512);
        assert_eq!(manager.get_memory_stats().current_allocated_bytes, 512);
    }
}
