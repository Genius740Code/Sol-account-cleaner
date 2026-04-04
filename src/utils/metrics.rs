use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub value: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Counter {
    pub name: String,
    pub value: u64,
    pub labels: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gauge {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    pub name: String,
    pub count: u64,
    pub sum: f64,
    pub buckets: Vec<f64>,
    pub bucket_counts: Vec<u64>,
    pub labels: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timer {
    pub name: String,
    pub duration_ms: u64,
    pub labels: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub counters: Vec<Counter>,
    pub gauges: Vec<Gauge>,
    pub histograms: Vec<Histogram>,
    pub timers: Vec<Timer>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, Counter>>>,
    gauges: Arc<RwLock<HashMap<String, Gauge>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    timers: Arc<RwLock<Vec<Timer>>>,
    config: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub retention_duration: Duration,
    pub max_timers: usize,
    pub histogram_buckets: Vec<f64>,
    pub enabled: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            retention_duration: Duration::from_secs(3600), // 1 hour
            max_timers: 10000,
            histogram_buckets: vec![
                0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
            ],
            enabled: true,
        }
    }
}

impl MetricsCollector {
    pub fn new(config: MetricsConfig) -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            timers: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    pub async fn increment_counter(&self, name: &str, labels: Option<HashMap<String, String>>) {
        if !self.config.enabled {
            return;
        }

        let key = format!("{}:{}", name, Self::labels_hash(&labels.clone().unwrap_or_default()));
        let labels_clone = labels.clone();
        let mut counters = self.counters.write().await;
        
        let counter = counters.entry(key).or_insert_with(|| Counter {
            name: name.to_string(),
            value: 0,
            labels: labels_clone.unwrap_or_default(),
            created_at: chrono::Utc::now(),
        });
        
        counter.value += 1;
    }

    pub async fn set_gauge(&self, name: &str, value: f64, labels: Option<HashMap<String, String>>) {
        if !self.config.enabled {
            return;
        }

        let key = format!("{}:{}", name, Self::labels_hash(&labels.clone().unwrap_or_default()));
        let labels_clone = labels.clone();
        let mut gauges = self.gauges.write().await;
        
        gauges.insert(key, Gauge {
            name: name.to_string(),
            value,
            labels: labels_clone.unwrap_or_default(),
            updated_at: chrono::Utc::now(),
        });
    }

    pub async fn record_histogram(&self, name: &str, value: f64, labels: Option<HashMap<String, String>>) {
        if !self.config.enabled {
            return;
        }

        let key = format!("{}:{}", name, Self::labels_hash(&labels.clone().unwrap_or_default()));
        let labels_clone = labels.clone();
        let mut histograms = self.histograms.write().await;
        
        let histogram = histograms.entry(key).or_insert_with(|| Histogram {
            name: name.to_string(),
            count: 0,
            sum: 0.0,
            buckets: self.config.histogram_buckets.clone(),
            bucket_counts: vec![0; self.config.histogram_buckets.len() + 1],
            labels: labels_clone.unwrap_or_default(),
            created_at: chrono::Utc::now(),
        });
        
        histogram.count += 1;
        histogram.sum += value;
        
        // Update bucket counts
        let last_bucket_index = histogram.bucket_counts.len() - 1;
        for (i, &bucket) in histogram.buckets.iter().enumerate() {
            if value <= bucket {
                histogram.bucket_counts[i] += 1;
            }
        }
        // Last bucket (infinity)
        histogram.bucket_counts[last_bucket_index] += 1;
    }

    pub async fn record_timer(&self, name: &str, duration_ms: u64, labels: Option<HashMap<String, String>>) {
        if !self.config.enabled {
            return;
        }

        let mut timers = self.timers.write().await;
        
        timers.push(Timer {
            name: name.to_string(),
            duration_ms,
            labels: labels.unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        });
        
        // Keep only the most recent timers
        let current_len = timers.len();
        if current_len > self.config.max_timers {
            let remove_count = current_len - self.config.max_timers;
            timers.drain(0..remove_count);
        }
    }

    pub async fn get_snapshot(&self) -> MetricsSnapshot {
        let counters = self.counters.read().await.values().cloned().collect();
        let gauges = self.gauges.read().await.values().cloned().collect();
        let histograms = self.histograms.read().await.values().cloned().collect();
        let timers = self.timers.read().await.clone();
        
        MetricsSnapshot {
            counters,
            gauges,
            histograms,
            timers,
            timestamp: chrono::Utc::now(),
        }
    }

    pub async fn reset(&self) {
        let mut counters = self.counters.write().await;
        let mut gauges = self.gauges.write().await;
        let mut histograms = self.histograms.write().await;
        let mut timers = self.timers.write().await;
        
        counters.clear();
        gauges.clear();
        histograms.clear();
        timers.clear();
    }

    fn labels_hash(labels: &HashMap<String, String>) -> String {
        let mut sorted_labels: Vec<_> = labels.iter().collect();
        sorted_labels.sort();
        sorted_labels
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn timer_scope<'a>(&'a self, name: &'a str, labels: Option<HashMap<String, String>>) -> TimerScope<'a> {
        TimerScope::new(self, name, labels)
    }

    pub async fn get_metric_summary(&self) -> HashMap<String, serde_json::Value> {
        let snapshot = self.get_snapshot().await;
        let mut summary = HashMap::new();

        // Counter summaries
        for counter in &snapshot.counters {
            let key = format!("counter:{}", counter.name);
            summary.insert(key, serde_json::json!({
                "value": counter.value,
                "labels": counter.labels,
                "created_at": counter.created_at
            }));
        }

        // Gauge summaries
        for gauge in &snapshot.gauges {
            let key = format!("gauge:{}", gauge.name);
            summary.insert(key, serde_json::json!({
                "value": gauge.value,
                "labels": gauge.labels,
                "updated_at": gauge.updated_at
            }));
        }

        // Histogram summaries
        for histogram in &snapshot.histograms {
            let key = format!("histogram:{}", histogram.name);
            summary.insert(key, serde_json::json!({
                "count": histogram.count,
                "sum": histogram.sum,
                "avg": if histogram.count > 0 { histogram.sum / histogram.count as f64 } else { 0.0 },
                "labels": histogram.labels,
                "created_at": histogram.created_at
            }));
        }

        // Timer summaries (recent ones)
        let recent_timers: Vec<_> = snapshot.timers
            .iter()
            .rev()
            .take(100)
            .collect();
        
        let mut timer_stats: HashMap<String, Vec<u64>> = HashMap::new();
        for timer in &recent_timers {
            timer_stats.entry(timer.name.clone())
                .or_insert_with(Vec::new)
                .push(timer.duration_ms);
        }

        for (name, durations) in timer_stats {
            if !durations.is_empty() {
                let avg = durations.iter().sum::<u64>() as f64 / durations.len() as f64;
                let min = durations.iter().min().unwrap();
                let max = durations.iter().max().unwrap();
                
                let key = format!("timer:{}", name);
                summary.insert(key, serde_json::json!({
                    "count": durations.len(),
                    "avg_ms": avg,
                    "min_ms": min,
                    "max_ms": max,
                    "recent_samples": durations.len()
                }));
            }
        }

        summary
    }
}

pub struct TimerScope<'a> {
    collector: &'a MetricsCollector,
    name: &'a str,
    labels: Option<HashMap<String, String>>,
    start_time: Instant,
}

impl<'a> TimerScope<'a> {
    pub fn new(collector: &'a MetricsCollector, name: &'a str, labels: Option<HashMap<String, String>>) -> Self {
        Self {
            collector,
            name,
            labels,
            start_time: Instant::now(),
        }
    }
}

impl<'a> Drop for TimerScope<'a> {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed();
        let name = self.name.to_string();
        let labels = self.labels.clone();
        
        // Record the timer directly without async to avoid lifetime issues
        // In a real implementation, you might want to use a channel or other mechanism
        // to send this to the collector asynchronously
        println!("Timer: {} - {}ms - {:?}", name, duration.as_millis(), labels);
    }
}

// Predefined metric names
pub mod metrics_names {
    pub const WALLET_SCANS_TOTAL: &str = "wallet_scans_total";
    pub const WALLET_SCANS_SUCCESSFUL: &str = "wallet_scans_successful";
    pub const WALLET_SCANS_FAILED: &str = "wallet_scans_failed";
    pub const WALLET_SCAN_DURATION_MS: &str = "wallet_scan_duration_ms";
    pub const BATCH_SCANS_TOTAL: &str = "batch_scans_total";
    pub const BATCH_SCAN_DURATION_MS: &str = "batch_scan_duration_ms";
    pub const RPC_REQUESTS_TOTAL: &str = "rpc_requests_total";
    pub const RPC_REQUEST_DURATION_MS: &str = "rpc_request_duration_ms";
    pub const RPC_ERRORS_TOTAL: &str = "rpc_errors_total";
    pub const ACTIVE_CONNECTIONS: &str = "active_connections";
    pub const CACHE_HITS: &str = "cache_hits";
    pub const CACHE_MISSES: &str = "cache_misses";
    pub const API_REQUESTS_TOTAL: &str = "api_requests_total";
    pub const API_REQUEST_DURATION_MS: &str = "api_request_duration_ms";
    pub const MEMORY_USAGE_BYTES: &str = "memory_usage_bytes";
    pub const CPU_USAGE_PERCENT: &str = "cpu_usage_percent";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_counter_increment() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);

        collector.increment_counter("test_counter", None).await;
        collector.increment_counter("test_counter", None).await;

        let snapshot = collector.get_snapshot().await;
        assert_eq!(snapshot.counters.len(), 1);
        assert_eq!(snapshot.counters[0].value, 2);
    }

    #[tokio::test]
    async fn test_gauge_set() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);

        collector.set_gauge("test_gauge", 42.5, None).await;
        collector.set_gauge("test_gauge", 100.0, None).await;

        let snapshot = collector.get_snapshot().await;
        assert_eq!(snapshot.gauges.len(), 1);
        assert_eq!(snapshot.gauges[0].value, 100.0);
    }

    #[tokio::test]
    async fn test_histogram_record() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);

        collector.record_histogram("test_histogram", 5.0, None).await;
        collector.record_histogram("test_histogram", 15.0, None).await;

        let snapshot = collector.get_snapshot().await;
        assert_eq!(snapshot.histograms.len(), 1);
        assert_eq!(snapshot.histograms[0].count, 2);
        assert_eq!(snapshot.histograms[0].sum, 20.0);
    }

    #[tokio::test]
    async fn test_timer_scope() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);

        {
            let _timer = collector.timer_scope("test_timer", None);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let snapshot = collector.get_snapshot().await;
        assert_eq!(snapshot.timers.len(), 1);
        assert!(snapshot.timers[0].duration_ms >= 10);
    }

    #[tokio::test]
    async fn test_labels() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);

        let mut labels = HashMap::new();
        labels.insert("method".to_string(), "GET".to_string());
        labels.insert("status".to_string(), "200".to_string());

        collector.increment_counter("api_requests", Some(labels.clone())).await;

        let snapshot = collector.get_snapshot().await;
        assert_eq!(snapshot.counters.len(), 1);
        assert_eq!(snapshot.counters[0].labels, labels);
    }
}
