use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use tracing::{debug, warn, error};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// High-performance asynchronous audit logger
#[derive(Clone)]
pub struct AsyncAuditLogger {
    /// Async log processor
    processor: Arc<AuditLogProcessor>,
    /// Configuration
    config: AuditConfig,
}

/// Audit log processor with batching and optimization
#[derive(Debug)]
#[allow(dead_code)]
pub struct AuditLogProcessor {
    /// Log entry queue
    log_queue: Arc<RwLock<mpsc::UnboundedSender<AuditLogEntry>>>,
    /// Batch processor handle
    batch_handle: Option<tokio::task::JoinHandle<()>>,
    /// Metrics
    metrics: Arc<RwLock<AuditMetrics>>,
    /// Configuration
    #[allow(dead_code)]
    config: AuditConfig,
}

/// Audit configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Batch size for log processing
    pub batch_size: usize,
    /// Batch flush interval
    pub flush_interval: Duration,
    /// Enable compression for batch logs
    pub enable_compression: bool,
    /// Enable cryptographic signatures
    pub enable_signatures: bool,
    /// Maximum queue size before dropping logs
    pub max_queue_size: usize,
    /// Log retention period
    pub retention_period: Duration,
    /// Enable structured logging
    pub enable_structured: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            flush_interval: Duration::from_secs(5),
            enable_compression: true,
            enable_signatures: true,
            max_queue_size: 100_000,
            retention_period: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            enable_structured: true,
        }
    }
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique entry ID
    pub id: String,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: AuditEventType,
    /// User or system performing the action
    pub actor: String,
    /// Action performed
    pub action: String,
    /// Target resource
    pub target: Option<String>,
    /// Event details
    pub details: serde_json::Value,
    /// IP address of the requester
    pub ip_address: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// Cryptographic signature
    pub signature: Option<String>,
    /// Processing time in microseconds
    pub processing_time_us: Option<u64>,
}

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Authentication events
    Authentication,
    /// Authorization events
    Authorization,
    /// Data access events
    DataAccess,
    /// Data modification events
    DataModification,
    /// System events
    System,
    /// Security events
    Security,
    /// Performance events
    Performance,
    /// Error events
    Error,
    /// Custom event type
    Custom(String),
}

/// Audit performance metrics
#[derive(Debug, Default, Clone)]
pub struct AuditMetrics {
    /// Total log entries processed
    pub total_entries: u64,
    /// Entries dropped due to queue overflow
    pub dropped_entries: u64,
    /// Average processing time per entry (microseconds)
    pub avg_processing_time_us: f64,
    /// Current queue size
    pub queue_size: usize,
    /// Batches processed
    pub batches_processed: u64,
    /// Compression ratio
    pub compression_ratio: f64,
    /// Throughput (entries per second)
    pub throughput_eps: f64,
}

/// Batch of audit log entries
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AuditBatch {
    #[allow(dead_code)]
    entries: Vec<AuditLogEntry>,
    #[allow(dead_code)]
    batch_id: String,
    #[allow(dead_code)]
    created_at: Instant,
    #[allow(dead_code)]
    size_bytes: usize,
}

impl AsyncAuditLogger {
    /// Create new async audit logger
    pub fn new(config: AuditConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let _tx = Arc::new(RwLock::new(tx));
        
        let processor = Arc::new(AuditLogProcessor::new(config.clone(), rx)?);
        
        Ok(Self {
            processor,
            config,
        })
    }

    /// Log an audit event asynchronously
    pub async fn log_event(&self, entry: AuditLogEntry) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        
        // Add signature if enabled
        let mut entry = entry;
        if self.config.enable_signatures {
            entry.signature = Some(self.generate_signature(&entry).await?);
        }

        // Send to async processor
        let sender = self.processor.log_queue.read().await;
        if let Err(e) = sender.send(entry) {
            warn!("Failed to queue audit log entry: {}", e);
            
            // Update dropped metrics
            let mut metrics = self.processor.metrics.write().await;
            metrics.dropped_entries += 1;
        }

        // Update metrics
        let processing_time = start_time.elapsed().as_micros() as u64;
        {
            let mut metrics = self.processor.metrics.write().await;
            metrics.total_entries += 1;
            metrics.avg_processing_time_us = (metrics.avg_processing_time_us * (metrics.total_entries - 1) as f64 + processing_time as f64) / metrics.total_entries as f64;
        }

        Ok(())
    }

    /// Create a simple audit log entry
    pub async fn create_entry(
        &self,
        event_type: AuditEventType,
        actor: String,
        action: String,
        details: serde_json::Value,
    ) -> AuditLogEntry {
        AuditLogEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type,
            actor,
            action,
            target: None,
            details,
            ip_address: None,
            user_agent: None,
            session_id: None,
            request_id: None,
            signature: None,
            processing_time_us: None,
        }
    }

    /// Log wallet scan event
    pub async fn log_wallet_scan(
        &self,
        wallet_address: &str,
        actor: &str,
        scan_result: &serde_json::Value,
        processing_time_us: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let entry = self.create_entry(
            AuditEventType::DataAccess,
            actor.to_string(),
            format!("wallet_scan:{}", wallet_address),
            scan_result.clone(),
        ).await;

        let mut entry_with_time = entry;
        entry_with_time.target = Some(wallet_address.to_string());
        entry_with_time.processing_time_us = Some(processing_time_us);

        self.log_event(entry_with_time).await
    }

    /// Log batch processing event
    pub async fn log_batch_processing(
        &self,
        batch_id: &str,
        wallet_count: usize,
        actor: &str,
        processing_time_us: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let details = serde_json::json!({
            "batch_id": batch_id,
            "wallet_count": wallet_count,
            "processing_time_us": processing_time_us,
            "throughput_wps": (wallet_count as f64 * 1_000_000.0) / processing_time_us as f64
        });

        let entry = self.create_entry(
            AuditEventType::Performance,
            actor.to_string(),
            format!("batch_processing:{}", batch_id),
            details,
        ).await;

        let mut entry_with_time = entry;
        entry_with_time.target = Some(batch_id.to_string());
        entry_with_time.processing_time_us = Some(processing_time_us);

        self.log_event(entry_with_time).await
    }

    /// Log security event
    pub async fn log_security_event(
        &self,
        event_type: &str,
        actor: &str,
        details: serde_json::Value,
        ip_address: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let entry = self.create_entry(
            AuditEventType::Security,
            actor.to_string(),
            event_type.to_string(),
            details,
        ).await;

        let mut security_entry = entry;
        security_entry.ip_address = ip_address;

        self.log_event(security_entry).await
    }

    /// Get current audit metrics
    pub async fn get_metrics(&self) -> AuditMetrics {
        self.processor.metrics.read().await.clone()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.processor.metrics.write().await;
        *metrics = AuditMetrics::default();
    }

    /// Generate cryptographic signature for audit entry
    async fn generate_signature(&self, entry: &AuditLogEntry) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let entry_json = serde_json::to_string(entry)?;
        let mut hasher = Sha256::new();
        hasher.update(entry_json.as_bytes());
        let hash = hasher.finalize();
        
        Ok(general_purpose::STANDARD.encode(hash))
    }
}

impl AuditLogProcessor {
    fn new(config: AuditConfig, receiver: mpsc::UnboundedReceiver<AuditLogEntry>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, _) = mpsc::unbounded_channel();
        let tx = Arc::new(RwLock::new(tx));
        let metrics = Arc::new(RwLock::new(AuditMetrics::default()));
        
        let config_clone = config.clone();
        let metrics_clone = metrics.clone();
        
        // Start batch processing task
        let batch_handle = tokio::spawn(async move {
            Self::batch_processor(config_clone, receiver, metrics_clone).await;
        });

        Ok(Self {
            log_queue: tx,
            batch_handle: Some(batch_handle),
            metrics,
            config,
        })
    }

    /// Async batch processor for audit logs
    async fn batch_processor(
        config: AuditConfig,
        mut receiver: mpsc::UnboundedReceiver<AuditLogEntry>,
        metrics: Arc<RwLock<AuditMetrics>>,
    ) {
        let mut batch = Vec::with_capacity(config.batch_size);
        let mut last_flush = Instant::now();
        let mut flush_interval = interval(config.flush_interval);
        flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                // Receive new log entry
                entry = receiver.recv() => {
                    match entry {
                        Some(log_entry) => {
                            batch.push(log_entry);
                            
                            // Check if batch is full
                            if batch.len() >= config.batch_size {
                                Self::process_batch(&config, &metrics, std::mem::take(&mut batch)).await;
                                last_flush = Instant::now();
                            }
                        }
                        None => {
                            // Channel closed, process remaining entries and exit
                            if !batch.is_empty() {
                                Self::process_batch(&config, &metrics, batch).await;
                            }
                            break;
                        }
                    }
                }
                
                // Flush on interval
                _ = flush_interval.tick() => {
                    if !batch.is_empty() && last_flush.elapsed() >= config.flush_interval {
                        Self::process_batch(&config, &metrics, std::mem::take(&mut batch)).await;
                        last_flush = Instant::now();
                    }
                }
            }
        }
    }

    /// Process a batch of audit log entries
    async fn process_batch(
        config: &AuditConfig,
        metrics: &Arc<RwLock<AuditMetrics>>,
        batch: Vec<AuditLogEntry>,
    ) {
        let start_time = Instant::now();
        
        if batch.is_empty() {
            return;
        }

        let batch_id = Uuid::new_v4().to_string();
        let size_bytes = batch.iter().map(|e| e.id.len() + 100).sum(); // Rough estimate

        // Create audit batch
        let audit_batch = AuditBatch {
            entries: batch,
            batch_id,
            created_at: start_time,
            size_bytes,
        };

        // Process batch (write to storage, send to external system, etc.)
        if let Err(e) = Self::write_batch_to_storage(config, &audit_batch).await {
            error!("Failed to write audit batch {}: {}", audit_batch.batch_id, e);
        }

        let processing_time = start_time.elapsed();
        
        // Update metrics
        {
            let mut metrics_guard = metrics.write().await;
            metrics_guard.batches_processed += 1;
            metrics_guard.queue_size = metrics_guard.queue_size.saturating_sub(audit_batch.entries.len());
            
            // Calculate compression ratio if compression is enabled
            if config.enable_compression {
                // This would be calculated based on actual compression results
                metrics_guard.compression_ratio = 0.7; // Example: 30% compression
            }
            
            // Calculate throughput
            let total_time_s = processing_time.as_secs_f64();
            if total_time_s > 0.0 {
                metrics_guard.throughput_eps = audit_batch.entries.len() as f64 / total_time_s;
            }
        }

        debug!("Processed audit batch {} with {} entries in {:?}", 
               audit_batch.batch_id, audit_batch.entries.len(), processing_time);
    }

    /// Write batch to storage (placeholder implementation)
    async fn write_batch_to_storage(
        _config: &AuditConfig,
        batch: &AuditBatch,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // In a real implementation, this would write to:
        // - Database
        // - File system
        // - External logging service
        // - SIEM system
        
        debug!("Writing audit batch {} to storage ({} entries)", 
               batch.batch_id, batch.entries.len());

        // Simulate async write operation
        tokio::time::sleep(Duration::from_millis(10)).await;

        Ok(())
    }
}

impl Drop for AuditLogProcessor {
    fn drop(&mut self) {
        // Ensure batch processor is properly shut down
        if let Some(handle) = self.batch_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let config = AuditConfig::default();
        let logger = AsyncAuditLogger::new(config);
        assert!(logger.is_ok());
    }

    #[tokio::test]
    async fn test_simple_audit_logging() {
        let config = AuditConfig::default();
        let logger = AsyncAuditLogger::new(config).unwrap();
        
        let entry = logger.create_entry(
            AuditEventType::Authentication,
            "test_user".to_string(),
            "login".to_string(),
            json!({"success": true}),
        ).await;

        let result = logger.log_event(entry).await;
        assert!(result.is_ok());

        // Give some time for async processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        let metrics = logger.get_metrics().await;
        assert!(metrics.total_entries > 0);
    }

    #[tokio::test]
    async fn test_wallet_scan_logging() {
        let config = AuditConfig::default();
        let logger = AsyncAuditLogger::new(config).unwrap();
        
        let scan_result = json!({
            "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
            "empty_accounts": 5,
            "recoverable_sol": 0.123
        });

        let result = logger.log_wallet_scan(
            "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
            "test_user",
            &scan_result,
            1500, // 1.5ms
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_processing_logging() {
        let config = AuditConfig::default();
        let logger = AsyncAuditLogger::new(config).unwrap();

        let result = logger.log_batch_processing(
            "batch_123",
            100,
            "test_user",
            5_000_000, // 5 seconds
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_security_event_logging() {
        let config = AuditConfig::default();
        let logger = AsyncAuditLogger::new(config).unwrap();

        let details = json!({
            "event": "rate_limit_exceeded",
            "ip": "192.168.1.100",
            "attempts": 100
        });

        let result = logger.log_security_event(
            "rate_limit",
            "unknown_user",
            details,
            Some("192.168.1.100".to_string()),
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let config = AuditConfig {
            batch_size: 10,
            flush_interval: Duration::from_millis(100),
            ..Default::default()
        };
        let logger = AsyncAuditLogger::new(config).unwrap();

        // Log multiple events
        for i in 0..15 {
            let entry = logger.create_entry(
                AuditEventType::System,
                format!("user_{}", i),
                format!("action_{}", i),
                json!({"index": i}),
            ).await;

            logger.log_event(entry).await.unwrap();
        }

        // Wait for batch processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        let metrics = logger.get_metrics().await;
        assert!(metrics.total_entries >= 15);
        assert!(metrics.batches_processed > 0);
    }
}
