use solana_recover::utils::*;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_metrics_collector_counter() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);

    collector.increment_counter("test_counter", None).await;
    collector.increment_counter("test_counter", None).await;

    let snapshot = collector.get_snapshot().await;
    assert_eq!(snapshot.counters.len(), 1);
    assert_eq!(snapshot.counters[0].value, 2);
    assert_eq!(snapshot.counters[0].name, "test_counter");
}

#[tokio::test]
async fn test_metrics_collector_counter_with_labels() {
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

#[tokio::test]
async fn test_metrics_collector_gauge() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);

    collector.set_gauge("test_gauge", 42.5, None).await;
    collector.set_gauge("test_gauge", 100.0, None).await;

    let snapshot = collector.get_snapshot().await;
    assert_eq!(snapshot.gauges.len(), 1);
    assert_eq!(snapshot.gauges[0].value, 100.0);
    assert_eq!(snapshot.gauges[0].name, "test_gauge");
}

#[tokio::test]
async fn test_metrics_collector_histogram() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);

    collector.record_histogram("test_histogram", 5.0, None).await;
    collector.record_histogram("test_histogram", 15.0, None).await;
    collector.record_histogram("test_histogram", 0.5, None).await;

    let snapshot = collector.get_snapshot().await;
    assert_eq!(snapshot.histograms.len(), 1);
    assert_eq!(snapshot.histograms[0].count, 3);
    assert_eq!(snapshot.histograms[0].sum, 20.5);
    
    // Check bucket counts
    assert!(snapshot.histograms[0].bucket_counts.iter().sum::<u64>() >= 3);
}

#[tokio::test]
async fn test_metrics_collector_timer() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);

    collector.record_timer("test_timer", 150, None).await;
    collector.record_timer("test_timer", 200, None).await;

    let snapshot = collector.get_snapshot().await;
    assert_eq!(snapshot.timers.len(), 2);
    assert_eq!(snapshot.timers[0].duration_ms, 150);
    assert_eq!(snapshot.timers[1].duration_ms, 200);
}

#[tokio::test]
async fn test_metrics_collector_timer_scope() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);

    {
        let _timer = collector.timer_scope("scoped_timer", None);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let snapshot = collector.get_snapshot().await;
    assert_eq!(snapshot.timers.len(), 1);
    assert!(snapshot.timers[0].duration_ms >= 10);
    assert_eq!(snapshot.timers[0].name, "scoped_timer");
}

#[tokio::test]
async fn test_metrics_collector_reset() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);

    collector.increment_counter("test_counter", None).await;
    collector.set_gauge("test_gauge", 42.0, None).await;
    collector.record_timer("test_timer", 100, None).await;

    collector.reset().await;

    let snapshot = collector.get_snapshot().await;
    assert_eq!(snapshot.counters.len(), 0);
    assert_eq!(snapshot.gauges.len(), 0);
    assert_eq!(snapshot.timers.len(), 0);
}

#[tokio::test]
async fn test_metrics_collector_disabled() {
    let mut config = MetricsConfig::default();
    config.enabled = false;
    let collector = MetricsCollector::new(config);

    collector.increment_counter("test_counter", None).await;
    collector.set_gauge("test_gauge", 42.0, None).await;
    collector.record_timer("test_timer", 100, None).await;

    let snapshot = collector.get_snapshot().await;
    assert_eq!(snapshot.counters.len(), 0);
    assert_eq!(snapshot.gauges.len(), 0);
    assert_eq!(snapshot.timers.len(), 0);
}

#[tokio::test]
async fn test_metrics_collector_summary() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config);

    collector.increment_counter("test_counter", None).await;
    collector.set_gauge("test_gauge", 42.0, None).await;
    collector.record_timer("test_timer", 100, None).await;
    collector.record_timer("test_timer", 200, None).await;

    let summary = collector.get_metric_summary().await;
    
    assert!(summary.contains_key("counter:test_counter"));
    assert!(summary.contains_key("gauge:test_gauge"));
    assert!(summary.contains_key("timer:test_timer"));
    
    // Check timer summary
    let timer_summary = &summary["timer:test_timer"];
    assert_eq!(timer_summary["count"], 2);
    assert_eq!(timer_summary["min_ms"], 100);
    assert_eq!(timer_summary["max_ms"], 200);
}

#[test]
fn test_metrics_config_default() {
    let config = MetricsConfig::default();
    assert!(config.enabled);
    assert_eq!(config.max_timers, 10000);
    assert_eq!(config.retention_duration, Duration::from_secs(3600));
    assert!(!config.histogram_buckets.is_empty());
    assert_eq!(config.histogram_buckets[0], 0.1);
}

#[test]
fn test_logging_config_default() {
    let config = LoggingConfig::default();
    assert_eq!(config.level, "info");
    assert!(matches!(config.format, LogFormat::Pretty));
    assert!(matches!(config.output, LogOutput::Stdout));
    assert!(config.file_path.is_none());
    assert!(!config.json_fields.is_empty());
}

#[test]
fn test_log_format_variants() {
    let pretty = LogFormat::Pretty;
    let json = LogFormat::Json;
    let compact = LogFormat::Compact;
    
    // Test that we can create all variants
    assert!(matches!(pretty, LogFormat::Pretty));
    assert!(matches!(json, LogFormat::Json));
    assert!(matches!(compact, LogFormat::Compact));
}

#[test]
fn test_log_output_variants() {
    let stdout = LogOutput::Stdout;
    let stderr = LogOutput::Stderr;
    let file = LogOutput::File("test.log".to_string());
    let both = LogOutput::Both("test.log".to_string());
    
    assert!(matches!(stdout, LogOutput::Stdout));
    assert!(matches!(stderr, LogOutput::Stderr));
    if let LogOutput::File(path) = file {
        assert_eq!(path, "test.log");
    }
    if let LogOutput::Both(path) = both {
        assert_eq!(path, "test.log");
    }
}

#[test]
fn test_structured_logger_functions() {
    // These tests just verify the functions compile and don't panic
    StructuredLogger::log_wallet_scan_start("test_address", "test_scan_id");
    StructuredLogger::log_wallet_scan_complete("test_address", "test_scan_id", 1000, 0.1);
    StructuredLogger::log_wallet_scan_error("test_address", "test_scan_id", "test error");
    StructuredLogger::log_batch_scan_start("test_batch", 10);
    StructuredLogger::log_batch_scan_complete("test_batch", 5000, 8, 2, 0.5);
    StructuredLogger::log_rpc_request("https://api.mainnet-beta.solana.com", "getAccountInfo", 100);
    StructuredLogger::log_rpc_error("https://api.mainnet-beta.solana.com", "getAccountInfo", "timeout");
    StructuredLogger::log_api_request("GET", "/api/v1/scan", 200, 50);
    StructuredLogger::log_cache_hit("test_key");
    StructuredLogger::log_cache_miss("test_key");
    StructuredLogger::log_wallet_connection("phantom", "conn_123");
    StructuredLogger::log_wallet_disconnection("phantom", "conn_123");
    StructuredLogger::log_fee_calculation("test_address", 0.1, 0.015, 0.085);
    StructuredLogger::log_security_event("login_attempt", "failed login", Some("user_123"));
    StructuredLogger::log_performance_metric("scan_duration", 1500.0, "ms");
}

#[test]
fn test_metric_names() {
    assert_eq!(metrics_names::WALLET_SCANS_TOTAL, "wallet_scans_total");
    assert_eq!(metrics_names::WALLET_SCANS_SUCCESSFUL, "wallet_scans_successful");
    assert_eq!(metrics_names::WALLET_SCANS_FAILED, "wallet_scans_failed");
    assert_eq!(metrics_names::WALLET_SCAN_DURATION_MS, "wallet_scan_duration_ms");
    assert_eq!(metrics_names::BATCH_SCANS_TOTAL, "batch_scans_total");
    assert_eq!(metrics_names::BATCH_SCAN_DURATION_MS, "batch_scan_duration_ms");
    assert_eq!(metrics_names::RPC_REQUESTS_TOTAL, "rpc_requests_total");
    assert_eq!(metrics_names::RPC_REQUEST_DURATION_MS, "rpc_request_duration_ms");
    assert_eq!(metrics_names::RPC_ERRORS_TOTAL, "rpc_errors_total");
    assert_eq!(metrics_names::ACTIVE_CONNECTIONS, "active_connections");
    assert_eq!(metrics_names::CACHE_HITS, "cache_hits");
    assert_eq!(metrics_names::CACHE_MISSES, "cache_misses");
    assert_eq!(metrics_names::API_REQUESTS_TOTAL, "api_requests_total");
    assert_eq!(metrics_names::API_REQUEST_DURATION_MS, "api_request_duration_ms");
    assert_eq!(metrics_names::MEMORY_USAGE_BYTES, "memory_usage_bytes");
    assert_eq!(metrics_names::CPU_USAGE_PERCENT, "cpu_usage_percent");
}

#[test]
fn test_metric_value_creation() {
    let mut labels = HashMap::new();
    labels.insert("test".to_string(), "value".to_string());
    
    let metric_value = MetricValue {
        value: 42.5,
        timestamp: chrono::Utc::now(),
        labels: labels.clone(),
    };
    
    assert_eq!(metric_value.value, 42.5);
    assert_eq!(metric_value.labels, labels);
}

#[test]
fn test_counter_creation() {
    let mut labels = HashMap::new();
    labels.insert("test".to_string(), "value".to_string());
    
    let counter = Counter {
        name: "test_counter".to_string(),
        value: 10,
        labels: labels.clone(),
        created_at: chrono::Utc::now(),
    };
    
    assert_eq!(counter.name, "test_counter");
    assert_eq!(counter.value, 10);
    assert_eq!(counter.labels, labels);
}

#[test]
fn test_gauge_creation() {
    let mut labels = HashMap::new();
    labels.insert("test".to_string(), "value".to_string());
    
    let gauge = Gauge {
        name: "test_gauge".to_string(),
        value: 25.7,
        labels: labels.clone(),
        updated_at: chrono::Utc::now(),
    };
    
    assert_eq!(gauge.name, "test_gauge");
    assert_eq!(gauge.value, 25.7);
    assert_eq!(gauge.labels, labels);
}

#[test]
fn test_histogram_creation() {
    let mut labels = HashMap::new();
    labels.insert("test".to_string(), "value".to_string());
    
    let histogram = Histogram {
        name: "test_histogram".to_string(),
        count: 5,
        sum: 25.0,
        buckets: vec![1.0, 5.0, 10.0],
        bucket_counts: vec![1, 3, 4, 5],
        labels: labels.clone(),
        created_at: chrono::Utc::now(),
    };
    
    assert_eq!(histogram.name, "test_histogram");
    assert_eq!(histogram.count, 5);
    assert_eq!(histogram.sum, 25.0);
    assert_eq!(histogram.buckets, vec![1.0, 5.0, 10.0]);
    assert_eq!(histogram.bucket_counts, vec![1, 3, 4, 5]);
    assert_eq!(histogram.labels, labels);
}

#[test]
fn test_timer_creation() {
    let mut labels = HashMap::new();
    labels.insert("test".to_string(), "value".to_string());
    
    let timer = Timer {
        name: "test_timer".to_string(),
        duration_ms: 150,
        labels: labels.clone(),
        timestamp: chrono::Utc::now(),
    };
    
    assert_eq!(timer.name, "test_timer");
    assert_eq!(timer.duration_ms, 150);
    assert_eq!(timer.labels, labels);
}
