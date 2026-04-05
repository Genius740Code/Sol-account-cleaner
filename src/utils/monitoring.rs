use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use tracing::{info, warn};
use metrics::{counter, histogram, gauge, describe_counter, describe_histogram, describe_gauge};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_total_mb: u64,
    pub disk_usage_mb: u64,
    pub disk_total_mb: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub active_connections: u64,
    pub goroutines: u64,
    pub gc_pause_ms: u64,
    pub heap_size_mb: u64,
    pub heap_objects: u64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0,
            memory_total_mb: 0,
            disk_usage_mb: 0,
            disk_total_mb: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            active_connections: 0,
            goroutines: 0,
            gc_pause_ms: 0,
            heap_size_mb: 0,
            heap_objects: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationMetrics {
    pub timestamp: DateTime<Utc>,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
    pub requests_per_second: f64,
    pub active_users: u64,
    pub total_wallets_scanned: u64,
    pub total_sol_recovered: f64,
    pub cache_hit_rate: f64,
    pub database_connections: u32,
    pub rpc_connections: u32,
    pub queue_size: u64,
}

impl Default for ApplicationMetrics {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_response_time_ms: 0.0,
            requests_per_second: 0.0,
            active_users: 0,
            total_wallets_scanned: 0,
            total_sol_recovered: 0.0,
            cache_hit_rate: 0.0,
            database_connections: 0,
            rpc_connections: 0,
            queue_size: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessMetrics {
    pub timestamp: DateTime<Utc>,
    pub daily_active_users: u64,
    pub weekly_active_users: u64,
    pub monthly_active_users: u64,
    pub total_revenue_sol: f64,
    pub average_revenue_per_user_sol: f64,
    pub conversion_rate: f64,
    pub user_retention_rate: f64,
    pub error_rate: f64,
    pub uptime_percentage: f64,
}

impl Default for BusinessMetrics {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            daily_active_users: 0,
            weekly_active_users: 0,
            monthly_active_users: 0,
            total_revenue_sol: 0.0,
            average_revenue_per_user_sol: 0.0,
            conversion_rate: 0.0,
            user_retention_rate: 0.0,
            error_rate: 0.0,
            uptime_percentage: 100.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsConfig {
    pub collection_interval_seconds: u64,
    pub retention_hours: u64,
    pub enable_system_metrics: bool,
    pub enable_application_metrics: bool,
    pub enable_business_metrics: bool,
    pub enable_prometheus_export: bool,
    pub prometheus_port: u16,
    pub alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub error_rate_percent: f64,
    pub response_time_ms: f64,
    pub queue_size: u64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 80.0,
            memory_usage_percent: 85.0,
            disk_usage_percent: 90.0,
            error_rate_percent: 5.0,
            response_time_ms: 5000.0,
            queue_size: 1000,
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collection_interval_seconds: 30,
            retention_hours: 24,
            enable_system_metrics: true,
            enable_application_metrics: true,
            enable_business_metrics: true,
            enable_prometheus_export: true,
            prometheus_port: 9091,
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

pub struct MonitoringService {
    config: MetricsConfig,
    system_metrics: Arc<RwLock<SystemMetrics>>,
    application_metrics: Arc<RwLock<ApplicationMetrics>>,
    business_metrics: Arc<RwLock<BusinessMetrics>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    metrics_history: Arc<RwLock<HashMap<String, Vec<f64>>>>,
    start_time: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub level: AlertLevel,
    pub message: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

impl MonitoringService {
    pub fn new(config: MetricsConfig) -> Self {
        // Initialize metrics descriptions
        Self::initialize_metrics();
        
        Self {
            config,
            system_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
            application_metrics: Arc::new(RwLock::new(ApplicationMetrics::default())),
            business_metrics: Arc::new(RwLock::new(BusinessMetrics::default())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            metrics_history: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }
    
    fn initialize_metrics() {
        // System metrics
        describe_counter!("system_cpu_usage_total", "Total CPU usage counter");
        describe_histogram!("system_memory_usage_bytes", "Memory usage in bytes");
        describe_histogram!("system_disk_usage_bytes", "Disk usage in bytes");
        describe_gauge!("system_network_rx_bytes", "Network bytes received");
        describe_gauge!("system_network_tx_bytes", "Network bytes transmitted");
        describe_gauge!("system_active_connections", "Active network connections");
        
        // Application metrics
        describe_counter!("http_requests_total", "Total HTTP requests");
        describe_counter!("http_requests_success_total", "Total successful HTTP requests");
        describe_counter!("http_requests_error_total", "Total failed HTTP requests");
        describe_histogram!("http_request_duration_ms", "HTTP request duration in milliseconds");
        describe_gauge!("active_users_total", "Number of active users");
        describe_gauge!("wallets_scanned_total", "Total wallets scanned");
        describe_gauge!("sol_recovered_total", "Total SOL recovered");
        describe_gauge!("cache_hit_rate", "Cache hit rate percentage");
        describe_gauge!("database_connections_active", "Active database connections");
        describe_gauge!("rpc_connections_active", "Active RPC connections");
        describe_gauge!("queue_size", "Queue size");
        
        // Business metrics
        describe_gauge!("daily_active_users", "Daily active users");
        describe_gauge!("weekly_active_users", "Weekly active users");
        describe_gauge!("monthly_active_users", "Monthly active users");
        describe_gauge!("total_revenue_sol", "Total revenue in SOL");
        describe_gauge!("conversion_rate", "Conversion rate percentage");
        describe_gauge!("user_retention_rate", "User retention rate percentage");
        describe_gauge!("error_rate", "Error rate percentage");
        describe_gauge!("uptime_percentage", "Uptime percentage");
    }
    
    pub async fn start_collection(&self) -> Result<(), crate::SolanaRecoverError> {
        info!("Starting metrics collection with interval {}s", self.config.collection_interval_seconds);
        
        let system_metrics = self.system_metrics.clone();
        let application_metrics = self.application_metrics.clone();
        let business_metrics = self.business_metrics.clone();
        let alerts = self.alerts.clone();
        let metrics_history = self.metrics_history.clone();
        let config = self.config.clone();
        let start_time = self.start_time;
        
        // Start metrics collection task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.collection_interval_seconds));
            
            loop {
                interval.tick().await;
                
                // Collect system metrics
                if config.enable_system_metrics {
                    if let Ok(system_metrics_collected) = Self::collect_system_metrics().await {
                        Self::update_prometheus_system_metrics(&system_metrics_collected);
                        *system_metrics.write().await = system_metrics_collected;
                    }
                }
                
                // Update application metrics
                if config.enable_application_metrics {
                    let mut app_metrics = application_metrics.write().await;
                    app_metrics.timestamp = Utc::now();
                    Self::update_prometheus_application_metrics(&app_metrics);
                }
                
                // Update business metrics
                if config.enable_business_metrics {
                    let mut biz_metrics = business_metrics.write().await;
                    biz_metrics.timestamp = Utc::now();
                    biz_metrics.uptime_percentage = Self::calculate_uptime(start_time);
                    Self::update_prometheus_business_metrics(&biz_metrics);
                }
                
                // Check alerts
                {
                    let system = system_metrics.read().await;
                    let application = application_metrics.read().await;
                    Self::check_alerts(&*system, &*application, &config, &alerts).await;
                }
                
                // Cleanup old metrics history
                {
                    let mut history = metrics_history.write().await;
                    Self::cleanup_metrics_history(&mut *history, &config).await;
                }
            }
        });
        
        info!("Metrics collection started successfully");
        Ok(())
    }
    
    async fn collect_system_metrics() -> Result<SystemMetrics, crate::SolanaRecoverError> {
        let _metrics = SystemMetrics::default();
        
        // In a real implementation, you'd use system libraries to collect actual metrics
        // For now, we'll provide placeholder values
        
        // Memory metrics (using sysinfo crate in real implementation)
        let memory_usage_mb = 512; // Placeholder
        let memory_total_mb = 8192; // Placeholder
        
        // CPU usage (using sysinfo crate in real implementation)
        let cpu_usage_percent = 25.5; // Placeholder
        
        // Disk usage
        let disk_usage_mb = 2048; // Placeholder
        let disk_total_mb = 10240; // Placeholder
        
        let updated_metrics = SystemMetrics {
            timestamp: Utc::now(),
            cpu_usage_percent,
            memory_usage_mb,
            memory_total_mb,
            disk_usage_mb,
            disk_total_mb,
            network_rx_bytes: 1024000, // Placeholder
            network_tx_bytes: 512000,  // Placeholder
            active_connections: 150,    // Placeholder
            goroutines: 25,             // Placeholder (for Rust, this would be thread count)
            gc_pause_ms: 2,             // Placeholder
            heap_size_mb: memory_usage_mb,
            heap_objects: 50000,        // Placeholder
        };
        
        Ok(updated_metrics)
    }
    
    fn update_prometheus_system_metrics(metrics: &SystemMetrics) {
        gauge!("system_cpu_usage_percent", metrics.cpu_usage_percent as f64);
        gauge!("system_memory_usage_mb", metrics.memory_usage_mb as f64);
        gauge!("system_memory_total_mb", metrics.memory_total_mb as f64);
        gauge!("system_disk_usage_mb", metrics.disk_usage_mb as f64);
        gauge!("system_disk_total_mb", metrics.disk_total_mb as f64);
        gauge!("system_network_rx_bytes", metrics.network_rx_bytes as f64);
        gauge!("system_network_tx_bytes", metrics.network_tx_bytes as f64);
        gauge!("system_active_connections", metrics.active_connections as f64);
        gauge!("system_gc_pause_ms", metrics.gc_pause_ms as f64);
        gauge!("system_heap_size_mb", metrics.heap_size_mb as f64);
        gauge!("system_heap_objects", metrics.heap_objects as f64);
    }
    
    fn update_prometheus_application_metrics(metrics: &ApplicationMetrics) {
        gauge!("http_requests_total", metrics.total_requests as f64);
        gauge!("http_requests_success_total", metrics.successful_requests as f64);
        gauge!("http_requests_error_total", metrics.failed_requests as f64);
        gauge!("http_request_average_duration_ms", metrics.average_response_time_ms);
        gauge!("requests_per_second", metrics.requests_per_second);
        gauge!("active_users_total", metrics.active_users as f64);
        gauge!("wallets_scanned_total", metrics.total_wallets_scanned as f64);
        gauge!("sol_recovered_total", metrics.total_sol_recovered);
        gauge!("cache_hit_rate", metrics.cache_hit_rate);
        gauge!("database_connections_active", metrics.database_connections as f64);
        gauge!("rpc_connections_active", metrics.rpc_connections as f64);
        gauge!("queue_size", metrics.queue_size as f64);
    }
    
    fn update_prometheus_business_metrics(metrics: &BusinessMetrics) {
        gauge!("daily_active_users", metrics.daily_active_users as f64);
        gauge!("weekly_active_users", metrics.weekly_active_users as f64);
        gauge!("monthly_active_users", metrics.monthly_active_users as f64);
        gauge!("total_revenue_sol", metrics.total_revenue_sol);
        gauge!("average_revenue_per_user_sol", metrics.average_revenue_per_user_sol);
        gauge!("conversion_rate", metrics.conversion_rate);
        gauge!("user_retention_rate", metrics.user_retention_rate);
        gauge!("error_rate", metrics.error_rate);
        gauge!("uptime_percentage", metrics.uptime_percentage);
    }
    
    async fn check_alerts(
        system_metrics: &SystemMetrics,
        application_metrics: &ApplicationMetrics,
        config: &MetricsConfig,
        alerts: &Arc<RwLock<Vec<Alert>>>,
    ) {
        let mut new_alerts = Vec::new();
        
        // Check CPU usage
        if system_metrics.cpu_usage_percent > config.alert_thresholds.cpu_usage_percent {
            new_alerts.push(Alert {
                id: Uuid::new_v4(),
                level: AlertLevel::Warning,
                message: format!("CPU usage is {}%", system_metrics.cpu_usage_percent),
                metric_name: "cpu_usage_percent".to_string(),
                current_value: system_metrics.cpu_usage_percent,
                threshold: config.alert_thresholds.cpu_usage_percent,
                created_at: Utc::now(),
                resolved_at: None,
            });
        }
        
        // Check memory usage
        let memory_usage_percent = (system_metrics.memory_usage_mb as f64 / system_metrics.memory_total_mb as f64) * 100.0;
        if memory_usage_percent > config.alert_thresholds.memory_usage_percent {
            new_alerts.push(Alert {
                id: Uuid::new_v4(),
                level: AlertLevel::Warning,
                message: format!("Memory usage is {:.1}%", memory_usage_percent),
                metric_name: "memory_usage_percent".to_string(),
                current_value: memory_usage_percent,
                threshold: config.alert_thresholds.memory_usage_percent,
                created_at: Utc::now(),
                resolved_at: None,
            });
        }
        
        // Check error rate
        if application_metrics.total_requests > 0 {
            let error_rate = (application_metrics.failed_requests as f64 / application_metrics.total_requests as f64) * 100.0;
            if error_rate > config.alert_thresholds.error_rate_percent {
                new_alerts.push(Alert {
                    id: Uuid::new_v4(),
                    level: AlertLevel::Critical,
                    message: format!("Error rate is {:.1}%", error_rate),
                    metric_name: "error_rate".to_string(),
                    current_value: error_rate,
                    threshold: config.alert_thresholds.error_rate_percent,
                    created_at: Utc::now(),
                    resolved_at: None,
                });
            }
        }
        
        // Check response time
        if application_metrics.average_response_time_ms > config.alert_thresholds.response_time_ms {
            new_alerts.push(Alert {
                id: Uuid::new_v4(),
                level: AlertLevel::Warning,
                message: format!("Average response time is {:.1}ms", application_metrics.average_response_time_ms),
                metric_name: "response_time_ms".to_string(),
                current_value: application_metrics.average_response_time_ms,
                threshold: config.alert_thresholds.response_time_ms,
                created_at: Utc::now(),
                resolved_at: None,
            });
        }
        
        // Add new alerts to the existing alerts
        if !new_alerts.is_empty() {
            let mut alerts_guard = alerts.write().await;
            for alert in new_alerts {
                warn!("Alert triggered: {}", alert.message);
                alerts_guard.push(alert);
            }
            
            // Keep only recent alerts (last 100)
            let len = alerts_guard.len();
            if len > 100 {
                alerts_guard.drain(0..len - 100);
            }
        }
    }
    
    fn calculate_uptime(start_time: Instant) -> f64 {
        let _uptime = start_time.elapsed();
        100.0 // Always 100% for now, would calculate actual uptime
    }
    
    async fn cleanup_metrics_history(history: &mut HashMap<String, Vec<f64>>, config: &MetricsConfig) {
        let max_points = (config.retention_hours * 3600) / config.collection_interval_seconds;
        
        for values in history.values_mut() {
            if values.len() > max_points as usize {
                let excess = values.len() - max_points as usize;
                values.drain(0..excess);
            }
        }
    }
    
    // Public methods for updating metrics from application code
    pub async fn record_http_request(&self, success: bool, duration_ms: u64) {
        let mut metrics = self.application_metrics.write().await;
        metrics.total_requests += 1;
        
        if success {
            metrics.successful_requests += 1;
        } else {
            metrics.failed_requests += 1;
        }
        
        // Update average response time
        let total_time = metrics.average_response_time_ms * (metrics.total_requests - 1) as f64 + duration_ms as f64;
        metrics.average_response_time_ms = total_time / metrics.total_requests as f64;
        
        // Update Prometheus metrics
        counter!("http_requests_total", 1);
        histogram!("http_request_duration_ms", duration_ms as f64);
    }
    
    pub async fn record_wallet_scan(&self, successful: bool) {
        let mut metrics = self.application_metrics.write().await;
        if successful {
            metrics.total_wallets_scanned += 1;
            counter!("wallet_scans_total", 1);
        } else {
            counter!("wallet_scans_error", 1);
        }
    }
    
    pub async fn record_sol_recovery(&self, amount_sol: f64) {
        let mut metrics = self.application_metrics.write().await;
        metrics.total_sol_recovered += amount_sol;
        
        // Update business metrics
        let mut biz_metrics = self.business_metrics.write().await;
        biz_metrics.total_revenue_sol += amount_sol;
        
        // Update Prometheus metrics
        gauge!("sol_recovered_total", metrics.total_sol_recovered);
        gauge!("total_revenue_sol", biz_metrics.total_revenue_sol);
    }
    
    pub async fn update_cache_hit_rate(&self, hit_rate: f64) {
        let mut metrics = self.application_metrics.write().await;
        metrics.cache_hit_rate = hit_rate;
        gauge!("cache_hit_rate", hit_rate);
    }
    
    pub async fn update_connection_counts(&self, db_connections: u32, rpc_connections: u32) {
        let mut metrics = self.application_metrics.write().await;
        metrics.database_connections = db_connections;
        metrics.rpc_connections = rpc_connections;
        
        gauge!("database_connections_active", db_connections as f64);
        gauge!("rpc_connections_active", rpc_connections as f64);
    }
    
    pub async fn update_queue_size(&self, size: u64) {
        let mut metrics = self.application_metrics.write().await;
        metrics.queue_size = size;
        gauge!("queue_size", size as f64);
    }
    
    pub async fn update_active_users(&self, count: u64) {
        let mut metrics = self.application_metrics.write().await;
        metrics.active_users = count;
        
        let mut biz_metrics = self.business_metrics.write().await;
        biz_metrics.daily_active_users = count;
        
        gauge!("active_users_total", count as f64);
        gauge!("daily_active_users", count as f64);
    }
    
    pub async fn get_current_metrics(&self) -> (SystemMetrics, ApplicationMetrics, BusinessMetrics) {
        let system = self.system_metrics.read().await.clone();
        let application = self.application_metrics.read().await.clone();
        let business = self.business_metrics.read().await.clone();
        
        (system, application, business)
    }
    
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        self.alerts.read().await.clone()
    }
    
    pub async fn resolve_alert(&self, alert_id: Uuid) -> bool {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved_at = Some(Utc::now());
            info!("Alert resolved: {}", alert.message);
            true
        } else {
            false
        }
    }
    
    pub async fn get_metrics_summary(&self) -> serde_json::Value {
        let (system, application, business) = self.get_current_metrics().await;
        let alerts = self.get_active_alerts().await;
        
        serde_json::json!({
            "timestamp": Utc::now(),
            "uptime_seconds": self.start_time.elapsed().as_secs(),
            "system": system,
            "application": application,
            "business": business,
            "alerts": alerts,
            "alert_count": alerts.len(),
            "config": {
                "collection_interval_seconds": self.config.collection_interval_seconds,
                "retention_hours": self.config.retention_hours,
                "prometheus_port": self.config.prometheus_port,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_monitoring_service_creation() {
        let config = MetricsConfig::default();
        let service = MonitoringService::new(config);
        
        let (system, application, business) = service.get_current_metrics().await;
        
        assert_eq!(system.total_connections, 0);
        assert_eq!(application.total_requests, 0);
        assert_eq!(business.daily_active_users, 0);
    }
    
    #[tokio::test]
    async fn test_metrics_recording() {
        let config = MetricsConfig::default();
        let service = MonitoringService::new(config);
        
        service.record_http_request(true, 100).await;
        service.record_wallet_scan(true).await;
        service.record_sol_recovery(1.5).await;
        
        let (_, application, _) = service.get_current_metrics().await;
        
        assert_eq!(application.total_requests, 1);
        assert_eq!(application.successful_requests, 1);
        assert_eq!(application.total_wallets_scanned, 1);
        assert_eq!(application.total_sol_recovered, 1.5);
    }
    
    #[tokio::test]
    async fn test_alert_creation() {
        let config = MetricsConfig {
            alert_thresholds: AlertThresholds {
                cpu_usage_percent: 10.0, // Low threshold for testing
                ..Default::default()
            },
            ..Default::default()
        };
        
        let service = MonitoringService::new(config);
        
        // Simulate high CPU usage
        {
            let mut system_metrics = service.system_metrics.write().await;
            system_metrics.cpu_usage_percent = 85.0;
        }
        
        // Check alerts
        MonitoringService::check_alerts(
            &service.system_metrics.read().await,
            &service.application_metrics.read().await,
            &service.config,
            &service.alerts,
        ).await;
        
        let alerts = service.get_active_alerts().await;
        assert!(!alerts.is_empty());
        
        let cpu_alert = alerts.iter().find(|a| a.metric_name == "cpu_usage_percent");
        assert!(cpu_alert.is_some());
    }
}
