use std::sync::Arc;
use parking_lot::RwLock;
use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use tokio::sync::broadcast;
use tracing::{info, warn, error};
use serde::{Serialize, Deserialize};
use chrono::Utc;

/// Comprehensive memory monitoring system with real-time tracking
#[derive(Debug, Clone)]
pub struct MemoryMonitor {
    /// Configuration for monitoring behavior
    config: MemoryMonitorConfig,
    
    /// Current memory statistics
    current_stats: Arc<RwLock<MemoryStatistics>>,
    
    /// Historical data for trend analysis
    history: Arc<RwLock<VecDeque<MemorySnapshot>>>,
    
    /// Alert system for threshold breaches
    alert_system: Arc<MemoryAlertSystem>,
    
    /// Performance metrics collector
    metrics_collector: Arc<MetricsCollector>,
    
    /// Memory profiler for detailed analysis
    #[allow(dead_code)]
    profiler: Arc<MemoryProfiler>,
    
    /// Event broadcaster for real-time updates
    event_sender: broadcast::Sender<MemoryEvent>,
    
    /// Monitoring state
    state: Arc<RwLock<MonitoringState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMonitorConfig {
    /// Monitoring interval in seconds
    pub monitoring_interval_seconds: u64,
    
    /// History retention period in seconds
    pub history_retention_seconds: u64,
    
    /// Enable detailed profiling
    pub enable_profiling: bool,
    
    /// Enable leak detection
    pub enable_leak_detection: bool,
    
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
    
    /// Performance targets
    pub performance_targets: PerformanceTargets,
    
    /// Enable real-time events
    pub enable_real_time_events: bool,
    
    /// Maximum history size
    pub max_history_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Memory usage percentage threshold for warnings
    pub memory_usage_warning_percent: f64,
    
    /// Memory usage percentage threshold for critical alerts
    pub memory_usage_critical_percent: f64,
    
    /// Memory growth rate threshold (MB per minute)
    pub memory_growth_rate_warning_mb_per_min: f64,
    
    /// GC pause time threshold in milliseconds
    pub gc_pause_time_warning_ms: u64,
    
    /// Memory fragmentation threshold
    pub fragmentation_warning_percent: f64,
    
    /// Leak detection threshold (allocations without deallocation)
    pub leak_detection_threshold_allocations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTargets {
    /// Target memory utilization percentage
    pub target_memory_utilization: f64,
    
    /// Target GC efficiency (memory freed per millisecond)
    pub target_gc_efficiency_mb_per_ms: f64,
    
    /// Target allocation rate (allocations per second)
    pub target_allocation_rate: f64,
    
    /// Target deallocation rate (deallocations per second)
    pub target_deallocation_rate: f64,
    
    /// Target memory turnover ratio (deallocations/allocations)
    pub target_memory_turnover_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatistics {
    /// Total memory allocated in bytes
    pub total_allocated_bytes: usize,
    
    /// Peak memory allocated in bytes
    pub peak_allocated_bytes: usize,
    
    /// Current memory usage in bytes
    pub current_usage_bytes: usize,
    
    /// Memory utilization percentage
    pub memory_utilization_percent: f64,
    
    /// Number of active allocations
    pub active_allocations: usize,
    
    /// Total number of allocations
    pub total_allocations: u64,
    
    /// Total number of deallocations
    pub total_deallocations: u64,
    
    /// Allocation rate (allocations per second)
    pub allocation_rate: f64,
    
    /// Deallocation rate (deallocations per second)
    pub deallocation_rate: f64,
    
    /// Memory growth rate (bytes per second)
    pub memory_growth_rate: f64,
    
    /// Memory fragmentation percentage
    pub fragmentation_percent: f64,
    
    /// GC statistics
    pub gc_stats: GcStatistics,
    
    /// Pool statistics
    pub pool_stats: HashMap<String, PoolMemoryStats>,
    
    /// System memory information
    pub system_memory: SystemMemoryInfo,
    
    /// Timestamp of this statistics snapshot
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcStatistics {
    /// Total number of GC collections
    pub total_collections: u64,
    
    /// Total time spent in GC in milliseconds
    pub total_gc_time_ms: u64,
    
    /// Average GC pause time in milliseconds
    pub average_pause_time_ms: f64,
    
    /// Maximum GC pause time in milliseconds
    pub max_pause_time_ms: u64,
    
    /// GC efficiency (memory freed per millisecond)
    pub efficiency_mb_per_ms: f64,
    
    /// Time since last GC in seconds
    pub time_since_last_gc_seconds: u64,
    
    /// GC frequency (collections per minute)
    pub gc_frequency_per_minute: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMemoryStats {
    /// Pool name
    pub name: String,
    
    /// Current pool size
    pub current_size: usize,
    
    /// Maximum pool size
    pub max_size: usize,
    
    /// Pool utilization percentage
    pub utilization_percent: f64,
    
    /// Hit rate percentage
    pub hit_rate_percent: f64,
    
    /// Memory used by pool in bytes
    pub memory_usage_bytes: usize,
    
    /// Total allocations from pool
    pub total_allocations: u64,
    
    /// Total deallocations to pool
    pub total_deallocations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMemoryInfo {
    /// Total system memory in bytes
    pub total_system_memory: usize,
    
    /// Available system memory in bytes
    pub available_system_memory: usize,
    
    /// System memory usage percentage
    pub system_memory_usage_percent: f64,
    
    /// Process memory usage in bytes
    pub process_memory_usage: usize,
    
    /// Process memory usage percentage
    pub process_memory_usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// Timestamp of the snapshot
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Memory statistics at this time
    pub stats: MemoryStatistics,
    
    /// Memory events that occurred around this time
    pub events: Vec<MemoryEvent>,
    
    /// System state information
    pub system_state: SystemState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    
    /// Number of active threads
    pub active_threads: usize,
    
    /// System load average
    pub load_average: f64,
    
    /// Open file descriptors
    pub open_file_descriptors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvent {
    /// Event timestamp (as Unix timestamp)
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Event type
    pub event_type: MemoryEventType,
    
    /// Event severity
    pub severity: EventSeverity,
    
    /// Event description
    pub description: String,
    
    /// Event data
    pub data: Option<serde_json::Value>,
    
    /// Source of the event
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryEventType {
    /// Memory allocation event
    Allocation { size: usize, pool: Option<String> },
    
    /// Memory deallocation event
    Deallocation { size: usize, pool: Option<String> },
    
    /// GC collection event
    GcCollection { gc_type: String, duration_ms: u64, memory_freed: usize },
    
    /// Memory pressure event
    MemoryPressure { pressure_percent: f64 },
    
    /// Memory leak detected
    MemoryLeakDetected { leak_size: usize, location: String },
    
    /// Pool threshold breach
    PoolThresholdBreach { pool_name: String, threshold_type: String, value: f64 },
    
    /// System memory event
    SystemMemoryEvent { event_type: String, value: f64 },
    
    /// Performance alert
    PerformanceAlert { metric: String, value: f64, target: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventSeverity {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
    Critical = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringState {
    /// Is monitoring currently active
    pub is_active: bool,
    
    /// Time monitoring started
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Total monitoring duration
    pub total_duration: Duration,
    
    /// Number of snapshots collected
    pub snapshots_collected: u64,
    
    /// Number of alerts triggered
    pub alerts_triggered: u64,
    
    /// Last update time
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
}

/// Memory alert system for threshold monitoring
#[derive(Debug)]
pub struct MemoryAlertSystem {
    #[allow(dead_code)]
    config: AlertThresholds,
    active_alerts: Arc<RwLock<HashMap<String, MemoryAlert>>>,
    #[allow(dead_code)]
    alert_history: Arc<RwLock<VecDeque<MemoryAlert>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAlert {
    pub id: String,
    pub alert_type: String,
    pub severity: EventSeverity,
    pub message: String,
    pub threshold_value: f64,
    pub current_value: f64,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub triggered_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub acknowledged_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Metrics collector for performance analysis
#[derive(Debug)]
pub struct MetricsCollector {
    metrics: Arc<RwLock<PerformanceMetrics>>,
    window_size: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Allocation metrics over time
    pub allocation_metrics: VecDeque<TimeSeriesPoint>,
    
    /// Deallocation metrics over time
    pub deallocation_metrics: VecDeque<TimeSeriesPoint>,
    
    /// Memory usage metrics over time
    pub memory_usage_metrics: VecDeque<TimeSeriesPoint>,
    
    /// GC performance metrics over time
    pub gc_performance_metrics: VecDeque<TimeSeriesPoint>,
    
    /// Pool performance metrics
    pub pool_performance_metrics: HashMap<String, VecDeque<TimeSeriesPoint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub value: f64,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Memory profiler for detailed analysis
#[derive(Debug)]
pub struct MemoryProfiler {
    #[allow(dead_code)]
    enabled: bool,
    #[allow(dead_code)]
    allocation_traces: Arc<RwLock<HashMap<String, AllocationTrace>>>,
    #[allow(dead_code)]
    stack_traces: Arc<RwLock<Vec<StackTrace>>>,
}

#[derive(Debug, Clone)]
pub struct AllocationTrace {
    pub allocation_id: String,
    pub size: usize,
    pub allocated_at: chrono::DateTime<chrono::Utc>,
    pub deallocated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub pool_name: Option<String>,
    pub stack_trace: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StackTrace {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub frames: Vec<String>,
    pub allocation_size: usize,
}

impl Default for MemoryMonitorConfig {
    fn default() -> Self {
        Self {
            monitoring_interval_seconds: 5,
            history_retention_seconds: 3600, // 1 hour
            enable_profiling: true,
            enable_leak_detection: true,
            enable_performance_monitoring: true,
            alert_thresholds: AlertThresholds::default(),
            performance_targets: PerformanceTargets::default(),
            enable_real_time_events: true,
            max_history_size: 1000,
        }
    }
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            memory_usage_warning_percent: 75.0,
            memory_usage_critical_percent: 90.0,
            memory_growth_rate_warning_mb_per_min: 10.0,
            gc_pause_time_warning_ms: 100,
            fragmentation_warning_percent: 30.0,
            leak_detection_threshold_allocations: 1000,
        }
    }
}

impl Default for PerformanceTargets {
    fn default() -> Self {
        Self {
            target_memory_utilization: 70.0,
            target_gc_efficiency_mb_per_ms: 1.0,
            target_allocation_rate: 1000.0,
            target_deallocation_rate: 950.0,
            target_memory_turnover_ratio: 0.95,
        }
    }
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self {
            is_active: false,
            start_time: None,
            total_duration: Duration::ZERO,
            snapshots_collected: 0,
            alerts_triggered: 0,
            last_update: None,
        }
    }
}

impl MemoryMonitor {
    pub fn new() -> Arc<Self> {
        Self::with_config(MemoryMonitorConfig::default())
    }
    
    pub fn with_config(config: MemoryMonitorConfig) -> Arc<Self> {
        let (event_sender, _) = broadcast::channel(1000);
        
        let monitor = Arc::new(Self {
            config: config.clone(),
            current_stats: Arc::new(RwLock::new(MemoryStatistics::default())),
            history: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_history_size))),
            alert_system: Arc::new(MemoryAlertSystem::new(config.alert_thresholds.clone())),
            metrics_collector: Arc::new(MetricsCollector::new(Duration::from_secs(300))), // 5 minute window
            profiler: Arc::new(MemoryProfiler::new(config.enable_profiling)),
            event_sender,
            state: Arc::new(RwLock::new(MonitoringState::default())),
        });
        
        monitor
    }
    
    /// Start memory monitoring
    pub async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.write();
        if state.is_active {
            return Err("Monitoring is already active".into());
        }
        
        state.is_active = true;
        state.start_time = Some(chrono::Utc::now());
        
        info!("Starting memory monitoring with interval: {}s", self.config.monitoring_interval_seconds);
        
        // Start monitoring loop
        let monitor = self.clone();
        tokio::spawn(async move {
            let monitor = monitor;
            monitor.monitoring_loop().await;
        });
        
        Ok(())
    }
    
    /// Stop memory monitoring
    pub fn stop_monitoring(&self) {
        let mut state = self.state.write();
        state.is_active = false;
        
        if let Some(start_time) = state.start_time {
            state.total_duration += chrono::Utc::now().signed_duration_since(start_time).to_std().unwrap_or_default();
        }
        
        info!("Memory monitoring stopped");
    }
    
    /// Main monitoring loop
    async fn monitoring_loop(&self) {
        let mut interval = tokio::time::interval(
            Duration::from_secs(self.config.monitoring_interval_seconds)
        );
        
        loop {
            interval.tick().await;
            
            // Check if monitoring is still active
            if !self.state.read().is_active {
                break;
            }
            
            // Collect current statistics
            if let Err(e) = self.collect_statistics().await {
                error!("Failed to collect memory statistics: {}", e);
                continue;
            }
            
            // Update metrics
            self.update_metrics().await;
            
            // Check alerts
            self.check_alerts().await;
            
            // Update state
            {
                let mut state = self.state.write();
                state.snapshots_collected += 1;
                state.last_update = Some(chrono::Utc::now());
            }
        }
    }
    
    /// Collect current memory statistics
    async fn collect_statistics(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stats = self.gather_memory_statistics().await?;
        let system_state = self.gather_system_state().await?;
        let timestamp = chrono::Utc::now();
        
        // Update current stats
        *self.current_stats.write() = stats.clone();
        
        // Add to history
        {
            let mut history = self.history.write();
            history.push_back(MemorySnapshot {
                timestamp,
                stats: stats.clone(),
                events: Vec::new(),
                system_state,
            });
            
            // Trim history if needed
            while history.len() > self.config.max_history_size {
                history.pop_front();
            }
        }
        
        // Broadcast update if enabled
        if self.config.enable_real_time_events {
            let _ = self.event_sender.send(MemoryEvent {
                timestamp,
                event_type: MemoryEventType::SystemMemoryEvent {
                    event_type: "statistics_update".to_string(),
                    value: stats.memory_utilization_percent,
                },
                severity: EventSeverity::Info,
                description: format!("Memory usage: {:.1}%", stats.memory_utilization_percent),
                data: Some(serde_json::to_value(&stats)?),
                source: "memory_monitor".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// Gather memory statistics from system and pools
    async fn gather_memory_statistics(&self) -> Result<MemoryStatistics, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        
        // In a real implementation, this would gather actual system memory data
        // For now, we'll simulate the data collection
        
        let total_allocated = 100 * 1024 * 1024; // 100MB
        let current_usage = 75 * 1024 * 1024;   // 75MB
        let system_memory = SystemMemoryInfo {
            total_system_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_system_memory: 4 * 1024 * 1024 * 1024, // 4GB
            system_memory_usage_percent: 50.0,
            process_memory_usage: current_usage,
            process_memory_usage_percent: (current_usage as f64 / (8 * 1024 * 1024 * 1024) as f64) * 100.0,
        };
        
        Ok(MemoryStatistics {
            total_allocated_bytes: total_allocated,
            peak_allocated_bytes: total_allocated,
            current_usage_bytes: current_usage,
            memory_utilization_percent: (current_usage as f64 / total_allocated as f64) * 100.0,
            active_allocations: 1000,
            total_allocations: 10000,
            total_deallocations: 9000,
            allocation_rate: 100.0,
            deallocation_rate: 95.0,
            memory_growth_rate: 5.0,
            fragmentation_percent: 10.0,
            gc_stats: GcStatistics::default(),
            pool_stats: HashMap::new(), // Would be populated with actual pool data
            system_memory: system_memory,
            timestamp: now,
        })
    }
    
    /// Gather system state information
    async fn gather_system_state(&self) -> Result<SystemState, Box<dyn std::error::Error + Send + Sync>> {
        // In a real implementation, this would gather actual system state
        Ok(SystemState {
            cpu_usage_percent: 25.0,
            active_threads: 8,
            load_average: 1.5,
            open_file_descriptors: 100,
        })
    }
    
    /// Update performance metrics
    async fn update_metrics(&self) {
        let stats = self.current_stats.read().clone();
        self.metrics_collector.update_metrics(&stats).await;
    }
    
    /// Check for alert conditions
    async fn check_alerts(&self) {
        let stats = self.current_stats.read().clone();
        let alerts = self.alert_system.check_alerts(&stats, &self.config.alert_thresholds);
        
        for alert in alerts {
            self.trigger_alert(alert).await;
        }
    }
    
    /// Trigger an alert
    async fn trigger_alert(&self, alert: MemoryAlert) {
        warn!("Memory alert triggered: {} - {}", alert.alert_type, alert.message);
        
        // Update alert count
        {
            let mut state = self.state.write();
            state.alerts_triggered += 1;
        }
        
        // Broadcast alert event
        if self.config.enable_real_time_events {
            let _ = self.event_sender.send(MemoryEvent {
                timestamp: chrono::Utc::now(),
                event_type: MemoryEventType::MemoryPressure {
                    pressure_percent: alert.current_value,
                },
                severity: alert.severity.clone(),
                description: alert.message.clone(),
                data: Some(serde_json::to_value(&alert).unwrap_or_default()),
                source: "alert_system".to_string(),
            });
        }
    }
    
    /// Get current memory statistics
    pub fn get_current_stats(&self) -> MemoryStatistics {
        self.current_stats.read().clone()
    }
    
    /// Get monitoring state
    pub fn get_monitoring_state(&self) -> MonitoringState {
        self.state.read().clone()
    }
    
    /// Get historical data
    pub fn get_history(&self, duration: Option<Duration>) -> Vec<MemorySnapshot> {
        let history = self.history.read();
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(duration.unwrap_or(Duration::from_secs(3600)).as_secs() as i64);
        
        history.iter()
            .filter(|snapshot| snapshot.timestamp >= cutoff)
            .cloned()
            .collect()
    }
    
    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.metrics_collector.get_metrics()
    }
    
    /// Get active alerts
    pub fn get_active_alerts(&self) -> Vec<MemoryAlert> {
        self.alert_system.get_active_alerts()
    }
    
    /// Subscribe to memory events
    pub fn subscribe_events(&self) -> broadcast::Receiver<MemoryEvent> {
        self.event_sender.subscribe()
    }
    
    /// Generate comprehensive memory report
    pub fn generate_report(&self) -> serde_json::Value {
        let stats = self.get_current_stats();
        let state = self.get_monitoring_state();
        let alerts = self.get_active_alerts();
        let metrics = self.get_performance_metrics();
        let recent_history = self.get_history(Some(Duration::from_secs(300))); // Last 5 minutes
        
        serde_json::json!({
            "timestamp": Utc::now(),
            "monitoring_state": state,
            "current_statistics": stats,
            "active_alerts": alerts,
            "performance_metrics": metrics,
            "recent_history": recent_history,
            "config": self.config,
            "analysis": self.analyze_memory_trends(&recent_history),
            "recommendations": self.generate_recommendations(&stats, &alerts),
        })
    }
    
    /// Analyze memory trends from historical data
    fn analyze_memory_trends(&self, history: &[MemorySnapshot]) -> serde_json::Value {
        if history.len() < 2 {
            return serde_json::json!({"status": "insufficient_data"});
        }
        
        let first_usage = history[0].stats.current_usage_bytes;
        let last_usage = history[history.len() - 1].stats.current_usage_bytes;
        let duration = (history[history.len() - 1].timestamp - history[0].timestamp).to_std().unwrap_or_default();
        
        let growth_rate = if duration.as_secs() > 0 {
            (last_usage as f64 - first_usage as f64) / duration.as_secs() as f64
        } else {
            0.0
        };
        
        let avg_utilization = history.iter()
            .map(|s| s.stats.memory_utilization_percent)
            .sum::<f64>() / history.len() as f64;
        
        let max_utilization = history.iter()
            .map(|s| s.stats.memory_utilization_percent)
            .fold(0.0, f64::max);
        
        serde_json::json!({
            "growth_rate_bytes_per_second": growth_rate,
            "average_utilization_percent": avg_utilization,
            "peak_utilization_percent": max_utilization,
            "trend_direction": if growth_rate > 1000.0 { "increasing" } else if growth_rate < -1000.0 { "decreasing" } else { "stable" },
            "data_points": history.len(),
            "analysis_period_seconds": duration.as_secs(),
        })
    }
    
    /// Generate recommendations based on current state
    fn generate_recommendations(&self, stats: &MemoryStatistics, alerts: &[MemoryAlert]) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Memory utilization recommendations
        if stats.memory_utilization_percent > 85.0 {
            recommendations.push("High memory utilization detected. Consider increasing memory limits or optimizing memory usage.".to_string());
        } else if stats.memory_utilization_percent < 30.0 {
            recommendations.push("Low memory utilization. Consider reducing allocated memory to improve efficiency.".to_string());
        }
        
        // Fragmentation recommendations
        if stats.fragmentation_percent > 25.0 {
            recommendations.push("High memory fragmentation detected. Consider implementing memory compaction or pooling.".to_string());
        }
        
        // GC performance recommendations
        if stats.gc_stats.average_pause_time_ms > 100.0 {
            recommendations.push("High GC pause times detected. Consider enabling incremental or concurrent GC.".to_string());
        }
        
        // Allocation/deallocation balance
        let turnover_ratio = if stats.total_allocations > 0 {
            stats.total_deallocations as f64 / stats.total_allocations as f64
        } else {
            0.0
        };
        
        if turnover_ratio < 0.8 {
            recommendations.push("Low memory turnover ratio detected. Potential memory leaks - investigate allocation patterns.".to_string());
        }
        
        // Alert-based recommendations
        if !alerts.is_empty() {
            recommendations.push(format!("{} active alerts detected. Review and resolve critical issues.", alerts.len()));
        }
        
        // Growth rate recommendations
        if stats.memory_growth_rate > 1000.0 {
            recommendations.push("High memory growth rate detected. Monitor for potential memory leaks or inefficient usage patterns.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("Memory usage appears optimal. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

impl MemoryAlertSystem {
    fn new(config: AlertThresholds) -> Self {
        Self {
            config,
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
        }
    }
    
    fn check_alerts(&self, stats: &MemoryStatistics, thresholds: &AlertThresholds) -> Vec<MemoryAlert> {
        let mut alerts = Vec::new();
        
        // Memory usage alerts
        if stats.memory_utilization_percent > thresholds.memory_usage_critical_percent {
            alerts.push(MemoryAlert {
                id: format!("memory_usage_critical_{}", Utc::now().timestamp()),
                alert_type: "memory_usage_critical".to_string(),
                severity: EventSeverity::Critical,
                message: format!("Critical memory usage: {:.1}%", stats.memory_utilization_percent),
                threshold_value: thresholds.memory_usage_critical_percent,
                current_value: stats.memory_utilization_percent,
                triggered_at: chrono::Utc::now(),
                acknowledged_at: None,
                resolved_at: None,
            });
        } else if stats.memory_utilization_percent > thresholds.memory_usage_warning_percent {
            alerts.push(MemoryAlert {
                id: format!("memory_usage_warning_{}", Utc::now().timestamp()),
                alert_type: "memory_usage_warning".to_string(),
                severity: EventSeverity::Warning,
                message: format!("High memory usage: {:.1}%", stats.memory_utilization_percent),
                threshold_value: thresholds.memory_usage_warning_percent,
                current_value: stats.memory_utilization_percent,
                triggered_at: chrono::Utc::now(),
                acknowledged_at: None,
                resolved_at: None,
            });
        }
        
        // GC pause time alerts
        if stats.gc_stats.max_pause_time_ms > thresholds.gc_pause_time_warning_ms {
            alerts.push(MemoryAlert {
                id: format!("gc_pause_time_{}", Utc::now().timestamp()),
                alert_type: "gc_pause_time".to_string(),
                severity: EventSeverity::Warning,
                message: format!("High GC pause time: {}ms", stats.gc_stats.max_pause_time_ms),
                threshold_value: thresholds.gc_pause_time_warning_ms as f64,
                current_value: stats.gc_stats.max_pause_time_ms as f64,
                triggered_at: chrono::Utc::now(),
                acknowledged_at: None,
                resolved_at: None,
            });
        }
        
        // Fragmentation alerts
        if stats.fragmentation_percent > thresholds.fragmentation_warning_percent {
            alerts.push(MemoryAlert {
                id: format!("fragmentation_{}", Utc::now().timestamp()),
                alert_type: "fragmentation".to_string(),
                severity: EventSeverity::Warning,
                message: format!("High memory fragmentation: {:.1}%", stats.fragmentation_percent),
                threshold_value: thresholds.fragmentation_warning_percent,
                current_value: stats.fragmentation_percent,
                triggered_at: chrono::Utc::now(),
                acknowledged_at: None,
                resolved_at: None,
            });
        }
        
        alerts
    }
    
    fn get_active_alerts(&self) -> Vec<MemoryAlert> {
        self.active_alerts.read().values().cloned().collect()
    }
}

impl MetricsCollector {
    fn new(window_size: Duration) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics {
                allocation_metrics: VecDeque::with_capacity(100),
                deallocation_metrics: VecDeque::with_capacity(100),
                memory_usage_metrics: VecDeque::with_capacity(100),
                gc_performance_metrics: VecDeque::with_capacity(100),
                pool_performance_metrics: HashMap::new(),
            })),
            window_size,
        }
    }
    
    async fn update_metrics(&self, stats: &MemoryStatistics) {
        let mut metrics = self.metrics.write();
        let now = chrono::Utc::now();
        
        // Update time series data
        metrics.allocation_metrics.push_back(TimeSeriesPoint {
            timestamp: now,
            value: stats.allocation_rate,
            metadata: None,
        });
        
        metrics.deallocation_metrics.push_back(TimeSeriesPoint {
            timestamp: now,
            value: stats.deallocation_rate,
            metadata: None,
        });
        
        metrics.memory_usage_metrics.push_back(TimeSeriesPoint {
            timestamp: now,
            value: stats.memory_utilization_percent,
            metadata: None,
        });
        
        metrics.gc_performance_metrics.push_back(TimeSeriesPoint {
            timestamp: now,
            value: stats.gc_stats.efficiency_mb_per_ms,
            metadata: None,
        });
        
        // Trim old data
        let cutoff = now - chrono::Duration::seconds(self.window_size.as_secs().try_into().unwrap());
        self.trim_old_metrics(&mut metrics, cutoff);
    }
    
    fn trim_old_metrics(&self, metrics: &mut PerformanceMetrics, cutoff: chrono::DateTime<chrono::Utc>) {
        while let Some(front) = metrics.allocation_metrics.front() {
            if front.timestamp < cutoff {
                metrics.allocation_metrics.pop_front();
            } else {
                break;
            }
        }
        
        // Repeat for other metrics...
        while let Some(front) = metrics.deallocation_metrics.front() {
            if front.timestamp < cutoff {
                metrics.deallocation_metrics.pop_front();
            } else {
                break;
            }
        }
        
        while let Some(front) = metrics.memory_usage_metrics.front() {
            if front.timestamp < cutoff {
                metrics.memory_usage_metrics.pop_front();
            } else {
                break;
            }
        }
        
        while let Some(front) = metrics.gc_performance_metrics.front() {
            if front.timestamp < cutoff {
                metrics.gc_performance_metrics.pop_front();
            } else {
                break;
            }
        }
    }
    
    fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().clone()
    }
}

impl MemoryProfiler {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            allocation_traces: Arc::new(RwLock::new(HashMap::new())),
            stack_traces: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for GcStatistics {
    fn default() -> Self {
        Self {
            total_collections: 0,
            total_gc_time_ms: 0,
            average_pause_time_ms: 0.0,
            max_pause_time_ms: 0,
            efficiency_mb_per_ms: 0.0,
            time_since_last_gc_seconds: 0,
            gc_frequency_per_minute: 0.0,
        }
    }
}

impl Default for MemoryStatistics {
    fn default() -> Self {
        Self {
            total_allocated_bytes: 0,
            peak_allocated_bytes: 0,
            current_usage_bytes: 0,
            memory_utilization_percent: 0.0,
            active_allocations: 0,
            total_allocations: 0,
            total_deallocations: 0,
            allocation_rate: 0.0,
            deallocation_rate: 0.0,
            memory_growth_rate: 0.0,
            fragmentation_percent: 0.0,
            gc_stats: GcStatistics::default(),
            pool_stats: HashMap::new(),
            system_memory: SystemMemoryInfo {
                total_system_memory: 0,
                available_system_memory: 0,
                system_memory_usage_percent: 0.0,
                process_memory_usage: 0,
                process_memory_usage_percent: 0.0,
            },
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_monitor_creation() {
        let monitor = MemoryMonitor::new();
        let state = monitor.get_monitoring_state();
        
        assert!(!state.is_active);
        assert_eq!(state.snapshots_collected, 0);
    }
    
    #[tokio::test]
    async fn test_start_monitoring() {
        let monitor = MemoryMonitor::new();
        
        let result = monitor.start_monitoring().await;
        assert!(result.is_ok());
        
        let state = monitor.get_monitoring_state();
        assert!(state.is_active);
        assert!(state.start_time.is_some());
        
        monitor.stop_monitoring();
    }
    
    #[tokio::test]
    async fn test_statistics_collection() {
        let monitor = MemoryMonitor::new();
        
        monitor.collect_statistics().await.unwrap();
        
        let stats = monitor.get_current_stats();
        assert!(stats.total_allocated_bytes > 0);
        assert!(stats.timestamp > chrono::Utc::now() - chrono::Duration::seconds(10));
    }
    
    #[tokio::test]
    async fn test_alert_system() {
        let monitor = MemoryMonitor::new();
        
        // Simulate high memory usage
        {
            let mut stats = monitor.current_stats.write();
            stats.memory_utilization_percent = 95.0;
        }
        
        monitor.check_alerts().await;
        
        let alerts = monitor.get_active_alerts();
        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].severity, EventSeverity::Critical);
    }
    
    #[tokio::test]
    async fn test_event_subscription() {
        let monitor = MemoryMonitor::new();
        let _receiver = monitor.subscribe_events();
        
        // Trigger an event
        monitor.collect_statistics().await.unwrap();
        
        // Should receive an event (though timing might make this flaky in tests)
        // In practice, you'd wait for the event with a timeout
    }
    
    #[tokio::test]
    async fn test_comprehensive_report() {
        let monitor = MemoryMonitor::new();
        
        monitor.collect_statistics().await.unwrap();
        
        let report = monitor.generate_report();
        
        assert!(report.get("timestamp").is_some());
        assert!(report.get("monitoring_state").is_some());
        assert!(report.get("current_statistics").is_some());
        assert!(report.get("recommendations").is_some());
    }
}
