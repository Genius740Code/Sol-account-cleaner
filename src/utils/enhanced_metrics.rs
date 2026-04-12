use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::utils::metrics::{MetricsCollector, MetricsConfig};

/// Enhanced metrics collector with detailed performance and security monitoring
pub struct EnhancedMetricsCollector {
    base_collector: MetricsCollector,
    detailed_metrics: Arc<RwLock<DetailedMetrics>>,
    performance_tracker: Arc<RwLock<PerformanceTracker>>,
    security_monitor: Arc<RwLock<SecurityMonitor>>,
    config: EnhancedMetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedMetrics {
    /// Cache performance metrics
    pub cache_metrics: CacheMetrics,
    /// Connection pool metrics
    pub connection_pool_metrics: ConnectionPoolMetrics,
    /// Memory pool efficiency metrics
    pub memory_pool_metrics: MemoryPoolMetrics,
    /// Encryption performance metrics
    pub encryption_metrics: EncryptionMetrics,
    /// Audit logging metrics
    pub audit_log_metrics: AuditLogMetrics,
    /// Protocol optimization metrics
    pub protocol_metrics: ProtocolMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetrics {
    pub l1_cache_hit_rate: f64,
    pub l2_cache_hit_rate: f64,
    pub l3_cache_hit_rate: f64,
    pub overall_hit_rate: f64,
    pub cache_memory_usage_mb: f64,
    pub cache_evictions_total: u64,
    pub cache_operations_per_second: f64,
    pub average_cache_lookup_time_us: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolMetrics {
    pub active_connections: u32,
    pub idle_connections: u32,
    pub connection_reuse_rate: f64,
    pub average_connection_lifetime_ms: f64,
    pub connection_creation_rate: f64,
    pub connection_errors: u64,
    pub connection_utilization: f64,
    pub endpoint_health_scores: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPoolMetrics {
    pub pool_efficiency: f64,
    pub memory_saved_bytes: u64,
    pub allocation_rate: f64,
    pub deallocation_rate: f64,
    pub pool_hit_rate: f64,
    pub average_allocation_time_ns: f64,
    pub fragmentation_ratio: f64,
    pub gc_pressure: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetrics {
    pub encryption_ops_per_second: f64,
    pub decryption_ops_per_second: f64,
    pub average_encryption_time_us: f64,
    pub average_decryption_time_us: f64,
    pub hardware_acceleration_rate: f64,
    pub key_rotation_count: u64,
    pub encryption_errors: u64,
    pub throughput_mbps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogMetrics {
    pub log_entries_per_second: f64,
    pub average_log_processing_time_us: f64,
    pub batch_processing_efficiency: f64,
    pub dropped_entries_rate: f64,
    pub audit_trail_size_mb: f64,
    pub signature_verification_rate: f64,
    pub log_compression_ratio: f64,
    pub failed_verifications: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMetrics {
    pub http2_multiplexing_efficiency: f64,
    pub compression_ratio: f64,
    pub request_optimization_rate: f64,
    pub protocol_upgrade_success_rate: f64,
    pub average_request_size_bytes: f64,
    pub bandwidth_savings_percent: f64,
    pub connection_reuse_efficiency: f64,
    pub protocol_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTracker {
    pub current_throughput: f64,
    pub peak_throughput: f64,
    pub average_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub error_rate: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub disk_io_rate_mb_s: f64,
    pub network_io_rate_mb_s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMonitor {
    pub authentication_failures: u64,
    pub authorization_failures: u64,
    pub input_validation_failures: u64,
    pub rate_limit_violations: u64,
    pub suspicious_activities: u64,
    pub blocked_ips: HashMap<String, u64>,
    pub attack_patterns_detected: HashMap<String, u64>,
    pub security_score: f64,
    pub last_security_event: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedMetricsConfig {
    pub base_config: MetricsConfig,
    pub detailed_metrics_interval: Duration,
    pub performance_tracking_enabled: bool,
    pub security_monitoring_enabled: bool,
    pub alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub max_error_rate: f64,
    pub max_response_time_ms: f64,
    pub min_cache_hit_rate: f64,
    pub max_cpu_usage_percent: f64,
    pub max_memory_usage_mb: f64,
    pub min_security_score: f64,
    pub max_authentication_failures: u64,
}

impl Default for EnhancedMetricsConfig {
    fn default() -> Self {
        Self {
            base_config: MetricsConfig::default(),
            detailed_metrics_interval: Duration::from_secs(30),
            performance_tracking_enabled: true,
            security_monitoring_enabled: true,
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            max_error_rate: 0.05, // 5%
            max_response_time_ms: 1000.0,
            min_cache_hit_rate: 0.70, // 70%
            max_cpu_usage_percent: 80.0,
            max_memory_usage_mb: 1024.0, // 1GB
            min_security_score: 0.80, // 80%
            max_authentication_failures: 10,
        }
    }
}

impl EnhancedMetricsCollector {
    pub fn new(config: EnhancedMetricsConfig) -> Self {
        Self {
            base_collector: MetricsCollector::new(config.base_config.clone()),
            detailed_metrics: Arc::new(RwLock::new(DetailedMetrics::default())),
            performance_tracker: Arc::new(RwLock::new(PerformanceTracker::default())),
            security_monitor: Arc::new(RwLock::new(SecurityMonitor::default())),
            config,
        }
    }

    /// Record cache performance metrics
    pub async fn record_cache_metrics(&self, metrics: CacheMetrics) {
        let mut detailed = self.detailed_metrics.write().await;
        detailed.cache_metrics = metrics.clone();
        
        // Update base metrics
        self.base_collector.set_gauge("cache_hit_rate", metrics.overall_hit_rate, None).await;
        self.base_collector.set_gauge("cache_memory_usage_mb", metrics.cache_memory_usage_mb, None).await;
        self.base_collector.increment_counter("cache_operations_total", None).await;
    }

    /// Record connection pool metrics
    pub async fn record_connection_pool_metrics(&self, metrics: ConnectionPoolMetrics) {
        let mut detailed = self.detailed_metrics.write().await;
        detailed.connection_pool_metrics = metrics.clone();
        
        // Update base metrics
        self.base_collector.set_gauge("active_connections", metrics.active_connections as f64, None).await;
        self.base_collector.set_gauge("connection_utilization", metrics.connection_utilization, None).await;
        self.base_collector.increment_counter("connection_errors_total", None).await;
    }

    /// Record memory pool metrics
    pub async fn record_memory_pool_metrics(&self, metrics: MemoryPoolMetrics) {
        let mut detailed = self.detailed_metrics.write().await;
        detailed.memory_pool_metrics = metrics.clone();
        
        // Update base metrics
        self.base_collector.set_gauge("memory_pool_efficiency", metrics.pool_efficiency, None).await;
        self.base_collector.set_gauge("memory_saved_bytes", metrics.memory_saved_bytes as f64, None).await;
        self.base_collector.set_gauge("gc_pressure", metrics.gc_pressure, None).await;
    }

    /// Record encryption metrics
    pub async fn record_encryption_metrics(&self, metrics: EncryptionMetrics) {
        let mut detailed = self.detailed_metrics.write().await;
        detailed.encryption_metrics = metrics.clone();
        
        // Update base metrics
        self.base_collector.set_gauge("encryption_ops_per_second", metrics.encryption_ops_per_second, None).await;
        self.base_collector.set_gauge("hardware_acceleration_rate", metrics.hardware_acceleration_rate, None).await;
        self.base_collector.increment_counter("encryption_operations_total", None).await;
    }

    /// Record audit log metrics
    pub async fn record_audit_log_metrics(&self, metrics: AuditLogMetrics) {
        let mut detailed = self.detailed_metrics.write().await;
        detailed.audit_log_metrics = metrics.clone();
        
        // Update base metrics
        self.base_collector.set_gauge("log_entries_per_second", metrics.log_entries_per_second, None).await;
        self.base_collector.set_gauge("audit_trail_size_mb", metrics.audit_trail_size_mb, None).await;
        self.base_collector.increment_counter("audit_log_entries_total", None).await;
    }

    /// Record protocol metrics
    pub async fn record_protocol_metrics(&self, metrics: ProtocolMetrics) {
        let mut detailed = self.detailed_metrics.write().await;
        detailed.protocol_metrics = metrics.clone();
        
        // Update base metrics
        self.base_collector.set_gauge("http2_multiplexing_efficiency", metrics.http2_multiplexing_efficiency, None).await;
        self.base_collector.set_gauge("compression_ratio", metrics.compression_ratio, None).await;
        self.base_collector.set_gauge("bandwidth_savings_percent", metrics.bandwidth_savings_percent, None).await;
    }

    /// Update performance tracker
    pub async fn update_performance_metrics(&self, response_time_ms: f64, success: bool) {
        if !self.config.performance_tracking_enabled {
            return;
        }

        let mut tracker = self.performance_tracker.write().await;
        
        // Update response time metrics
        tracker.average_response_time_ms = (tracker.average_response_time_ms * 0.9) + (response_time_ms * 0.1);
        
        // Update error rate (exponential moving average)
        let current_error = if success { 0.0 } else { 1.0 };
        tracker.error_rate = (tracker.error_rate * 0.9) + (current_error * 0.1);
        
        // Update throughput (simplified calculation)
        tracker.current_throughput = 1000.0 / tracker.average_response_time_ms;
        if tracker.current_throughput > tracker.peak_throughput {
            tracker.peak_throughput = tracker.current_throughput;
        }
        
        // Update base metrics
        self.base_collector.record_histogram("response_time_ms", response_time_ms, None).await;
        self.base_collector.set_gauge("current_throughput", tracker.current_throughput, None).await;
        self.base_collector.set_gauge("error_rate", tracker.error_rate, None).await;
    }

    /// Record security event
    pub async fn record_security_event(&self, event_type: &str, details: &str) {
        if !self.config.security_monitoring_enabled {
            return;
        }

        let mut monitor = self.security_monitor.write().await;
        monitor.last_security_event = chrono::Utc::now();
        
        match event_type {
            "authentication_failure" => monitor.authentication_failures += 1,
            "authorization_failure" => monitor.authorization_failures += 1,
            "input_validation_failure" => monitor.input_validation_failures += 1,
            "rate_limit_violation" => monitor.rate_limit_violations += 1,
            "suspicious_activity" => {
                monitor.suspicious_activities += 1;
                if let Some((pattern, _)) = details.split_once(':') {
                    *monitor.attack_patterns_detected.entry(pattern.to_string()).or_insert(0) += 1;
                }
            }
            _ => {}
        }
        
        // Update security score
        let security_score = self.calculate_security_score(&monitor);
        
        // Update base metrics
        self.base_collector.increment_counter(&format!("security_events_{}", event_type), None).await;
        self.base_collector.set_gauge("security_score", security_score, None).await;
    }

    /// Calculate security score based on various factors
    fn calculate_security_score(&self, monitor: &SecurityMonitor) -> f64 {
        let auth_score = if monitor.authentication_failures < self.config.alert_thresholds.max_authentication_failures {
            1.0
        } else {
            0.5
        };
        
        let validation_score = if monitor.input_validation_failures < 10 {
            1.0
        } else {
            0.5
        };
        
        let activity_score = if monitor.suspicious_activities < 10 {
            1.0
        } else {
            0.3
        };
        
        let rate_limit_score = if monitor.rate_limit_violations < 5 {
            1.0
        } else {
            0.5
        };
        
        (auth_score + validation_score + activity_score + rate_limit_score) / 4.0
    }

    /// Get comprehensive metrics snapshot
    pub async fn get_comprehensive_snapshot(&self) -> ComprehensiveMetricsSnapshot {
        let base_snapshot = self.base_collector.get_snapshot().await;
        let detailed = self.detailed_metrics.read().await.clone();
        let performance = self.performance_tracker.read().await.clone();
        let security = self.security_monitor.read().await.clone();
        
        ComprehensiveMetricsSnapshot {
            base_metrics: base_snapshot,
            detailed_metrics: detailed,
            performance_tracker: performance,
            security_monitor: security,
            timestamp: chrono::Utc::now(),
            alerts: self.check_alerts().await,
        }
    }

    /// Check for alert conditions
    async fn check_alerts(&self) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let performance = self.performance_tracker.read().await;
        let security = self.security_monitor.read().await;
        let thresholds = &self.config.alert_thresholds;
        
        // Performance alerts
        if performance.error_rate > thresholds.max_error_rate {
            alerts.push(Alert {
                level: AlertLevel::Warning,
                category: AlertCategory::Performance,
                message: format!("High error rate detected: {:.2}%", performance.error_rate * 100.0),
                timestamp: chrono::Utc::now(),
            });
        }
        
        if performance.average_response_time_ms > thresholds.max_response_time_ms {
            alerts.push(Alert {
                level: AlertLevel::Warning,
                category: AlertCategory::Performance,
                message: format!("High response time detected: {:.2}ms", performance.average_response_time_ms),
                timestamp: chrono::Utc::now(),
            });
        }
        
        // Security alerts
        if security.security_score < thresholds.min_security_score {
            alerts.push(Alert {
                level: AlertLevel::Critical,
                category: AlertCategory::Security,
                message: format!("Low security score: {:.2}", security.security_score),
                timestamp: chrono::Utc::now(),
            });
        }
        
        if security.authentication_failures > thresholds.max_authentication_failures {
            alerts.push(Alert {
                level: AlertLevel::Critical,
                category: AlertCategory::Security,
                message: format!("High authentication failure count: {}", security.authentication_failures),
                timestamp: chrono::Utc::now(),
            });
        }
        
        alerts
    }

    /// Get metrics in Prometheus format
    pub async fn get_prometheus_metrics(&self) -> String {
        let snapshot = self.get_comprehensive_snapshot().await;
        let mut output = String::new();
        
        // Base metrics
        for counter in &snapshot.base_metrics.counters {
            output.push_str(&format!("# HELP {} {}\n", counter.name, counter.name));
            output.push_str(&format!("# TYPE {} counter\n", counter.name));
            output.push_str(&format!("{} {}\n", counter.name, counter.value));
        }
        
        for gauge in &snapshot.base_metrics.gauges {
            output.push_str(&format!("# HELP {} {}\n", gauge.name, gauge.name));
            output.push_str(&format!("# TYPE {} gauge\n", gauge.name));
            output.push_str(&format!("{} {}\n", gauge.name, gauge.value));
        }
        
        // Detailed metrics
        output.push_str("# HELP cache_hit_rate Cache hit rate\n");
        output.push_str("# TYPE cache_hit_rate gauge\n");
        output.push_str(&format!("cache_hit_rate {}\n", snapshot.detailed_metrics.cache_metrics.overall_hit_rate));
        
        output.push_str("# HELP security_score Security score\n");
        output.push_str("# TYPE security_score gauge\n");
        output.push_str(&format!("security_score {}\n", snapshot.security_monitor.security_score));
        
        output
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveMetricsSnapshot {
    pub base_metrics: crate::utils::metrics::MetricsSnapshot,
    pub detailed_metrics: DetailedMetrics,
    pub performance_tracker: PerformanceTracker,
    pub security_monitor: SecurityMonitor,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub level: AlertLevel,
    pub category: AlertCategory,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCategory {
    Performance,
    Security,
    Resource,
    Network,
}

// Default implementations
impl Default for DetailedMetrics {
    fn default() -> Self {
        Self {
            cache_metrics: CacheMetrics::default(),
            connection_pool_metrics: ConnectionPoolMetrics::default(),
            memory_pool_metrics: MemoryPoolMetrics::default(),
            encryption_metrics: EncryptionMetrics::default(),
            audit_log_metrics: AuditLogMetrics::default(),
            protocol_metrics: ProtocolMetrics::default(),
        }
    }
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self {
            l1_cache_hit_rate: 0.0,
            l2_cache_hit_rate: 0.0,
            l3_cache_hit_rate: 0.0,
            overall_hit_rate: 0.0,
            cache_memory_usage_mb: 0.0,
            cache_evictions_total: 0,
            cache_operations_per_second: 0.0,
            average_cache_lookup_time_us: 0.0,
        }
    }
}

impl Default for ConnectionPoolMetrics {
    fn default() -> Self {
        Self {
            active_connections: 0,
            idle_connections: 0,
            connection_reuse_rate: 0.0,
            average_connection_lifetime_ms: 0.0,
            connection_creation_rate: 0.0,
            connection_errors: 0,
            connection_utilization: 0.0,
            endpoint_health_scores: HashMap::new(),
        }
    }
}

impl Default for MemoryPoolMetrics {
    fn default() -> Self {
        Self {
            pool_efficiency: 0.0,
            memory_saved_bytes: 0,
            allocation_rate: 0.0,
            deallocation_rate: 0.0,
            pool_hit_rate: 0.0,
            average_allocation_time_ns: 0.0,
            fragmentation_ratio: 0.0,
            gc_pressure: 0.0,
        }
    }
}

impl Default for EncryptionMetrics {
    fn default() -> Self {
        Self {
            encryption_ops_per_second: 0.0,
            decryption_ops_per_second: 0.0,
            average_encryption_time_us: 0.0,
            average_decryption_time_us: 0.0,
            hardware_acceleration_rate: 0.0,
            key_rotation_count: 0,
            encryption_errors: 0,
            throughput_mbps: 0.0,
        }
    }
}

impl Default for AuditLogMetrics {
    fn default() -> Self {
        Self {
            log_entries_per_second: 0.0,
            average_log_processing_time_us: 0.0,
            batch_processing_efficiency: 0.0,
            dropped_entries_rate: 0.0,
            audit_trail_size_mb: 0.0,
            signature_verification_rate: 0.0,
            log_compression_ratio: 0.0,
            failed_verifications: 0,
        }
    }
}

impl Default for ProtocolMetrics {
    fn default() -> Self {
        Self {
            http2_multiplexing_efficiency: 0.0,
            compression_ratio: 0.0,
            request_optimization_rate: 0.0,
            protocol_upgrade_success_rate: 0.0,
            average_request_size_bytes: 0.0,
            bandwidth_savings_percent: 0.0,
            connection_reuse_efficiency: 0.0,
            protocol_errors: 0,
        }
    }
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self {
            current_throughput: 0.0,
            peak_throughput: 0.0,
            average_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            error_rate: 0.0,
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0.0,
            disk_io_rate_mb_s: 0.0,
            network_io_rate_mb_s: 0.0,
        }
    }
}

impl Default for SecurityMonitor {
    fn default() -> Self {
        Self {
            authentication_failures: 0,
            authorization_failures: 0,
            input_validation_failures: 0,
            rate_limit_violations: 0,
            suspicious_activities: 0,
            blocked_ips: HashMap::new(),
            attack_patterns_detected: HashMap::new(),
            security_score: 1.0,
            last_security_event: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_metrics_creation() {
        let config = EnhancedMetricsConfig::default();
        let collector = EnhancedMetricsCollector::new(config);
        
        // Test that all components are initialized
        let snapshot = collector.get_comprehensive_snapshot().await;
        assert!(snapshot.timestamp > chrono::Utc::now() - Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_cache_metrics_recording() {
        let config = EnhancedMetricsConfig::default();
        let collector = EnhancedMetricsCollector::new(config);
        
        let cache_metrics = CacheMetrics {
            overall_hit_rate: 0.85,
            cache_memory_usage_mb: 150.0,
            ..Default::default()
        };
        
        collector.record_cache_metrics(cache_metrics).await;
        
        let snapshot = collector.get_comprehensive_snapshot().await;
        assert_eq!(snapshot.detailed_metrics.cache_metrics.overall_hit_rate, 0.85);
        assert_eq!(snapshot.detailed_metrics.cache_metrics.cache_memory_usage_mb, 150.0);
    }

    #[tokio::test]
    async fn test_security_event_recording() {
        let config = EnhancedMetricsConfig::default();
        let collector = EnhancedMetricsCollector::new(config);
        
        collector.record_security_event("authentication_failure", "invalid_api_key").await;
        collector.record_security_event("suspicious_activity", "sql_injection:DROP TABLE").await;
        
        let snapshot = collector.get_comprehensive_snapshot().await;
        assert_eq!(snapshot.security_monitor.authentication_failures, 1);
        assert_eq!(snapshot.security_monitor.suspicious_activities, 1);
        assert!(snapshot.security_monitor.attack_patterns_detected.contains_key("sql_injection"));
    }

    #[tokio::test]
    async fn test_performance_tracking() {
        let config = EnhancedMetricsConfig::default();
        let collector = EnhancedMetricsCollector::new(config);
        
        collector.update_performance_metrics(100.0, true).await;
        collector.update_performance_metrics(200.0, false).await;
        
        let snapshot = collector.get_comprehensive_snapshot().await;
        assert!(snapshot.performance_tracker.average_response_time_ms > 0.0);
        assert!(snapshot.performance_tracker.error_rate > 0.0);
        assert!(snapshot.performance_tracker.current_throughput > 0.0);
    }

    #[tokio::test]
    async fn test_alert_generation() {
        let config = EnhancedMetricsConfig::default();
        let collector = EnhancedMetricsCollector::new(config);
        
        // Simulate high error rate
        for _ in 0..100 {
            collector.update_performance_metrics(100.0, false).await;
        }
        
        let snapshot = collector.get_comprehensive_snapshot().await;
        assert!(!snapshot.alerts.is_empty());
        
        // Check that we get a performance alert for high error rate
        let performance_alerts: Vec<_> = snapshot.alerts.iter()
            .filter(|alert| matches!(alert.category, AlertCategory::Performance))
            .collect();
        assert!(!performance_alerts.is_empty());
    }

    #[tokio::test]
    async fn test_prometheus_format() {
        let config = EnhancedMetricsConfig::default();
        let collector = EnhancedMetricsCollector::new(config);
        
        collector.base_collector.increment_counter("test_counter", None).await;
        collector.base_collector.set_gauge("test_gauge", 42.0, None).await;
        
        let prometheus_output = collector.get_prometheus_metrics().await;
        assert!(prometheus_output.contains("# HELP test_counter"));
        assert!(prometheus_output.contains("# TYPE test_counter counter"));
        assert!(prometheus_output.contains("test_counter 1"));
        assert!(prometheus_output.contains("# HELP test_gauge"));
        assert!(prometheus_output.contains("# TYPE test_gauge gauge"));
        assert!(prometheus_output.contains("test_gauge 42"));
    }
}
