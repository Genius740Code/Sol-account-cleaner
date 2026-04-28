pub mod metrics;
pub mod logging;
pub mod monitoring;
pub mod memory_optimizer;
pub mod memory_pool;
pub mod enhanced_memory_manager;
pub mod advanced_buffer_pools;
pub mod gc_scheduler;
pub mod memory_monitor;
pub mod memory_integration;
pub mod enhanced_metrics;
pub mod http2_client;
pub mod hardware_encryption;
pub mod async_audit_logger;
pub mod object_pool;
pub mod validation;
pub mod distributed_rate_limiter;
pub mod security_auditor;
pub mod circuit_breaker;
pub mod cache;

// Re-export specific items to avoid conflicts
pub use metrics::MetricsConfig as MetricsCollectorConfig;
pub use logging::*;
pub use monitoring::*;
pub use memory_optimizer::*;
pub use memory_pool::{MemoryPool, PooledItem, MemoryManager, BufferPool};
pub use enhanced_memory_manager::{EnhancedMemoryManager, MemoryManagerConfig, EnhancedMemoryStats};
pub use advanced_buffer_pools::{AdvancedBufferPool, BufferPoolConfig, BufferPoolStats};
pub use gc_scheduler::{GcScheduler, GcSchedulerConfig, GcSchedulerStats};
pub use memory_monitor::{MemoryMonitor, MemoryMonitorConfig, MemoryStatistics};
pub use memory_integration::{MemoryIntegrationLayer, MemoryIntegrationConfig, ScannerMemoryManager, RpcMemoryManager};
pub use enhanced_metrics::{EnhancedMetricsCollector, EnhancedMetricsConfig, ComprehensiveMetricsSnapshot, DetailedMetrics};
pub use http2_client::Http2Client;
pub use hardware_encryption::{HardwareEncryptionEngine, EncryptionConfig};
pub use async_audit_logger::AsyncAuditLogger;
pub use object_pool::{ObjectPool, PooledObject, MemoryManager as ObjectMemoryManager, PoolConfig, PoolMetrics, MemoryManagerConfig as ObjectMemoryManagerConfig, MemoryUsageStats};
pub use validation::{InputValidator, InputSanitizer};
pub use distributed_rate_limiter::{
    DistributedRateLimiter, EnhancedRateLimiter, RateLimiterConfig, 
    RateLimitRequest, RateLimiterStats, ComprehensiveRateLimiterStats
};
pub use security_auditor::{
    SecurityAuditor, AuditEntry, OperationResult, AuditorConfig, AuditStatistics
};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerManager, CircuitBreakerConfig, CircuitState,
    CircuitBreakerMetrics, MetricsSnapshot
};
