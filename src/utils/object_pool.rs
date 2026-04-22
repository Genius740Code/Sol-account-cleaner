use crate::core::{Result, SolanaRecoverError};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, RwLock};
use serde::{Serialize, Deserialize};
use std::any::Any;
use std::marker::PhantomData;

/// Generic object pool for efficient memory management
pub struct ObjectPool<T> {
    objects: Arc<tokio::sync::Mutex<VecDeque<T>>>,
    factory: Box<dyn Fn() -> T + Send + Sync>,
    reset_fn: Option<Box<dyn Fn(&mut T) + Send + Sync>>,
    max_size: usize,
    current_size: Arc<std::sync::atomic::AtomicUsize>,
    metrics: Arc<RwLock<PoolMetrics>>,
    config: PoolConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct PoolMetrics {
    pub total_created: u64,
    pub total_reused: u64,
    pub total_discarded: u64,
    pub current_size: usize,
    pub peak_size: usize,
    pub hit_rate: f64,
    pub avg_lifetime_ms: f64,
    pub last_cleanup: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            total_created: 0,
            total_reused: 0,
            total_discarded: 0,
            current_size: 0,
            peak_size: 0,
            hit_rate: 0.0,
            avg_lifetime_ms: 0.0,
            last_cleanup: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_size: usize,
    pub initial_size: usize,
    pub cleanup_interval: Duration,
    pub max_idle_time: Duration,
    pub enable_metrics: bool,
    pub preallocate_initial: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            initial_size: 10,
            cleanup_interval: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(300),
            enable_metrics: true,
            preallocate_initial: true,
        }
    }
}

impl<T> ObjectPool<T>
where
    T: Send + 'static,
{
    pub fn new<F>(factory: F, config: PoolConfig) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let pool = Self {
            objects: Arc::new(tokio::sync::Mutex::new(VecDeque::with_capacity(config.max_size))),
            factory: Box::new(factory),
            reset_fn: None,
            max_size: config.max_size,
            current_size: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            metrics: Arc::new(RwLock::new(PoolMetrics::default())),
            config,
        };

        // Preallocate initial objects if enabled
        if pool.config.preallocate_initial {
            // Note: Skip preallocation for now due to Clone issues
            // In production, you'd implement a proper sharing mechanism
        }

        // Start cleanup task if needed
        if pool.config.cleanup_interval > Duration::ZERO {
            pool.start_cleanup_task();
        }

        pool
    }

    pub fn with_reset<F, R>(factory: F, reset_fn: R, config: PoolConfig) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
        R: Fn(&mut T) + Send + Sync + 'static,
    {
        let mut pool = Self::new(factory, config);
        pool.reset_fn = Some(Box::new(reset_fn));
        pool
    }

    /// Get an object from the pool
    pub async fn get(&self) -> PooledObject<T> {
        let start_time = Instant::now();
        
        // Try to get from pool first
        let mut objects = self.objects.lock().await;
        
        let object = if let Some(mut obj) = objects.pop_front() {
            // Reset object if reset function is provided
            if let Some(ref reset_fn) = self.reset_fn {
                reset_fn(&mut obj);
            }
            
            // Update metrics
            if self.config.enable_metrics {
                let mut metrics = self.metrics.write().await;
                metrics.total_reused += 1;
                self.update_hit_rate(&mut metrics);
            }
            
            obj
        } else {
            // Create new object
            let obj = (self.factory)();
            
            // Update metrics
            if self.config.enable_metrics {
                let mut metrics = self.metrics.write().await;
                metrics.total_created += 1;
                self.update_hit_rate(&mut metrics);
            }
            
            obj
        };
        
        let _current_size = self.current_size.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) - 1;
        
        PooledObject {
            object: Some(object),
            checkout_time: start_time,
        }
    }

    /// Return an object to the pool
    async fn return_object(&self, object: T, checkout_time: Instant) {
        let current_size = self.current_size.load(std::sync::atomic::Ordering::Relaxed);
        
        if current_size < self.max_size {
            let mut objects = self.objects.lock().await;
            objects.push_back(object);
            self.current_size.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            // Update metrics
            if self.config.enable_metrics {
                let mut metrics = self.metrics.write().await;
                let lifetime = checkout_time.elapsed().as_millis() as f64;
                self.update_lifetime_metrics(&mut metrics, lifetime);
            }
        } else {
            // Pool is full, discard the object
            if self.config.enable_metrics {
                let mut metrics = self.metrics.write().await;
                metrics.total_discarded += 1;
            }
        }
    }

    /// Preallocate initial objects
    async fn preallocate_initial_objects(&self) {
        let mut objects = self.objects.lock().await;
        let initial_count = std::cmp::min(self.config.initial_size, self.config.max_size);
        
        for _ in 0..initial_count {
            objects.push_back((self.factory)());
            self.current_size.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.total_created = initial_count as u64;
            metrics.current_size = initial_count;
            metrics.peak_size = initial_count;
        }
    }

    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        // Note: Skip cleanup task for now due to Clone issues
        // In production, you'd implement a proper sharing mechanism
        tracing::warn!("Object pool cleanup task disabled due to Clone limitations");
    }

    /// Perform cleanup of idle objects
    async fn perform_cleanup(&self, _max_idle_time: Duration) -> Result<()> {
        // For generic objects, we can't track idle time without additional metadata
        // This is a placeholder for more sophisticated cleanup logic
        Ok(())
    }

    /// Update hit rate metric
    fn update_hit_rate(&self, metrics: &mut PoolMetrics) {
        let total_requests = metrics.total_created + metrics.total_reused;
        if total_requests > 0 {
            metrics.hit_rate = metrics.total_reused as f64 / total_requests as f64;
        }
    }

    /// Update lifetime metrics
    fn update_lifetime_metrics(&self, metrics: &mut PoolMetrics, lifetime: f64) {
        let total_reused = metrics.total_reused;
        if total_reused > 0 {
            metrics.avg_lifetime_ms = 
                (metrics.avg_lifetime_ms * (total_reused - 1) as f64 + lifetime) / total_reused as f64;
        }
    }

    /// Get current pool metrics
    pub async fn get_metrics(&self) -> PoolMetrics {
        if !self.config.enable_metrics {
            return PoolMetrics::default();
        }
        
        let mut metrics = self.metrics.write().await;
        metrics.current_size = self.current_size.load(std::sync::atomic::Ordering::Relaxed);
        
        if metrics.current_size > metrics.peak_size {
            metrics.peak_size = metrics.current_size;
        }
        
        PoolMetrics {
            total_created: metrics.total_created,
            total_reused: metrics.total_reused,
            total_discarded: metrics.total_discarded,
            current_size: metrics.current_size,
            peak_size: metrics.peak_size,
            hit_rate: metrics.hit_rate,
            avg_lifetime_ms: metrics.avg_lifetime_ms,
            last_cleanup: metrics.last_cleanup,
        }
    }

    /// Clear all objects from the pool
    pub async fn clear(&self) {
        let mut objects = self.objects.lock().await;
        objects.clear();
        self.current_size.store(0, std::sync::atomic::Ordering::Relaxed);
        
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.current_size = 0;
        }
    }
}

// Note: ObjectPool cannot be cloned due to function pointers
// Use Arc<ObjectPool<T>> for sharing instead

/// Pooled object that needs to be manually returned to pool
pub struct PooledObject<T> {
    object: Option<T>,
    checkout_time: Instant,
}

impl<T> PooledObject<T> {
    /// Get a mutable reference to the pooled object
    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().expect("Object already returned to pool")
    }

    /// Get an immutable reference to the pooled object
    pub fn get(&self) -> &T {
        self.object.as_ref().expect("Object already returned to pool")
    }

    /// Consume the pooled object and return the inner value
    /// Note: This will not return the object to the pool
    pub fn into_inner(mut self) -> T {
        self.object.take().expect("Object already returned to pool")
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// Memory manager for coordinating multiple object pools
pub struct MemoryManager {
    transaction_pool: Arc<ObjectPool<TransactionWrapper>>,
    account_pool: Arc<ObjectPool<AccountWrapper>>,
    buffer_pool: Arc<ObjectPool<Vec<u8>>>,
    string_pool: Arc<ObjectPool<String>>,
    metrics: Arc<RwLock<MemoryMetrics>>,
    config: MemoryManagerConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryMetrics {
    pub total_allocated_objects: u64,
    pub total_reused_objects: u64,
    pub total_memory_saved_mb: f64,
    pub pool_efficiency: f64,
    pub gc_pressure: f64,
    pub allocation_rate: f64,
    pub deallocation_rate: f64,
    pub last_gc: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self {
            total_allocated_objects: 0,
            total_reused_objects: 0,
            total_memory_saved_mb: 0.0,
            pool_efficiency: 0.0,
            gc_pressure: 0.0,
            allocation_rate: 0.0,
            deallocation_rate: 0.0,
            last_gc: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryManagerConfig {
    pub enable_monitoring: bool,
    pub gc_threshold: f64,
    pub monitoring_interval: Duration,
    pub auto_gc_enabled: bool,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            enable_monitoring: true,
            gc_threshold: 0.8,
            monitoring_interval: Duration::from_secs(30),
            auto_gc_enabled: true,
        }
    }
}

/// Wrapper for Solana transactions
#[derive(Debug, Clone)]
pub struct TransactionWrapper {
    pub transaction: Option<solana_sdk::transaction::Transaction>,
    pub created_at: Instant,
}

/// Wrapper for account data
#[derive(Debug, Clone)]
pub struct AccountWrapper {
    pub account: Option<solana_sdk::account::Account>,
    pub created_at: Instant,
}

impl MemoryManager {
    pub fn new(config: MemoryManagerConfig) -> Result<Self> {
        // Create object pools with appropriate configurations
        let transaction_config = PoolConfig {
            max_size: 1000,
            initial_size: 50,
            cleanup_interval: Duration::from_secs(120),
            max_idle_time: Duration::from_secs(600),
            enable_metrics: config.enable_monitoring,
            preallocate_initial: true,
        };

        let account_config = PoolConfig {
            max_size: 5000,
            initial_size: 100,
            cleanup_interval: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(300),
            enable_metrics: config.enable_monitoring,
            preallocate_initial: true,
        };

        let buffer_config = PoolConfig {
            max_size: 2000,
            initial_size: 200,
            cleanup_interval: Duration::from_secs(30),
            max_idle_time: Duration::from_secs(180),
            enable_metrics: config.enable_monitoring,
            preallocate_initial: true,
        };

        let string_config = PoolConfig {
            max_size: 3000,
            initial_size: 300,
            cleanup_interval: Duration::from_secs(45),
            max_idle_time: Duration::from_secs(240),
            enable_metrics: config.enable_monitoring,
            preallocate_initial: true,
        };

        let transaction_pool = Arc::new(ObjectPool::with_reset(
            || TransactionWrapper {
                transaction: None,
                created_at: Instant::now(),
            },
            |wrapper| {
                wrapper.transaction = None;
                wrapper.created_at = Instant::now();
            },
            transaction_config,
        ));

        let account_pool = Arc::new(ObjectPool::with_reset(
            || AccountWrapper {
                account: None,
                created_at: Instant::now(),
            },
            |wrapper| {
                wrapper.account = None;
                wrapper.created_at = Instant::now();
            },
            account_config,
        ));

        let buffer_pool = Arc::new(ObjectPool::with_reset(
            || Vec::with_capacity(4096),
            |buffer| buffer.clear(),
            buffer_config,
        ));

        let string_pool = Arc::new(ObjectPool::with_reset(
            || String::with_capacity(256),
            |string| string.clear(),
            string_config,
        ));

        let manager = Self {
            transaction_pool,
            account_pool,
            buffer_pool,
            string_pool,
            metrics: Arc::new(RwLock::new(MemoryMetrics::default())),
            config: config.clone(),
        };

        // Start monitoring if enabled
        if config.enable_monitoring {
            manager.start_monitoring();
        }

        Ok(manager)
    }

    /// Get a transaction from the pool
    pub async fn get_transaction(&self) -> PooledObject<TransactionWrapper> {
        self.transaction_pool.get().await
    }

    /// Get an account wrapper from the pool
    pub async fn get_account(&self) -> PooledObject<AccountWrapper> {
        self.account_pool.get().await
    }

    /// Get a buffer from the pool
    pub async fn get_buffer(&self) -> PooledObject<Vec<u8>> {
        self.buffer_pool.get().await
    }

    /// Get a buffer from the pool (blocking version for sync contexts)
    pub fn get_buffer_blocking(&self) -> PooledObject<Vec<u8>> {
        // For now, create a new buffer since we can't block in sync context
        // In a real implementation, you might use a blocking pool or tokio::block_on
        PooledObject {
            object: Some(Vec::with_capacity(4096)),
            checkout_time: Instant::now(),
        }
    }

    /// Get a string from the pool
    pub async fn get_string(&self) -> PooledObject<String> {
        self.string_pool.get().await
    }

    /// Get comprehensive memory metrics
    pub async fn get_metrics(&self) -> Result<MemoryMetrics> {
        if !self.config.enable_monitoring {
            return Ok(MemoryMetrics::default());
        }

        let tx_metrics = self.transaction_pool.get_metrics().await;
        let account_metrics = self.account_pool.get_metrics().await;
        let buffer_metrics = self.buffer_pool.get_metrics().await;
        let string_metrics = self.string_pool.get_metrics().await;

        let total_allocated = tx_metrics.total_created + account_metrics.total_created 
            + buffer_metrics.total_created + string_metrics.total_created;
        let total_reused = tx_metrics.total_reused + account_metrics.total_reused 
            + buffer_metrics.total_reused + string_metrics.total_reused;

        // Estimate memory saved (rough calculation)
        let avg_object_size_kb = 4.0; // Average size assumption
        let memory_saved_mb = (total_reused as f64 * avg_object_size_kb) / 1024.0;

        let pool_efficiency = if total_allocated > 0 {
            total_reused as f64 / total_allocated as f64
        } else {
            0.0
        };

        let mut metrics = self.metrics.write().await;
        metrics.total_allocated_objects = total_allocated;
        metrics.total_reused_objects = total_reused;
        metrics.total_memory_saved_mb = memory_saved_mb;
        metrics.pool_efficiency = pool_efficiency;

        Ok(MemoryMetrics {
            total_allocated_objects: metrics.total_allocated_objects,
            total_reused_objects: metrics.total_reused_objects,
            total_memory_saved_mb: metrics.total_memory_saved_mb,
            pool_efficiency: metrics.pool_efficiency,
            gc_pressure: metrics.gc_pressure,
            allocation_rate: metrics.allocation_rate,
            deallocation_rate: metrics.deallocation_rate,
            last_gc: metrics.last_gc,
        })
    }

    /// Perform garbage collection if needed
    pub async fn maybe_gc(&self) -> Result<bool> {
        if !self.config.auto_gc_enabled {
            return Ok(false);
        }

        let metrics = self.get_metrics().await?;
        
        if metrics.pool_efficiency < (1.0 - self.config.gc_threshold) {
            self.perform_gc().await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Force garbage collection
    pub async fn perform_gc(&self) -> Result<()> {
        tracing::info!("Performing garbage collection on object pools");

        // Clear pools that have low efficiency
        let tx_metrics = self.transaction_pool.get_metrics().await;
        if tx_metrics.hit_rate < 0.3 {
            self.transaction_pool.clear().await;
        }

        let account_metrics = self.account_pool.get_metrics().await;
        if account_metrics.hit_rate < 0.3 {
            self.account_pool.clear().await;
        }

        let buffer_metrics = self.buffer_pool.get_metrics().await;
        if buffer_metrics.hit_rate < 0.3 {
            self.buffer_pool.clear().await;
        }

        let string_metrics = self.string_pool.get_metrics().await;
        if string_metrics.hit_rate < 0.3 {
            self.string_pool.clear().await;
        }

        // Update metrics
        if self.config.enable_monitoring {
            let mut metrics = self.metrics.write().await;
            metrics.last_gc = Some(chrono::Utc::now());
        }

        // Trigger JVM-like GC hint (though Rust doesn't have explicit GC)
        tokio::task::yield_now().await;

        Ok(())
    }

    /// Start memory monitoring
    fn start_monitoring(&self) {
        let manager = self.clone();
        let monitoring_interval = self.config.monitoring_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(monitoring_interval);
            
            loop {
                interval.tick().await;
                
                // Update metrics
                if let Ok(_) = manager.get_metrics().await {
                    // Metrics updated successfully
                }
                
                // Check if GC is needed
                if let Err(e) = manager.maybe_gc().await {
                    tracing::error!("Auto GC failed: {}", e);
                }
            }
        });
    }

    /// Get memory usage statistics
    pub async fn get_memory_usage(&self) -> MemoryUsageStats {
        let tx_metrics = self.transaction_pool.get_metrics().await;
        let account_metrics = self.account_pool.get_metrics().await;
        let buffer_metrics = self.buffer_pool.get_metrics().await;
        let string_metrics = self.string_pool.get_metrics().await;

        MemoryUsageStats {
            transaction_pool_size: tx_metrics.current_size,
            account_pool_size: account_metrics.current_size,
            buffer_pool_size: buffer_metrics.current_size,
            string_pool_size: string_metrics.current_size,
            total_pooled_objects: tx_metrics.current_size + account_metrics.current_size 
                + buffer_metrics.current_size + string_metrics.current_size,
            estimated_memory_mb: (tx_metrics.current_size * 1024 + account_metrics.current_size * 512 
                + buffer_metrics.current_size * 4 + string_metrics.current_size * 256) as f64 / 1024.0 / 1024.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryUsageStats {
    pub transaction_pool_size: usize,
    pub account_pool_size: usize,
    pub buffer_pool_size: usize,
    pub string_pool_size: usize,
    pub total_pooled_objects: usize,
    pub estimated_memory_mb: f64,
}

impl Clone for MemoryManager {
    fn clone(&self) -> Self {
        Self {
            transaction_pool: self.transaction_pool.clone(),
            account_pool: self.account_pool.clone(),
            buffer_pool: self.buffer_pool.clone(),
            string_pool: self.string_pool.clone(),
            metrics: self.metrics.clone(),
            config: self.config.clone(),
        }
    }
}
