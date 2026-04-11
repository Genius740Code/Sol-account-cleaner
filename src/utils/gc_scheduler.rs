use std::sync::Arc;
use parking_lot::RwLock;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use chrono::Utc;
use std::collections::{HashMap, VecDeque};

/// Sophisticated garbage collection scheduler with adaptive strategies
#[derive(Debug)]
pub struct GcScheduler {
    /// Configuration for GC behavior
    config: GcSchedulerConfig,
    
    /// GC state tracking
    state: Arc<RwLock<GcSchedulerState>>,
    
    /// Semaphore to limit concurrent GC operations
    gc_semaphore: Arc<Semaphore>,
    
    /// GC history for adaptive decisions
    history: Arc<RwLock<VecDeque<GcExecution>>>,
    
    /// Memory pressure monitor
    memory_monitor: Arc<MemoryPressureMonitor>,
    
    /// Adaptive strategy engine
    adaptive_engine: Arc<AdaptiveGcEngine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcSchedulerConfig {
    /// Base GC interval in seconds
    pub base_interval_seconds: u64,
    
    /// Maximum GC interval in seconds
    pub max_interval_seconds: u64,
    
    /// Minimum GC interval in seconds
    pub min_interval_seconds: u64,
    
    /// Memory pressure threshold to trigger immediate GC (percentage)
    pub memory_pressure_threshold: f64,
    
    /// Enable adaptive GC scheduling
    pub enable_adaptive_scheduling: bool,
    
    /// Maximum concurrent GC operations
    pub max_concurrent_gc: usize,
    
    /// GC timeout in seconds
    pub gc_timeout_seconds: u64,
    
    /// Enable incremental GC
    pub enable_incremental_gc: bool,
    
    /// Incremental GC batch size
    pub incremental_batch_size: usize,
    
    /// GC priority levels
    pub priority_config: GcPriorityConfig,
    
    /// Performance targets
    pub performance_targets: GcPerformanceTargets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPriorityConfig {
    /// Critical memory pressure threshold
    pub critical_threshold: f64,
    
    /// High memory pressure threshold
    pub high_threshold: f64,
    
    /// Medium memory pressure threshold
    pub medium_threshold: f64,
    
    /// Low memory pressure threshold
    pub low_threshold: f64,
    
    /// Priority multipliers for intervals
    pub critical_interval_multiplier: f64,
    pub high_interval_multiplier: f64,
    pub medium_interval_multiplier: f64,
    pub low_interval_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPerformanceTargets {
    /// Target maximum GC pause time in milliseconds
    pub max_pause_time_ms: u64,
    
    /// Target GC efficiency (memory freed per millisecond)
    pub target_efficiency_mb_per_ms: f64,
    
    /// Target memory utilization percentage
    pub target_memory_utilization: f64,
    
    /// Maximum acceptable GC frequency per minute
    pub max_gc_frequency_per_minute: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcSchedulerState {
    pub total_gc_executions: u64,
    pub is_gc_running: bool,
    pub last_gc_time: Option<chrono::DateTime<chrono::Utc>>,
    pub total_gc_time: Duration,
    pub current_memory_pressure: f64,
    pub gc_queue_size: usize,
    pub next_scheduled_gc: Option<chrono::DateTime<chrono::Utc>>,
    pub adaptive_interval_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcExecution {
    #[serde(with = "chrono::serde::ts_seconds")]
    pub start_time: chrono::DateTime<chrono::Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub duration: Duration,
    pub memory_freed_bytes: usize,
    pub memory_before_gc: usize,
    pub memory_after_gc: usize,
    pub gc_type: GcType,
    pub priority: GcPriority,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GcType {
    /// Full garbage collection
    Full,
    
    /// Incremental garbage collection
    Incremental { phase: u8, total_phases: u8 },
    
    /// Generational garbage collection
    Generational { generation: GcGeneration },
    
    /// Concurrent garbage collection
    Concurrent,
    
    /// Emergency garbage collection (high memory pressure)
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum GcPriority {
    Critical = 4,
    High = 3,
    Medium = 2,
    Low = 1,
    Background = 0,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GcGeneration {
    Young,
    Mature,
    Old,
}

#[derive(Debug, Clone)]
pub struct MemoryPressureMonitor {
    #[allow(dead_code)]
    gc_engine: Arc<AdaptiveGcEngine>,
    #[allow(dead_code)]
    thresholds: GcPriorityConfig,
    current_pressure: Arc<RwLock<f64>>,
    #[allow(dead_code)]
    pressure_history: Arc<RwLock<VecDeque<(Instant, f64)>>>,
    #[allow(dead_code)]
    monitoring_interval: Duration,
}

#[derive(Debug)]
pub struct AdaptiveGcEngine {
    /// Learning rate for adaptive adjustments
    #[allow(dead_code)]
    learning_rate: f64,
    
    /// Performance history
    performance_history: Arc<RwLock<VecDeque<GcPerformance>>>,
    
    /// Adaptive parameters
    adaptive_params: Arc<RwLock<AdaptiveParameters>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPerformance {
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub gc_type: GcType,
    pub duration: Duration,
    pub memory_freed_bytes: usize,
    pub efficiency_mb_per_ms: f64,
    pub pause_time_ms: u64,
    pub memory_pressure_before: f64,
    pub memory_pressure_after: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveParameters {
    pub interval_multiplier: f64,
    pub batch_size_multiplier: f64,
    pub priority_threshold_adjustment: f64,
    pub gc_type_selection_weights: HashMap<GcType, f64>,
    pub learning_rate: f64,
}

impl Default for GcSchedulerConfig {
    fn default() -> Self {
        Self {
            base_interval_seconds: 60,
            max_interval_seconds: 300,
            min_interval_seconds: 10,
            memory_pressure_threshold: 80.0,
            enable_adaptive_scheduling: true,
            max_concurrent_gc: 1,
            gc_timeout_seconds: 30,
            enable_incremental_gc: true,
            incremental_batch_size: 100,
            priority_config: GcPriorityConfig::default(),
            performance_targets: GcPerformanceTargets::default(),
        }
    }
}

impl Default for GcPriorityConfig {
    fn default() -> Self {
        Self {
            critical_threshold: 90.0,
            high_threshold: 80.0,
            medium_threshold: 60.0,
            low_threshold: 40.0,
            critical_interval_multiplier: 0.1,  // 10x faster
            high_interval_multiplier: 0.25,     // 4x faster
            medium_interval_multiplier: 0.5,    // 2x faster
            low_interval_multiplier: 1.0,      // Normal speed
        }
    }
}

impl Default for GcPerformanceTargets {
    fn default() -> Self {
        Self {
            max_pause_time_ms: 100,
            target_efficiency_mb_per_ms: 1.0,
            target_memory_utilization: 70.0,
            max_gc_frequency_per_minute: 2.0,
        }
    }
}

impl Default for GcSchedulerState {
    fn default() -> Self {
        Self {
            last_gc_time: None,
            total_gc_executions: 0,
            total_gc_time: Duration::ZERO,
            current_memory_pressure: 0.0,
            is_gc_running: false,
            gc_queue_size: 0,
            next_scheduled_gc: None,
            adaptive_interval_multiplier: 1.0,
        }
    }
}

impl Default for AdaptiveParameters {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert(GcType::Full, 1.0);
        weights.insert(GcType::Incremental { phase: 1, total_phases: 4 }, 2.0);
        weights.insert(GcType::Generational { generation: GcGeneration::Young }, 1.5);
        weights.insert(GcType::Concurrent, 0.8);
        weights.insert(GcType::Emergency, 3.0);
        
        Self {
            interval_multiplier: 1.0,
            batch_size_multiplier: 1.0,
            priority_threshold_adjustment: 0.0,
            gc_type_selection_weights: weights,
            learning_rate: 0.1,
        }
    }
}

impl GcScheduler {
    pub fn new() -> Arc<Self> {
        Self::with_config(GcSchedulerConfig::default())
    }
    
    pub fn with_config(config: GcSchedulerConfig) -> Arc<Self> {
        let scheduler = Arc::new(Self {
            config: config.clone(),
            state: Arc::new(RwLock::new(GcSchedulerState::default())),
            gc_semaphore: Arc::new(Semaphore::new(config.max_concurrent_gc)),
            history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            memory_monitor: Arc::new(MemoryPressureMonitor::new(config.priority_config.clone())),
            adaptive_engine: Arc::new(AdaptiveGcEngine::new()),
        });
        
        // Start background monitoring
        scheduler.start_monitoring();
        
        scheduler
    }
    
    fn start_monitoring(self: &Arc<Self>) {
        let scheduler = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                // Update memory pressure
                let pressure = scheduler.memory_monitor.get_current_pressure().await;
                {
                    let mut state = scheduler.state.write();
                    state.current_memory_pressure = pressure;
                }
                
                // Check if GC should be triggered
                if scheduler.should_trigger_gc(pressure).await {
                    scheduler.schedule_gc(pressure).await;
                }
                
                // Update adaptive parameters
                if scheduler.config.enable_adaptive_scheduling {
                    scheduler.adaptive_engine.update_parameters(&scheduler).await;
                }
            }
        });
    }
    
    /// Schedule garbage collection based on current conditions
    pub async fn schedule_gc(&self, memory_pressure: f64) {
        let priority = self.determine_gc_priority(memory_pressure);
        let gc_type = self.select_gc_type(memory_pressure, priority.clone());
        
        debug!("Scheduling GC: pressure={:.1}%, priority={:?}, type={:?}", 
               memory_pressure, priority, gc_type);
        
        match priority {
            GcPriority::Critical => self.execute_emergency_gc(gc_type).await,
            GcPriority::High => self.execute_high_priority_gc(gc_type).await,
            GcPriority::Medium => self.execute_medium_priority_gc(gc_type).await,
            GcPriority::Low => self.execute_low_priority_gc(gc_type).await,
            GcPriority::Background => self.execute_background_gc(gc_type).await,
        }
    }
    
    /// Execute emergency garbage collection immediately
    async fn execute_emergency_gc(&self, gc_type: GcType) {
        let _permit = self.gc_semaphore.acquire().await;
        
        let start_time = chrono::Utc::now();
        let memory_before = self.get_memory_usage().await;
        
        info!("Executing emergency GC: type={:?}", gc_type);
        
        let result = self.perform_gc(gc_type.clone(), GcPriority::Critical).await;
        
        let end_time = chrono::Utc::now();
        let duration = (end_time - start_time).to_std().unwrap_or_default();
        let memory_after = self.get_memory_usage().await;
        
        self.record_gc_execution(GcExecution {
            start_time,
            end_time,
            duration,
            memory_freed_bytes: memory_before.saturating_sub(memory_after),
            memory_before_gc: memory_before,
            memory_after_gc: memory_after,
            gc_type,
            priority: GcPriority::Critical,
            success: result.is_ok(),
            error_message: result.as_ref().err().map(|e| e.to_string()),
        });
        
        if let Err(e) = result {
            error!("Emergency GC failed: {}", e);
        }
    }
    
    /// Execute high priority garbage collection
    async fn execute_high_priority_gc(&self, gc_type: GcType) {
        let _permit = self.gc_semaphore.acquire().await;
        
        let start_time = chrono::Utc::now();
        let memory_before = self.get_memory_usage().await;
        
        debug!("Executing high priority GC: type={:?}", gc_type);
        
        let result = self.perform_gc(gc_type.clone(), GcPriority::High).await;
        
        let end_time = chrono::Utc::now();
        let duration = (end_time - start_time).to_std().unwrap_or_default();
        let memory_after = self.get_memory_usage().await;
        
        self.record_gc_execution(GcExecution {
            start_time,
            end_time,
            duration,
            memory_freed_bytes: memory_before.saturating_sub(memory_after),
            memory_before_gc: memory_before,
            memory_after_gc: memory_after,
            gc_type,
            priority: GcPriority::High,
            success: result.is_ok(),
            error_message: result.err().map(|e| e.to_string()),
        });
    }
    
    /// Execute medium priority garbage collection
    async fn execute_medium_priority_gc(&self, gc_type: GcType) {
        // Try to acquire permit with timeout
        let permit_result = tokio::time::timeout(
            Duration::from_secs(5),
            self.gc_semaphore.acquire()
        ).await;
        
        match permit_result {
            Ok(permit) => {
                let start_time = chrono::Utc::now();
                let memory_before = self.get_memory_usage().await;
                
                debug!("Executing medium priority GC: type={:?}", gc_type);
                
                let result = self.perform_gc(gc_type.clone(), GcPriority::Medium).await;
                
                let end_time = chrono::Utc::now();
                let duration = (end_time - start_time).to_std().unwrap_or_default();
                let memory_after = self.get_memory_usage().await;
                
                self.record_gc_execution(GcExecution {
                    start_time,
                    end_time,
                    duration,
                    memory_freed_bytes: memory_before.saturating_sub(memory_after),
                    memory_before_gc: memory_before,
                    memory_after_gc: memory_after,
                    gc_type,
                    priority: GcPriority::Medium,
                    success: result.is_ok(),
                    error_message: result.err().map(|e| e.to_string()),
                });
                
                drop(permit);
            }
            Err(_) => {
                debug!("Medium priority GC timed out, skipping");
            }
        }
    }
    
    /// Execute low priority garbage collection
    async fn execute_low_priority_gc(&self, gc_type: GcType) {
        // Try to acquire permit with longer timeout
        let permit_result = tokio::time::timeout(
            Duration::from_secs(10),
            self.gc_semaphore.acquire()
        ).await;
        
        if let Ok(permit) = permit_result {
            let start_time = chrono::Utc::now();
            let memory_before = self.get_memory_usage().await;
            
            debug!("Executing low priority GC: type={:?}", gc_type);
            
            let result = self.perform_gc(gc_type.clone(), GcPriority::Low).await;
            
            let end_time = chrono::Utc::now();
            let duration = (end_time - start_time).to_std().unwrap_or_default();
            let memory_after = self.get_memory_usage().await;
            
            self.record_gc_execution(GcExecution {
                start_time,
                end_time,
                duration,
                memory_freed_bytes: memory_before.saturating_sub(memory_after),
                memory_before_gc: memory_before,
                memory_after_gc: memory_after,
                gc_type,
                priority: GcPriority::Low,
                success: result.is_ok(),
                error_message: result.err().map(|e| e.to_string()),
            });
            
            drop(permit);
        }
    }
    
    /// Execute background garbage collection
    async fn execute_background_gc(&self, gc_type: GcType) {
        // Only run if no other GC is running
        if self.gc_semaphore.available_permits() == self.config.max_concurrent_gc {
            let _permit = self.gc_semaphore.acquire().await;
            
            let start_time = chrono::Utc::now();
            let memory_before = self.get_memory_usage().await;
            
            debug!("Executing background GC: type={:?}", gc_type);
            
            let result = self.perform_gc(gc_type.clone(), GcPriority::Background).await;
            
            let end_time = chrono::Utc::now();
            let duration = (end_time - start_time).to_std().unwrap_or_default();
            let memory_after = self.get_memory_usage().await;
            
            self.record_gc_execution(GcExecution {
                start_time,
                end_time,
                duration,
                memory_freed_bytes: memory_before.saturating_sub(memory_after),
                memory_before_gc: memory_before,
                memory_after_gc: memory_after,
                gc_type,
                priority: GcPriority::Background,
                success: result.is_ok(),
                error_message: result.err().map(|e| e.to_string()),
            });
        }
    }
    
    /// Perform the actual garbage collection
    async fn perform_gc(&self, gc_type: GcType, _priority: GcPriority) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        {
            let mut state = self.state.write();
            state.is_gc_running = true;
        }
        
        let result = match gc_type {
            GcType::Full => self.perform_full_gc().await,
            GcType::Incremental { phase, total_phases } => self.perform_incremental_gc(phase, total_phases).await,
            GcType::Generational { generation } => self.perform_generational_gc(generation).await,
            GcType::Concurrent => self.perform_concurrent_gc().await,
            GcType::Emergency => self.perform_emergency_gc_internal().await,
        };
        
        {
            let mut state = self.state.write();
            state.is_gc_running = false;
            state.last_gc_time = Some(chrono::Utc::now());
            state.total_gc_executions += 1;
        }
        
        result
    }
    
    /// Perform full garbage collection
    async fn perform_full_gc(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Starting full garbage collection");
        
        // Force Rust's garbage collection (drop all unused objects)
        // In practice, this would involve more sophisticated memory management
        
        // Simulate GC work
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        debug!("Full garbage collection completed");
        Ok(())
    }
    
    /// Perform incremental garbage collection
    async fn perform_incremental_gc(&self, phase: u8, total_phases: u8) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Starting incremental GC: phase {}/{}", phase, total_phases);
        
        // Simulate incremental work
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        debug!("Incremental GC phase {} completed", phase);
        Ok(())
    }
    
    /// Perform generational garbage collection
    async fn perform_generational_gc(&self, generation: GcGeneration) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Starting generational GC: {:?}", generation);
        
        // Simulate generational work
        let duration = match generation {
            GcGeneration::Young => Duration::from_millis(10),
            GcGeneration::Mature => Duration::from_millis(30),
            GcGeneration::Old => Duration::from_millis(50),
        };
        
        tokio::time::sleep(duration).await;
        
        debug!("Generational GC completed: {:?}", generation);
        Ok(())
    }
    
    /// Perform concurrent garbage collection
    async fn perform_concurrent_gc(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Starting concurrent garbage collection");
        
        // Simulate concurrent work
        tokio::time::sleep(Duration::from_millis(40)).await;
        
        debug!("Concurrent garbage collection completed");
        Ok(())
    }
    
    /// Perform emergency garbage collection (internal implementation)
    async fn perform_emergency_gc_internal(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        warn!("Starting emergency garbage collection");
        
        // Aggressive cleanup
        tokio::time::sleep(Duration::from_millis(30)).await;
        
        warn!("Emergency garbage collection completed");
        Ok(())
    }
    
    /// Determine GC priority based on memory pressure
    fn determine_gc_priority(&self, memory_pressure: f64) -> GcPriority {
        let thresholds = &self.config.priority_config;
        
        if memory_pressure >= thresholds.critical_threshold {
            GcPriority::Critical
        } else if memory_pressure >= thresholds.high_threshold {
            GcPriority::High
        } else if memory_pressure >= thresholds.medium_threshold {
            GcPriority::Medium
        } else if memory_pressure >= thresholds.low_threshold {
            GcPriority::Low
        } else {
            GcPriority::Background
        }
    }
    
    /// Select optimal GC type based on conditions
    fn select_gc_type(&self, _memory_pressure: f64, priority: GcPriority) -> GcType {
        match (priority, self.config.enable_incremental_gc) {
            (GcPriority::Critical, _) => GcType::Emergency,
            (GcPriority::High, true) => GcType::Incremental { phase: 1, total_phases: 2 },
            (GcPriority::Medium, true) => GcType::Incremental { phase: 1, total_phases: 4 },
            (GcPriority::Low, true) => GcType::Generational { generation: GcGeneration::Young },
            (_, _) => GcType::Full,
        }
    }
    
    /// Check if GC should be triggered
    async fn should_trigger_gc(&self, _memory_pressure: f64) -> bool {
        let state = self.state.read();
        
        // Always trigger if critical pressure
        if _memory_pressure >= self.config.priority_config.critical_threshold {
            return true;
        }
        
        // Check if enough time has passed since last GC
        if let Some(last_gc) = state.last_gc_time {
            let time_since_gc = chrono::Utc::now().signed_duration_since(last_gc).to_std().unwrap_or_default();
            let min_interval = Duration::from_secs(self.config.min_interval_seconds);
            
            if time_since_gc < min_interval {
                return false;
            }
        }
        
        // Check adaptive interval
        let adaptive_interval = Duration::from_secs(
            (self.config.base_interval_seconds as f64 * state.adaptive_interval_multiplier) as u64
        );
        
        if let Some(last_gc) = state.last_gc_time {
            chrono::Utc::now().signed_duration_since(last_gc).to_std().unwrap_or_default() >= adaptive_interval
        } else {
            true // First GC
        }
    }
    
    /// Record GC execution for analytics
    fn record_gc_execution(&self, execution: GcExecution) {
        let duration = execution.duration;
        let mut history = self.history.write();
        history.push_back(execution);
        
        // Keep only recent history
        if history.len() > 1000 {
            history.pop_front();
        }
        
        // Update state
        let mut state = self.state.write();
        state.total_gc_time += duration;
    }
    
    /// Get current memory usage (placeholder implementation)
    async fn get_memory_usage(&self) -> usize {
        // In a real implementation, this would query actual memory usage
        // For now, return a simulated value
        100 * 1024 * 1024 // 100MB
    }
    
    /// Get GC scheduler statistics
    pub fn get_stats(&self) -> GcSchedulerStats {
        let state = self.state.read();
        let history = self.history.read();
        
        let recent_executions: Vec<_> = history.iter().rev().take(10).cloned().collect();
        let success_rate = if history.is_empty() {
            0.0
        } else {
            (history.iter().filter(|e| e.success).count() as f64 / history.len() as f64) * 100.0
        };
        
        let avg_duration = if history.is_empty() {
            Duration::ZERO
        } else {
            let total: Duration = history.iter().map(|e| e.duration).sum();
            total / history.len() as u32
        };
        
        GcSchedulerStats {
            total_executions: state.total_gc_executions,
            is_gc_running: state.is_gc_running,
            current_memory_pressure: state.current_memory_pressure,
            last_gc_time: state.last_gc_time,
            next_scheduled_gc: state.next_scheduled_gc,
            success_rate,
            average_duration: avg_duration,
            recent_executions,
            adaptive_interval_multiplier: state.adaptive_interval_multiplier,
        }
    }
    
    /// Get comprehensive GC report
    pub fn get_comprehensive_report(&self) -> serde_json::Value {
        let stats = self.get_stats();
        let history = self.history.read();
        
        // Analyze execution patterns
        let mut type_counts = HashMap::new();
        let mut priority_counts = HashMap::new();
                
        for execution in history.iter() {
            *type_counts.entry(format!("{:?}", execution.gc_type)).or_insert(0) += 1;
            *priority_counts.entry(format!("{:?}", execution.priority)).or_insert(0) += 1;
            let _total_memory_freed = execution.memory_freed_bytes;
        }
        
        serde_json::json!({
            "timestamp": Utc::now(),
            "config": format!("{:?}", self.config),
            "stats": stats,
            "adaptive_parameters": self.adaptive_engine.get_current_parameters(),
            "recent_performance": self.get_recent_performance(10),
            "recommendations": self.generate_gc_recommendations(&stats),
        })
    }
    
    /// Get recent performance data
    fn get_recent_performance(&self, count: usize) -> Vec<GcPerformance> {
        let history = self.adaptive_engine.performance_history.read();
        history.iter().rev().take(count).cloned().collect()
    }
    
    fn generate_gc_recommendations(&self, stats: &GcSchedulerStats) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if stats.success_rate < 90.0 {
            recommendations.push("Low GC success rate detected. Check for resource contention or timeouts.".to_string());
        }
        
        if stats.average_duration > Duration::from_millis(200) {
            recommendations.push("High GC pause times detected. Consider enabling incremental or concurrent GC.".to_string());
        }
        
        if stats.current_memory_pressure > 80.0 {
            recommendations.push("High memory pressure. Consider increasing GC frequency or optimizing memory usage.".to_string());
        }
        
        if stats.adaptive_interval_multiplier < 0.5 {
            recommendations.push("Adaptive GC running frequently. System may be under memory pressure.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("GC performance appears optimal. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcSchedulerStats {
    pub total_executions: u64,
    pub is_gc_running: bool,
    pub current_memory_pressure: f64,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_gc_time: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub next_scheduled_gc: Option<chrono::DateTime<chrono::Utc>>,
    pub success_rate: f64,
    pub average_duration: Duration,
    pub recent_executions: Vec<GcExecution>,
    pub adaptive_interval_multiplier: f64,
}

impl MemoryPressureMonitor {
    fn new(thresholds: GcPriorityConfig) -> Self {
        Self {
            gc_engine: Arc::new(AdaptiveGcEngine::new()),
            current_pressure: Arc::new(RwLock::new(0.0)),
            pressure_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            thresholds,
            monitoring_interval: Duration::from_secs(5),
        }
    }
    
    async fn get_current_pressure(&self) -> f64 {
        // Simulate memory pressure calculation
        // In a real implementation, this would query actual system memory
        let pressure = (rand::random::<f64>() * 100.0).max(0.0).min(100.0);
        
        // Update history
        {
            let mut history = self.pressure_history.write();
            history.push_back((Instant::now(), pressure));
            
            if history.len() > 100 {
                history.pop_front();
            }
        }
        
        *self.current_pressure.write() = pressure;
        pressure
    }
}

impl AdaptiveGcEngine {
    fn new() -> Self {
        Self {
            learning_rate: 0.1,
            performance_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            adaptive_params: Arc::new(RwLock::new(AdaptiveParameters::default())),
        }
    }
    
    async fn update_parameters(&self, scheduler: &GcScheduler) {
        let history = scheduler.history.read();
        
        if history.len() < 5 {
            return; // Not enough data for adaptation
        }
        
        let recent_performances: Vec<_> = history.iter().rev().take(10).collect();
        let avg_efficiency = recent_performances.iter()
            .map(|e| {
                if e.duration.as_millis() > 0 {
                    e.memory_freed_bytes as f64 / e.duration.as_millis() as f64
                } else {
                    0.0
                }
            })
            .sum::<f64>() / recent_performances.len() as f64;
        
        let mut params = self.adaptive_params.write();
        
        // Adjust interval based on efficiency
        if avg_efficiency > 1000.0 { // High efficiency
            params.interval_multiplier *= 1.1; // Run less frequently
        } else if avg_efficiency < 100.0 { // Low efficiency
            params.interval_multiplier *= 0.9; // Run more frequently
        }
        
        // Keep within reasonable bounds
        params.interval_multiplier = params.interval_multiplier.clamp(0.1, 3.0);
        
        // Update scheduler state
        let mut state = scheduler.state.write();
        state.adaptive_interval_multiplier = params.interval_multiplier;
    }
    
    fn get_current_parameters(&self) -> AdaptiveParameters {
        self.adaptive_params.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_gc_scheduler_creation() {
        let scheduler = GcScheduler::new();
        let stats = scheduler.get_stats();
        
        assert_eq!(stats.total_executions, 0);
        assert!(!stats.is_gc_running);
        assert!(stats.current_memory_pressure >= 0.0);
    }
    
    #[tokio::test]
    async fn test_priority_determination() {
        let scheduler = GcScheduler::new();
        
        assert_eq!(scheduler.determine_gc_priority(95.0), GcPriority::Critical);
        assert_eq!(scheduler.determine_gc_priority(85.0), GcPriority::High);
        assert_eq!(scheduler.determine_gc_priority(70.0), GcPriority::Medium);
        assert_eq!(scheduler.determine_gc_priority(50.0), GcPriority::Low);
        assert_eq!(scheduler.determine_gc_priority(20.0), GcPriority::Background);
    }
    
    #[tokio::test]
    async fn test_gc_type_selection() {
        let scheduler = GcScheduler::new();
        
        let critical_type = scheduler.select_gc_type(95.0, GcPriority::Critical);
        assert_eq!(critical_type, GcType::Emergency);
        
        let high_type = scheduler.select_gc_type(85.0, GcPriority::High);
        matches!(high_type, GcType::Incremental { .. });
    }
    
    #[tokio::test]
    async fn test_emergency_gc() {
        let scheduler = GcScheduler::new();
        
        scheduler.execute_emergency_gc(GcType::Emergency).await;
        
        let stats = scheduler.get_stats();
        assert_eq!(stats.total_executions, 1);
    }
    
    #[tokio::test]
    async fn test_comprehensive_report() {
        let scheduler = GcScheduler::new();
        
        // Trigger some GC activity
        scheduler.execute_emergency_gc(GcType::Emergency).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let report = scheduler.get_comprehensive_report();
        
        assert!(report.get("timestamp").is_some());
        assert!(report.get("config").is_some());
        assert!(report.get("stats").is_some());
        assert!(report.get("execution_patterns").is_some());
        assert!(report.get("recommendations").is_some());
    }
}
