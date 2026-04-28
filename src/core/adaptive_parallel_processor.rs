use crate::core::{Result, SolanaRecoverError, BatchScanRequest, BatchScanResult, ScanResult, ScanStatus};
use crate::core::scanner::WalletScanner;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, Mutex, RwLock};
use crossbeam::deque::{Injector, Stealer, Worker};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use serde::Serialize;
use futures::stream;
use uuid::Uuid;

/// Adaptive parallel processor with work-stealing and dynamic resource management
pub struct AdaptiveParallelProcessor {
    work_queue: Arc<Injector<WalletTask>>,
    stealers: Vec<Stealer<WalletTask>>,
    resource_monitor: Arc<ResourceMonitor>,
    batch_sizer: Arc<DynamicBatchSizer>,
    thread_pool: Arc<rayon::ThreadPool>,
    config: ProcessorConfig,
    metrics: Arc<RwLock<ProcessorMetrics>>,
    shutdown_signal: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct WalletTask {
    pub id: Uuid,
    pub wallet_address: String,
    pub priority: TaskPriority,
    pub complexity_estimate: TaskComplexity,
    pub created_at: Instant,
    pub retry_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

#[derive(Debug, Clone)]
pub struct TaskComplexity {
    pub estimated_accounts: usize,
    pub estimated_rpc_calls: usize,
    pub estimated_memory_mb: f64,
    pub estimated_cpu_time_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub max_workers: usize,
    pub max_concurrent_tasks: usize,
    pub work_stealing_enabled: bool,
    pub cpu_affinity_enabled: bool,
    pub adaptive_batching: bool,
    pub resource_monitoring: bool,
    pub load_balancing_strategy: LoadBalancingStrategy,
    pub task_timeout: Duration,
    pub worker_idle_timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    WorkStealing,
    LoadAware,
    ComplexityAware,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ProcessorMetrics {
    pub total_tasks_processed: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub avg_task_duration_ms: f64,
    pub avg_batch_size: f64,
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub worker_utilization: f64,
    pub queue_depth: usize,
    pub steal_operations: u64,
    pub load_balancing_efficiency: f64,
    pub last_adjustment: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            max_workers: num_cpus::get(),
            max_concurrent_tasks: 100,
            work_stealing_enabled: true,
            cpu_affinity_enabled: true,
            adaptive_batching: true,
            resource_monitoring: true,
            load_balancing_strategy: LoadBalancingStrategy::WorkStealing,
            task_timeout: Duration::from_secs(300),
            worker_idle_timeout: Duration::from_secs(60),
        }
    }
}

impl AdaptiveParallelProcessor {
    pub fn new(_scanner: Arc<WalletScanner>, config: ProcessorConfig) -> Result<Self> {
        // Create work queue and stealers for each worker
        let work_queue = Arc::new(Injector::new());
        let mut stealers = Vec::new();
        
        for _ in 0..config.max_workers {
            let worker = Worker::new_fifo();
            stealers.push(worker.stealer());
        }
        
        // Create thread pool with custom configuration
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(config.max_workers)
            .thread_name(|index| format!("solana-worker-{}", index))
            .build()
            .map_err(|e| SolanaRecoverError::InternalError(format!("Failed to create thread pool: {}", e)))?;
        
        let processor = Self {
            work_queue,
            stealers,
            resource_monitor: Arc::new(ResourceMonitor::new(config.resource_monitoring)),
            batch_sizer: Arc::new(DynamicBatchSizer::new()),
            thread_pool: Arc::new(thread_pool),
            config: config.clone(),
            metrics: Arc::new(RwLock::new(ProcessorMetrics::default())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        };
        
        // Start background monitoring if enabled
        if config.resource_monitoring {
            processor.start_monitoring();
        }
        
        Ok(processor)
    }

    /// Process a batch of wallets with adaptive parallelization
    pub async fn process_batch_adaptive(&self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        let mut results = Vec::new();
        let mut successful_scans = 0;
        let mut failed_scans = 0;
        let mut total_recoverable_sol = 0.0;

        // Create tasks from wallet addresses
        let tasks = self.create_tasks_from_request(request).await?;
        
        // Determine optimal batch size based on current system load
        let batch_size = if self.config.adaptive_batching {
            self.batch_sizer.calculate_optimal_batch_size(&tasks).await
        } else {
            self.config.max_concurrent_tasks
        };

        tracing::info!(
            "Processing {} wallets with adaptive batch size {}",
            tasks.len(),
            batch_size
        );

        // Process tasks in adaptive batches
        let task_chunks = tasks.chunks(batch_size);
        
        for chunk in task_chunks {
            let chunk_results = self.process_task_chunk(chunk).await?;
            
            for result in chunk_results {
                match result.status {
                    ScanStatus::Completed => {
                        successful_scans += 1;
                        if let Some(wallet_info) = &result.result {
                            total_recoverable_sol += wallet_info.recoverable_sol;
                        }
                    }
                    ScanStatus::Failed => {
                        failed_scans += 1;
                    }
                    _ => {
                        failed_scans += 1;
                    }
                }
                results.push(result);
            }
            
            // Adaptive delay between batches based on system load
            if self.config.resource_monitoring {
                self.adaptive_delay().await;
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        // Update metrics
        self.update_batch_metrics(tasks.len(), successful_scans, failed_scans, duration_ms).await;

        let fee_structure = request.fee_percentage
            .map(|p| crate::core::FeeStructure { percentage: p, ..Default::default() })
            .unwrap_or_default();
        
        let estimated_fee_sol = total_recoverable_sol * fee_structure.percentage;

        Ok(BatchScanResult {
            request_id: request.id,
            batch_id: Some(request.id.to_string()),
            total_wallets: request.wallet_addresses.len(),
            successful_scans,
            failed_scans,
            completed_wallets: successful_scans,
            failed_wallets: failed_scans,
            total_recoverable_sol,
            estimated_fee_sol,
            results,
            created_at: request.created_at,
            completed_at: Some(chrono::Utc::now()),
            duration_ms: Some(duration_ms),
            scan_time_ms: duration_ms,
        })
    }

    /// Process a chunk of tasks using work-stealing
    async fn process_task_chunk(&self, tasks: &[WalletTask]) -> Result<Vec<ScanResult>> {
        let tasks: Vec<WalletTask> = tasks.to_vec();
        let work_queue = self.work_queue.clone();
        let stealers = self.stealers.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        
        // Add tasks to work queue
        for task in &tasks {
            work_queue.push(task.clone());
        }

        // Create worker handles
        let worker_count = std::cmp::min(tasks.len(), self.config.max_workers);
        let semaphore = Arc::new(Semaphore::new(worker_count));
        let results = Arc::new(Mutex::new(Vec::new()));

        let worker_futures = (0..worker_count).map(|worker_id| {
            let semaphore = semaphore.clone();
            let results = results.clone();
            let work_queue = work_queue.clone();
            let stealers = stealers.clone();
            let shutdown_signal = shutdown_signal.clone();
            let config = self.config.clone();
            
            async move {
                let _permit = semaphore.acquire().await
                    .map_err(|_| SolanaRecoverError::InternalError("Failed to acquire worker permit".to_string()))?;
                
                Self::worker_loop(
                    worker_id,
                    work_queue,
                    &stealers,
                    results,
                    shutdown_signal,
                    config,
                ).await
            }
        });

        // Wait for all workers to complete
        let worker_results = futures::future::join_all(worker_futures).await;
        
        // Check for worker errors
        for result in worker_results {
            result?;
        }

        // Collect results
        let results_guard = results.lock().await;
        Ok(results_guard.clone())
    }

    /// Worker loop with work-stealing
    async fn worker_loop(
        _worker_id: usize,
        work_queue: Arc<Injector<WalletTask>>,
        stealers: &[Stealer<WalletTask>],
        results: Arc<Mutex<Vec<ScanResult>>>,
        shutdown_signal: Arc<AtomicBool>,
        config: ProcessorConfig,
    ) -> Result<()> {
        let worker = Worker::new_fifo();
        let mut local_queue = worker;
        
        loop {
            // Try to get work from local queue first
            let task = if let Some(task) = local_queue.pop() {
                Some(task)
            } else {
                // Try to steal from other workers
                Self::steal_task(&work_queue, stealers, &mut local_queue).await
            };
            
            match task {
                Some(task) => {
                    let result = Self::process_single_task(task, config.task_timeout).await?;
                    
                    let mut results_guard = results.lock().await;
                    results_guard.push(result);
                    drop(results_guard);
                }
                None => {
                    // No work available, check if we should shut down
                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }
                    
                    // Brief sleep to prevent busy waiting
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
        
        Ok(())
    }

    /// Steal tasks from other workers or global queue
    async fn steal_task(
        global_queue: &Injector<WalletTask>,
        stealers: &[Stealer<WalletTask>],
        _local_queue: &mut Worker<WalletTask>,
    ) -> Option<WalletTask> {
        // Try global queue first
        match global_queue.steal() {
            crossbeam::deque::Steal::Success(task) => return Some(task),
            _ => {}
        }
        
        // Try to steal from other workers
        for stealer in stealers {
            match stealer.steal() {
                crossbeam::deque::Steal::Success(task) => return Some(task),
                _ => {}
            }
        }
        
        None
    }

    /// Process a single wallet task
    async fn process_single_task(task: WalletTask, timeout: Duration) -> Result<ScanResult> {
        let start_time = Instant::now();
        
        // Process with timeout
        let result = tokio::time::timeout(timeout, async {
            // This would integrate with the actual wallet scanner
            // For now, return a placeholder result
            Ok::<ScanResult, SolanaRecoverError>(ScanResult {
                id: task.id,
                wallet_address: task.wallet_address.clone(),
                status: ScanStatus::Completed,
                result: None, // Would contain actual WalletInfo
                empty_accounts_found: 0,
                recoverable_sol: 0.0,
                scan_time_ms: start_time.elapsed().as_millis() as u64,
                created_at: chrono::Utc::now(),
                completed_at: Some(chrono::Utc::now()),
                error_message: None,
            })
        }).await;
        
        match result {
            Ok(Ok(scan_result)) => {
                tracing::debug!(
                    "Completed task {} for wallet {} in {}ms",
                    task.id,
                    task.wallet_address,
                    start_time.elapsed().as_millis()
                );
                Ok(scan_result)
            }
            Ok(Err(e)) => {
                tracing::error!("Task {} failed: {}", task.id, e);
                Ok(ScanResult {
                    id: task.id,
                    wallet_address: task.wallet_address,
                    status: ScanStatus::Failed,
                    result: None,
                    empty_accounts_found: 0,
                    recoverable_sol: 0.0,
                    scan_time_ms: start_time.elapsed().as_millis() as u64,
                    created_at: chrono::Utc::now(),
                    completed_at: Some(chrono::Utc::now()),
                    error_message: Some(e.to_string()),
                })
            }
            Err(_) => {
                tracing::warn!("Task {} timed out after {:?}", task.id, timeout);
                Ok(ScanResult {
                    id: task.id,
                    wallet_address: task.wallet_address,
                    status: ScanStatus::Failed,
                    result: None,
                    empty_accounts_found: 0,
                    recoverable_sol: 0.0,
                    scan_time_ms: start_time.elapsed().as_millis() as u64,
                    created_at: chrono::Utc::now(),
                    completed_at: Some(chrono::Utc::now()),
                    error_message: Some("Task timed out".to_string()),
                })
            }
        }
    }

    /// Create tasks from batch scan request
    async fn create_tasks_from_request(&self, request: &BatchScanRequest) -> Result<Vec<WalletTask>> {
        let mut tasks = Vec::with_capacity(request.wallet_addresses.len());
        
        for (index, wallet_address) in request.wallet_addresses.iter().enumerate() {
            let task = WalletTask {
                id: Uuid::new_v4(),
                wallet_address: wallet_address.clone(),
                priority: self.determine_task_priority(index, request.wallet_addresses.len()),
                complexity_estimate: self.estimate_task_complexity(wallet_address).await?,
                created_at: Instant::now(),
                retry_count: 0,
                max_retries: 3,
            };
            
            tasks.push(task);
        }
        
        // Sort tasks by priority and complexity
        tasks.sort_by(|a, b| {
            b.priority.cmp(&a.priority)
                .then_with(|| a.complexity_estimate.estimated_cpu_time_ms.cmp(&b.complexity_estimate.estimated_cpu_time_ms))
        });
        
        Ok(tasks)
    }

    fn determine_task_priority(&self, index: usize, total_tasks: usize) -> TaskPriority {
        let progress_ratio = index as f64 / total_tasks as f64;
        
        if progress_ratio < 0.1 {
            TaskPriority::High // First 10% get high priority
        } else if progress_ratio < 0.5 {
            TaskPriority::Medium // Next 40% get medium priority
        } else {
            TaskPriority::Low // Remaining 50% get low priority
        }
    }

    async fn estimate_task_complexity(&self, _wallet_address: &str) -> Result<TaskComplexity> {
        // This would typically query historical data or make a lightweight RPC call
        // For now, use reasonable defaults
        Ok(TaskComplexity {
            estimated_accounts: 50, // Average estimate
            estimated_rpc_calls: 10,
            estimated_memory_mb: 10.0,
            estimated_cpu_time_ms: 500,
        })
    }

    /// Adaptive delay based on system load
    async fn adaptive_delay(&self) {
        if let Some(load_info) = self.resource_monitor.get_current_load().await {
            let delay_ms = if load_info.cpu_utilization > 0.8 {
                100 // High load - longer delay
            } else if load_info.cpu_utilization > 0.6 {
                50  // Medium load - moderate delay
            } else {
                10  // Low load - minimal delay
            };
            
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    /// Update batch processing metrics
    async fn update_batch_metrics(&self, total_tasks: usize, successful: usize, failed: usize, duration_ms: u64) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_tasks_processed += total_tasks as u64;
        metrics.successful_tasks += successful as u64;
        metrics.failed_tasks += failed as u64;
        
        // Update average task duration
        let total_processed = metrics.total_tasks_processed;
        if total_processed > 0 {
            metrics.avg_task_duration_ms = 
                (metrics.avg_task_duration_ms * (total_processed - total_tasks as u64) as f64 + duration_ms as f64)
                / total_processed as f64;
        }
        
        // Update queue depth (simplified - in production would track actual queue length)
        metrics.queue_depth = 0; // Placeholder
        
        // Update worker utilization (simplified)
        metrics.worker_utilization = (successful as f64 / total_tasks as f64) * 100.0;
    }

    /// Start background resource monitoring
    fn start_monitoring(&self) {
        let resource_monitor = self.resource_monitor.clone();
        let metrics = self.metrics.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                if let Some(load_info) = resource_monitor.get_current_load().await {
                    let mut metrics_guard = metrics.write().await;
                    metrics_guard.cpu_utilization = load_info.cpu_utilization;
                    metrics_guard.memory_utilization = load_info.memory_utilization;
                }
            }
        });
    }

    /// Get current processor metrics
    pub async fn get_metrics(&self) -> ProcessorMetrics {
        let metrics = self.metrics.read().await;
        ProcessorMetrics {
            total_tasks_processed: metrics.total_tasks_processed,
            successful_tasks: metrics.successful_tasks,
            failed_tasks: metrics.failed_tasks,
            avg_task_duration_ms: metrics.avg_task_duration_ms,
            avg_batch_size: metrics.avg_batch_size,
            cpu_utilization: metrics.cpu_utilization,
            memory_utilization: metrics.memory_utilization,
            worker_utilization: metrics.worker_utilization,
            queue_depth: metrics.queue_depth,
            steal_operations: metrics.steal_operations,
            load_balancing_efficiency: metrics.load_balancing_efficiency,
            last_adjustment: metrics.last_adjustment,
        }
    }

    /// Shutdown the processor gracefully
    pub async fn shutdown(&self) {
        self.shutdown_signal.store(true, Ordering::Relaxed);
        
        // Give workers time to finish current tasks
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        tracing::info!("Adaptive parallel processor shutdown complete");
    }
}

/// Resource monitor for system load tracking
pub struct ResourceMonitor {
    enabled: bool,
}

#[derive(Debug, Clone)]
pub struct LoadInfo {
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub available_memory_mb: f64,
    pub active_processes: usize,
}

impl ResourceMonitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
        }
    }

    pub async fn get_current_load(&self) -> Option<LoadInfo> {
        if !self.enabled {
            return None;
        }

        // This would typically use system APIs to get real metrics
        // For now, return simulated values
        Some(LoadInfo {
            cpu_utilization: 0.5, // 50% CPU usage
            memory_utilization: 0.3, // 30% memory usage
            available_memory_mb: 8000.0,
            active_processes: 150,
        })
    }
}

/// Dynamic batch sizer for adaptive batch processing
pub struct DynamicBatchSizer {
    current_size: Arc<AtomicUsize>,
    last_adjustment: Arc<Mutex<Option<Instant>>>,
}

impl DynamicBatchSizer {
    pub fn new() -> Self {
        Self {
            current_size: Arc::new(AtomicUsize::new(50)),
            last_adjustment: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn calculate_optimal_batch_size(&self, tasks: &[WalletTask]) -> usize {
        let current_size = self.current_size.load(Ordering::Relaxed);
        
        // Simple heuristic: adjust based on task complexity
        let avg_complexity: f64 = tasks.iter()
            .map(|t| t.complexity_estimate.estimated_cpu_time_ms as f64)
            .sum::<f64>() / tasks.len() as f64;
        
        let adjusted_size = if avg_complexity > 1000.0 {
            // High complexity - smaller batches
            (current_size as f64 * 0.7) as usize
        } else if avg_complexity < 200.0 {
            // Low complexity - larger batches
            (current_size as f64 * 1.3) as usize
        } else {
            current_size
        };
        
        // Ensure bounds
        std::cmp::max(10, std::cmp::min(adjusted_size, 200))
    }

    pub fn adjust_batch_size(&self, performance_factor: f64) {
        let current_size = self.current_size.load(Ordering::Relaxed);
        let new_size = (current_size as f64 * performance_factor) as usize;
        
        // Update with bounds checking
        let bounded_size = std::cmp::max(10, std::cmp::min(new_size, 200));
        self.current_size.store(bounded_size, Ordering::Relaxed);
        
        // Update last adjustment time
        if let Ok(mut last_adjustment) = self.last_adjustment.try_lock() {
            *last_adjustment = Some(Instant::now());
        }
    }
}

impl Clone for AdaptiveParallelProcessor {
    fn clone(&self) -> Self {
        Self {
            work_queue: self.work_queue.clone(),
            stealers: self.stealers.clone(),
            resource_monitor: self.resource_monitor.clone(),
            batch_sizer: self.batch_sizer.clone(),
            thread_pool: self.thread_pool.clone(),
            config: self.config.clone(),
            metrics: self.metrics.clone(),
            shutdown_signal: self.shutdown_signal.clone(),
        }
    }
}
