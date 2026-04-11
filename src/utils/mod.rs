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
