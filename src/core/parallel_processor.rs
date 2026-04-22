use crate::core::{BatchScanRequest, BatchScanResult, ScanResult, ScanStatus, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;
use chrono::Utc;
use crossbeam::queue::SegQueue;
use dashmap::DashMap;
use tokio::sync::Semaphore;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

/// Priority levels for wallet processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// A wallet task with metadata for intelligent processing
#[derive(Debug, Clone)]
pub struct WalletTask {
    pub wallet_address: String,
    pub task_id: u64,
    pub priority: Priority,
    pub dependencies: Vec<u64>,
    pub retry_count: u32,
    pub estimated_complexity: f64,
    pub created_at: std::time::SystemTime,
}

impl WalletTask {
    pub fn new(wallet_address: String, priority: Priority) -> Self {
        Self {
            wallet_address,
            task_id: uuid::Uuid::new_v4().as_u128() as u64,
            priority,
            dependencies: Vec::new(),
            retry_count: 0,
            estimated_complexity: 1.0,
            created_at: std::time::SystemTime::now(),
        }
    }
}

/// Simple work-stealing queue using standard library primitives
pub struct WorkStealingQueue<T: Send> {
    global_queue: Arc<SegQueue<T>>,
    worker_queues: Arc<Vec<Arc<SegQueue<T>>>>,
    num_workers: usize,
}

impl<T: Send> WorkStealingQueue<T> {
    pub fn new(num_workers: usize) -> Self {
        let mut worker_queues = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            worker_queues.push(Arc::new(SegQueue::new()));
        }
        
        Self {
            global_queue: Arc::new(SegQueue::new()),
            worker_queues: Arc::new(worker_queues),
            num_workers,
        }
    }
    
    /// Push a task to the global queue
    pub fn push(&self, task: T) {
        self.global_queue.push(task);
    }
    
    /// Push a task to a specific worker's local queue
    pub fn push_local(&self, worker_id: usize, task: T) {
        if worker_id < self.worker_queues.len() {
            self.worker_queues[worker_id].push(task);
        } else {
            self.global_queue.push(task);
        }
    }
    
    /// Get next task for a worker (local first, then global, then steal)
    pub fn get_task(&self, worker_id: usize) -> Option<T> {
        // Try local queue first
        if worker_id < self.worker_queues.len() {
            if let Some(task) = self.worker_queues[worker_id].pop() {
                return Some(task);
            }
        }
        
        // Try global queue
        if let Some(task) = self.global_queue.pop() {
            return Some(task);
        }
        
        // Try to steal from other workers
        for (i, queue) in self.worker_queues.iter().enumerate() {
            if i != worker_id {
                if let Some(task) = queue.pop() {
                    return Some(task);
                }
            }
        }
        
        None
    }
    
    /// Get the number of workers
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }
    
    /// Check if all queues appear to be empty
    pub fn is_empty(&self) -> bool {
        // Check global queue
        if self.global_queue.pop().is_some() {
            return false;
        }
        
        // Check worker queues
        for queue in self.worker_queues.iter() {
            if queue.pop().is_some() {
                return false;
            }
        }
        
        true
    }
}

/// Progress tracking for batch operations
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    total_tasks: Arc<AtomicUsize>,
    completed_tasks: Arc<AtomicUsize>,
    failed_tasks: Arc<AtomicUsize>,
    start_time: Arc<AtomicU64>,
    wallet_results: Arc<DashMap<String, ScanResult>>,
}

impl ProgressTracker {
    pub fn new(total_tasks: usize) -> Self {
        Self {
            total_tasks: Arc::new(AtomicUsize::new(total_tasks)),
            completed_tasks: Arc::new(AtomicUsize::new(0)),
            failed_tasks: Arc::new(AtomicUsize::new(0)),
            start_time: Arc::new(AtomicU64::new(Utc::now().timestamp() as u64)),
            wallet_results: Arc::new(DashMap::new()),
        }
    }
    
    pub fn increment_completed(&self) {
        self.completed_tasks.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn increment_failed(&self) {
        self.failed_tasks.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn add_result(&self, wallet_address: String, result: ScanResult) {
        self.wallet_results.insert(wallet_address, result);
    }
    
    pub fn get_progress(&self) -> (usize, usize, usize) {
        let total = self.total_tasks.load(Ordering::Relaxed);
        let completed = self.completed_tasks.load(Ordering::Relaxed);
        let failed = self.failed_tasks.load(Ordering::Relaxed);
        (total, completed, failed)
    }
    
    pub fn get_results(&self) -> Vec<ScanResult> {
        self.wallet_results.iter().map(|entry| entry.value().clone()).collect()
    }
    
    pub fn get_elapsed_ms(&self) -> u64 {
        let start = self.start_time.load(Ordering::Relaxed);
        let now = Utc::now().timestamp() as u64;
        now.saturating_sub(start) * 1000
    }
    
    pub fn get_throughput(&self) -> f64 {
        let (_, completed, _) = self.get_progress();
        let elapsed_ms = self.get_elapsed_ms();
        if elapsed_ms > 0 {
            completed as f64 / (elapsed_ms as f64 / 1000.0)
        } else {
            0.0
        }
    }
}

/// Trait for resource monitoring implementations
pub trait ResourceMonitorTrait: Send + Sync {
    fn get_metrics(&self) -> ResourceMetrics;
}

/// Resource monitoring for system health
#[derive(Debug, Clone)]
pub struct ResourceMonitor {
    cpu_usage: Arc<AtomicU64>, // Percentage * 100
    memory_usage_mb: Arc<AtomicU64>,
    network_rps: Arc<AtomicU64>,
    active_threads: Arc<AtomicUsize>,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            cpu_usage: Arc::new(AtomicU64::new(0)),
            memory_usage_mb: Arc::new(AtomicU64::new(0)),
            network_rps: Arc::new(AtomicU64::new(0)),
            active_threads: Arc::new(AtomicUsize::new(rayon::current_num_threads())),
        }
    }
    
    pub fn update_cpu_usage(&self, percentage: f64) {
        self.cpu_usage.store((percentage * 100.0) as u64, Ordering::Relaxed);
    }
    
    pub fn update_memory_usage(&self, mb: u64) {
        self.memory_usage_mb.store(mb, Ordering::Relaxed);
    }
    
    pub fn update_network_rps(&self, rps: u64) {
        self.network_rps.store(rps, Ordering::Relaxed);
    }
    
    pub fn get_metrics(&self) -> ResourceMetrics {
        ResourceMetrics {
            cpu_usage_percent: self.cpu_usage.load(Ordering::Relaxed) as f64 / 100.0,
            memory_usage_mb: self.memory_usage_mb.load(Ordering::Relaxed),
            network_requests_per_second: self.network_rps.load(Ordering::Relaxed),
            active_threads: self.active_threads.load(Ordering::Relaxed),
        }
    }
}

impl ResourceMonitorTrait for ResourceMonitor {
    fn get_metrics(&self) -> ResourceMetrics {
        self.get_metrics()
    }
}

#[derive(Debug, Clone)]
pub struct ResourceMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub network_requests_per_second: u64,
    pub active_threads: usize,
}

/// Dynamic batch sizer based on system load
#[derive(Clone)]
pub struct DynamicBatchSizer {
    pub base_batch_size: usize,
    pub min_batch_size: usize,
    pub max_batch_size: usize,
    resource_monitor: Arc<dyn ResourceMonitorTrait>,
    last_adjustment: Arc<AtomicU64>,
}

impl DynamicBatchSizer {
    pub fn new(base_batch_size: usize, resource_monitor: Arc<dyn ResourceMonitorTrait>) -> Self {
        Self {
            base_batch_size,
            min_batch_size: base_batch_size / 4,
            max_batch_size: base_batch_size * 4,
            resource_monitor,
            last_adjustment: Arc::new(AtomicU64::new(0)),
        }
    }
    
    pub fn get_optimal_batch_size(&self) -> usize {
        let metrics = self.resource_monitor.get_metrics();
        
        // Adjust batch size based on CPU and memory usage
        let cpu_factor = if metrics.cpu_usage_percent < 50.0 {
            2.0 // Scale up if CPU is underutilized
        } else if metrics.cpu_usage_percent > 80.0 {
            0.5 // Scale down if CPU is overloaded
        } else {
            1.0 // Keep current if CPU is moderately used
        };
        
        let memory_factor = if metrics.memory_usage_mb < 1024 { // < 1GB
            1.5
        } else if metrics.memory_usage_mb > 4096 { // > 4GB
            0.7
        } else {
            1.0
        };
        
        let adjusted_size = (self.base_batch_size as f64 * cpu_factor * memory_factor) as usize;
        adjusted_size.clamp(self.min_batch_size, self.max_batch_size)
    }
    
    pub fn should_adjust(&self) -> bool {
        let now = Utc::now().timestamp() as u64;
        let last = self.last_adjustment.load(Ordering::Relaxed);
        now - last > 30 // Adjust every 30 seconds
    }
    
    pub fn mark_adjustment(&self) {
        let now = Utc::now().timestamp() as u64;
        self.last_adjustment.store(now, Ordering::Relaxed);
    }
}

/// Enhanced parallel processor with work-stealing queue and dynamic optimization
pub struct IntelligentParallelProcessor {
    pub work_queue: Arc<WorkStealingQueue<WalletTask>>,
    pub worker_pool: Arc<rayon::ThreadPool>,
    pub progress_tracker: Arc<ProgressTracker>,
    pub resource_monitor: Arc<ResourceMonitor>,
    pub batch_sizer: Arc<DynamicBatchSizer>,
    pub semaphore: Arc<Semaphore>,
    pub scanner: Arc<crate::core::scanner::WalletScanner>,
    pub max_workers: usize,
}

impl IntelligentParallelProcessor {
    pub fn new(
        scanner: Arc<crate::core::scanner::WalletScanner>,
        max_workers: Option<usize>,
        max_concurrent_tasks: usize,
    ) -> Result<Self> {
        let num_workers = max_workers.unwrap_or_else(|| rayon::current_num_threads());
        let work_queue = Arc::new(WorkStealingQueue::new(num_workers));
        
        let worker_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_workers)
            .thread_name(|i| format!("wallet-worker-{}", i))
            .build()
            .map_err(|e| crate::core::SolanaRecoverError::InternalError(format!("Failed to create thread pool: {}", e)))?;
        
        let resource_monitor = Arc::new(ResourceMonitor::new());
        let resource_monitor_for_sizer: Arc<dyn ResourceMonitorTrait> = resource_monitor.clone();
        
        Ok(Self {
            work_queue,
            worker_pool: Arc::new(worker_pool),
            progress_tracker: Arc::new(ProgressTracker::new(0)),
            resource_monitor,
            batch_sizer: Arc::new(DynamicBatchSizer::new(100, resource_monitor_for_sizer)),
            semaphore: Arc::new(Semaphore::new(max_concurrent_tasks)),
            scanner,
            max_workers: num_workers,
        })
    }
    
    pub async fn process_batch_intelligently(&mut self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        
        // Create tasks from wallet addresses
        let tasks: Vec<WalletTask> = request.wallet_addresses
            .iter()
            .enumerate()
            .map(|(i, addr)| {
                let priority = if i < 10 { Priority::High } else { Priority::Medium };
                WalletTask::new(addr.clone(), priority)
            })
            .collect();
        
        // Update progress tracker
        self.progress_tracker = Arc::new(ProgressTracker::new(tasks.len()));
        
        // Distribute tasks across workers using work-stealing
        for (i, task) in tasks.into_iter().enumerate() {
            // Distribute tasks round-robin to local queues for better cache locality
            let worker_id = i % self.max_workers;
            self.work_queue.push_local(worker_id, task);
        }
        
        // Process tasks using work-stealing algorithm
        let results = self.process_tasks_with_work_stealing().await?;
        
        // Compile results
        let completed_wallets = results.iter()
            .filter(|r| r.status == ScanStatus::Completed)
            .count();
        
        let failed_wallets = results.iter()
            .filter(|r| r.status == ScanStatus::Failed)
            .count();
        
        let total_recoverable_sol: f64 = results.iter()
            .filter_map(|r| r.result.as_ref())
            .map(|w| w.recoverable_sol)
            .sum();
        
        let fee_structure = request.fee_percentage
            .map(|p| crate::core::FeeStructure { percentage: p, ..Default::default() })
            .unwrap_or_default();
        
        let estimated_fee_sol = self.calculate_fee(total_recoverable_sol, &fee_structure);
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(BatchScanResult {
            id: request.id,
            batch_id: Some(request.id.to_string()),
            total_wallets: request.wallet_addresses.len(),
            successful_scans: completed_wallets,
            failed_scans: failed_wallets,
            completed_wallets,
            failed_wallets,
            total_recoverable_sol,
            estimated_fee_sol,
            results,
            created_at: request.created_at,
            completed_at: Some(Utc::now()),
            duration_ms: Some(duration_ms),
        })
    }
    
    async fn process_tasks_with_work_stealing(&self) -> Result<Vec<ScanResult>> {
        let progress_tracker = Arc::clone(&self.progress_tracker);
        let resource_monitor = Arc::clone(&self.resource_monitor);
        let semaphore = Arc::clone(&self.semaphore);
        let scanner = Arc::clone(&self.scanner);
        let work_queue = Arc::clone(&self.work_queue);
        
        let results_queue = Arc::new(SegQueue::new());
        let num_workers = self.max_workers;
        
        // Spawn worker threads that use work-stealing
        let mut handles = Vec::new();
        
        for worker_id in 0..num_workers {
            let progress_tracker = Arc::clone(&progress_tracker);
            let _resource_monitor = Arc::clone(&resource_monitor);
            let semaphore = Arc::clone(&semaphore);
            let scanner = Arc::clone(&scanner);
            let results_queue = Arc::clone(&results_queue);
            let work_queue = Arc::clone(&work_queue);
            
            let handle = tokio::spawn(async move {
                // Each worker continuously processes tasks until all are done
                loop {
                    // Get task using work-stealing algorithm
                    if let Some(task) = work_queue.get_task(worker_id) {
                        let _permit = semaphore.acquire().await.unwrap();
                        
                        let result = match scanner.scan_wallet(&task.wallet_address).await {
                            Ok(mut scan_result) => {
                                scan_result.status = ScanStatus::Completed;
                                progress_tracker.increment_completed();
                                scan_result
                            }
                            Err(e) => {
                                progress_tracker.increment_failed();
                                ScanResult {
                                    id: Uuid::new_v4(),
                                    wallet_address: task.wallet_address.clone(),
                                    status: ScanStatus::Failed,
                                    result: None,
                                    error: Some(e.to_string()),
                                    created_at: Utc::now(),
                                }
                            }
                        };
                        
                        results_queue.push(result);
                    } else {
                        // No more tasks available, check if all workers are done
                        if work_queue.is_empty() {
                            break;
                        }
                        // Brief pause before trying again
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all workers to complete
        for handle in handles {
            handle.await.map_err(|e| {
                crate::core::SolanaRecoverError::InternalError(format!("Worker failed: {:?}", e))
            })?;
        }
        
        // Collect results
        let mut results = Vec::new();
        while let Some(result) = results_queue.pop() {
            results.push(result);
        }
        
        Ok(results)
    }
    
    fn calculate_fee(&self, total_sol: f64, fee_structure: &crate::core::FeeStructure) -> f64 {
        let total_lamports = (total_sol * 1_000_000_000.0) as u64;
        
        if let Some(waive_threshold) = fee_structure.waive_below_lamports {
            if total_lamports <= waive_threshold {
                return 0.0;
            }
        }
        
        let fee_lamports = (total_lamports as f64 * fee_structure.percentage) as u64;
        
        let final_fee = fee_lamports
            .max(fee_structure.minimum_lamports)
            .min(fee_structure.maximum_lamports.unwrap_or(u64::MAX));
        
        final_fee as f64 / 1_000_000_000.0
    }
    
    pub fn get_progress(&self) -> (usize, usize, usize) {
        self.progress_tracker.get_progress()
    }
    
    pub async fn get_resource_metrics(&self) -> ResourceMetrics {
        let snapshot = self.resource_monitor.get_metrics();
        snapshot
    }
    
    pub fn get_optimal_batch_size(&self) -> usize {
        self.batch_sizer.get_optimal_batch_size()
    }
    
    pub fn get_throughput(&self) -> f64 {
        self.progress_tracker.get_throughput()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scanner::WalletScanner;
    use crate::rpc::mock::MockConnectionPool;
    
    #[tokio::test]
    async fn test_work_stealing_queue() {
        let queue: WorkStealingQueue<i32> = WorkStealingQueue::new(4);
        
        // Push items to global injector
        for i in 0..10 {
            queue.push(i);
        }
        
        // Test work-stealing behavior
        let mut items = Vec::new();
        for worker_id in 0..4 {
            // Each worker tries to get tasks
            while let Some(item) = queue.get_task(worker_id) {
                items.push(item);
            }
        }
        
        assert_eq!(items.len(), 10);
    }
    
    #[tokio::test]
    async fn test_progress_tracker() {
        let tracker = ProgressTracker::new(100);
        
        assert_eq!(tracker.get_progress(), (100, 0, 0));
        
        tracker.increment_completed();
        tracker.increment_completed();
        tracker.increment_failed();
        
        assert_eq!(tracker.get_progress(), (100, 2, 1));
    }
    
    #[tokio::test]
    async fn test_dynamic_batch_sizer() {
        let monitor = Arc::new(ResourceMonitor::new());
        let monitor_trait: Arc<dyn ResourceMonitorTrait> = Arc::clone(&monitor) as Arc<dyn ResourceMonitorTrait>;
        let sizer = DynamicBatchSizer::new(100, monitor_trait);
        
        // Test with low CPU usage
        monitor.update_cpu_usage(25.0);
        assert!(sizer.get_optimal_batch_size() > 100);
        
        // Test with high CPU usage
        monitor.update_cpu_usage(85.0);
        assert!(sizer.get_optimal_batch_size() < 100);
    }
}
