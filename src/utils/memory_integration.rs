use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{error};
use serde::{Serialize, Deserialize};
use crate::core::types::{WalletInfo, EmptyAccount, ScanResult, BatchScanResult};

use super::enhanced_memory_manager::{EnhancedMemoryManager, MemoryManagerConfig};
use super::advanced_buffer_pools::{AdvancedBufferPool, BufferPoolConfig};
use super::gc_scheduler::GcScheduler;
use super::memory_monitor::{MemoryMonitor, MemoryMonitorConfig};

/// Memory integration layer that connects enhanced memory management with existing components
#[derive(Debug)]
pub struct MemoryIntegrationLayer {
    /// Enhanced memory manager
    memory_manager: Arc<EnhancedMemoryManager>,
    
    /// Advanced buffer pool
    buffer_pool: Arc<AdvancedBufferPool>,
    
    /// GC scheduler
    gc_scheduler: Arc<GcScheduler>,
    
    /// Memory monitor
    memory_monitor: Arc<MemoryMonitor>,
    
    /// Integration configuration
    config: MemoryIntegrationConfig,
    
    /// Integration statistics
    stats: Arc<RwLock<IntegrationStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIntegrationConfig {
    /// Enable object pooling for scanner operations
    pub enable_scanner_pooling: bool,
    
    /// Enable object pooling for RPC operations
    pub enable_rpc_pooling: bool,
    
    /// Enable buffer pooling for network operations
    pub enable_buffer_pooling: bool,
    
    /// Enable automatic GC scheduling
    pub enable_auto_gc: bool,
    
    /// Enable memory monitoring
    pub enable_monitoring: bool,
    
    /// Integration-specific settings
    pub scanner_config: ScannerMemoryConfig,
    pub rpc_config: RpcMemoryConfig,
    pub buffer_config: BufferIntegrationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerMemoryConfig {
    /// Pool size for wallet info objects
    pub wallet_info_pool_size: usize,
    
    /// Pool size for empty account objects
    pub empty_account_pool_size: usize,
    
    /// Pool size for scan result objects
    pub scan_result_pool_size: usize,
    
    /// Pool size for batch scan result objects
    pub batch_scan_result_pool_size: usize,
    
    /// Enable memory tracking for scan operations
    pub enable_scan_tracking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcMemoryConfig {
    /// Pool size for request buffers
    pub request_buffer_pool_size: usize,
    
    /// Pool size for response buffers
    pub response_buffer_pool_size: usize,
    
    /// Pool size for account data buffers
    pub account_data_buffer_pool_size: usize,
    
    /// Enable memory tracking for RPC operations
    pub enable_rpc_tracking: bool,
    
    /// Buffer size limits
    pub max_request_size: usize,
    pub max_response_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferIntegrationConfig {
    /// Enable size-tiered buffer pools
    pub enable_size_tiered_pools: bool,
    
    /// Enable specialized RPC buffers
    pub enable_rpc_specialized_buffers: bool,
    
    /// Buffer cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
    
    /// Maximum buffer age in seconds
    pub max_buffer_age_seconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrationStats {
    /// Total scanner operations using pooled objects
    pub scanner_pool_operations: u64,
    
    /// Total RPC operations using pooled objects
    pub rpc_pool_operations: u64,
    
    /// Total buffer operations
    pub buffer_operations: u64,
    
    /// Memory saved through pooling (estimated)
    pub memory_saved_bytes: usize,
    
    /// GC operations triggered by integration
    pub integration_gc_triggers: u64,
    
    /// Memory alerts from integration
    pub integration_alerts: u64,
    
    /// Performance improvements
    pub scanner_time_saved_ms: u64,
    pub rpc_time_saved_ms: u64,
}

impl Default for MemoryIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_scanner_pooling: true,
            enable_rpc_pooling: true,
            enable_buffer_pooling: true,
            enable_auto_gc: true,
            enable_monitoring: true,
            scanner_config: ScannerMemoryConfig::default(),
            rpc_config: RpcMemoryConfig::default(),
            buffer_config: BufferIntegrationConfig::default(),
        }
    }
}

impl Default for ScannerMemoryConfig {
    fn default() -> Self {
        Self {
            wallet_info_pool_size: 10000,
            empty_account_pool_size: 50000,
            scan_result_pool_size: 10000,
            batch_scan_result_pool_size: 1000,
            enable_scan_tracking: true,
        }
    }
}

impl Default for RpcMemoryConfig {
    fn default() -> Self {
        Self {
            request_buffer_pool_size: 1000,
            response_buffer_pool_size: 1000,
            account_data_buffer_pool_size: 2000,
            enable_rpc_tracking: true,
            max_request_size: 64 * 1024,  // 64KB
            max_response_size: 1024 * 1024, // 1MB
        }
    }
}

impl Default for BufferIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_size_tiered_pools: true,
            enable_rpc_specialized_buffers: true,
            cleanup_interval_seconds: 60,
            max_buffer_age_seconds: 300,
        }
    }
}

impl MemoryIntegrationLayer {
    pub fn new() -> Arc<Self> {
        Self::with_config(MemoryIntegrationConfig::default())
    }
    
    pub fn with_config(config: MemoryIntegrationConfig) -> Arc<Self> {
        // Configure memory manager based on integration needs
        let memory_config = Self::build_memory_manager_config(&config);
        let memory_manager = EnhancedMemoryManager::with_config(memory_config);
        
        // Configure buffer pool
        let buffer_config = Self::build_buffer_pool_config(&config);
        let buffer_pool = AdvancedBufferPool::with_config(buffer_config);
        
        // Configure GC scheduler
        let gc_scheduler = GcScheduler::new();
        
        // Configure memory monitor
        let monitor_config = Self::build_monitor_config(&config);
        let memory_monitor = MemoryMonitor::with_config(monitor_config);
        
        let integration = Arc::new(Self {
            memory_manager,
            buffer_pool,
            gc_scheduler,
            memory_monitor,
            config,
            stats: Arc::new(RwLock::new(IntegrationStats::default())),
        });
        
        // Start background services
        integration.start_background_services();
        
        integration
    }
    
    fn build_memory_manager_config(config: &MemoryIntegrationConfig) -> MemoryManagerConfig {
        let mut memory_config = MemoryManagerConfig::default();
        
        // Adjust pool sizes based on integration config
        memory_config.max_pool_sizes.wallet_info_pool = config.scanner_config.wallet_info_pool_size;
        memory_config.max_pool_sizes.empty_account_pool = config.scanner_config.empty_account_pool_size;
        memory_config.max_pool_sizes.scan_result_pool = config.scanner_config.scan_result_pool_size;
        memory_config.max_pool_sizes.batch_scan_result_pool = config.scanner_config.batch_scan_result_pool_size;
        
        // Enable/disable features based on integration config
        memory_config.enable_object_pooling = config.enable_scanner_pooling || config.enable_rpc_pooling;
        memory_config.enable_memory_monitoring = config.enable_monitoring;
        memory_config.enable_auto_optimization = config.enable_auto_gc;
        
        memory_config
    }
    
    fn build_buffer_pool_config(config: &MemoryIntegrationConfig) -> BufferPoolConfig {
        let mut buffer_config = BufferPoolConfig::default();
        
        // Adjust pool sizes based on RPC config
        buffer_config.pool_sizes.rpc_request_pool_size = config.rpc_config.request_buffer_pool_size;
        buffer_config.pool_sizes.rpc_response_pool_size = config.rpc_config.response_buffer_pool_size;
        buffer_config.pool_sizes.account_data_pool_size = config.rpc_config.account_data_buffer_pool_size;
        
        // Configure cleanup settings
        buffer_config.cleanup_interval_seconds = config.buffer_config.cleanup_interval_seconds;
        buffer_config.max_buffer_age_seconds = config.buffer_config.max_buffer_age_seconds;
        
        // Enable/disable features
        buffer_config.enable_stats_collection = config.enable_monitoring;
        
        buffer_config
    }
    
    fn build_monitor_config(config: &MemoryIntegrationConfig) -> MemoryMonitorConfig {
        let mut monitor_config = MemoryMonitorConfig::default();
        
        // Enable monitoring features based on integration config
        monitor_config.enable_profiling = config.enable_monitoring;
        monitor_config.enable_leak_detection = config.enable_monitoring;
        monitor_config.enable_performance_monitoring = config.enable_monitoring;
        monitor_config.enable_real_time_events = config.enable_monitoring;
        
        monitor_config
    }
    
    fn start_background_services(self: &Arc<Self>) {
        if self.config.enable_monitoring {
            let monitor = self.memory_monitor.clone();
            tokio::spawn(async move {
                if let Err(e) = monitor.start_monitoring().await {
                    error!("Failed to start memory monitoring: {}", e);
                }
            });
        }
        
        // Subscribe to memory events for integration tracking
        if self.config.enable_monitoring {
            let integration = self.clone();
            let mut receiver = self.memory_monitor.subscribe_events();
            
            tokio::spawn(async move {
                while let Ok(event) = receiver.recv().await {
                    integration.handle_memory_event(event).await;
                }
            });
        }
    }
    
    async fn handle_memory_event(&self, event: super::memory_monitor::MemoryEvent) {
        use super::memory_monitor::{MemoryEventType, EventSeverity};
        
        match event.event_type {
            MemoryEventType::Allocation { size, pool } => {
                if pool.is_some() {
                    let mut stats = self.stats.write();
                    stats.buffer_operations += 1;
                    stats.memory_saved_bytes += size / 2; // Estimate 50% savings
                }
            }
            MemoryEventType::GcCollection { .. } => {
                let mut stats = self.stats.write();
                stats.integration_gc_triggers += 1;
            }
            MemoryEventType::MemoryPressure { .. } => {
                if event.severity >= EventSeverity::Warning {
                    let mut stats = self.stats.write();
                    stats.integration_alerts += 1;
                }
            }
            _ => {}
        }
    }
    
    /// Get enhanced memory manager for scanner integration
    pub fn get_memory_manager(&self) -> Arc<EnhancedMemoryManager> {
        self.memory_manager.clone()
    }
    
    /// Get advanced buffer pool for RPC integration
    pub fn get_buffer_pool(&self) -> Arc<AdvancedBufferPool> {
        self.buffer_pool.clone()
    }
    
    /// Get GC scheduler
    pub fn get_gc_scheduler(&self) -> Arc<GcScheduler> {
        self.gc_scheduler.clone()
    }
    
    /// Get memory monitor
    pub fn get_memory_monitor(&self) -> Arc<MemoryMonitor> {
        self.memory_monitor.clone()
    }
    
    /// Get integration statistics
    pub fn get_integration_stats(&self) -> IntegrationStats {
        self.stats.read().clone()
    }
    
    /// Create a scanner-aware memory manager
    pub fn create_scanner_memory_manager(&self) -> ScannerMemoryManager {
        ScannerMemoryManager::new(
            self.memory_manager.clone(),
            self.config.scanner_config.clone(),
            self.stats.clone(),
        )
    }
    
    /// Create an RPC-aware memory manager
    pub fn create_rpc_memory_manager(&self) -> RpcMemoryManager {
        RpcMemoryManager::new(
            self.memory_manager.clone(),
            self.buffer_pool.clone(),
            self.config.rpc_config.clone(),
            self.stats.clone(),
        )
    }
    
    /// Generate comprehensive integration report
    pub async fn generate_integration_report(&self) -> serde_json::Value {
        let stats = self.get_integration_stats();
        let memory_report = self.memory_manager.get_comprehensive_report().await;
        let buffer_report = self.buffer_pool.get_performance_report();
        let gc_report = self.gc_scheduler.get_comprehensive_report();
        let monitor_report = self.memory_monitor.generate_report();
        
        serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "integration_stats": format!("{:?}", stats),
            "memory_manager_report": memory_report,
            "buffer_pool_report": buffer_report,
            "gc_scheduler_report": gc_report,
            "memory_monitor_report": monitor_report,
            "config": format!("{:?}", self.config),
            "recommendations": self.generate_integration_recommendations(&stats),
        })
    }
    
    fn generate_integration_recommendations(&self, stats: &IntegrationStats) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if stats.scanner_pool_operations == 0 {
            recommendations.push("Scanner object pooling is not being utilized. Consider enabling scanner pooling for better performance.".to_string());
        }
        
        if stats.rpc_pool_operations == 0 {
            recommendations.push("RPC object pooling is not being utilized. Consider enabling RPC pooling for better performance.".to_string());
        }
        
        if stats.memory_saved_bytes < 1024 * 1024 { // Less than 1MB saved
            recommendations.push("Low memory savings detected. Consider increasing pool sizes or optimizing allocation patterns.".to_string());
        }
        
        if stats.integration_alerts > 10 {
            recommendations.push("High number of memory alerts detected. Review memory usage patterns and consider optimization.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("Memory integration is performing optimally. No immediate action required.".to_string());
        }
        
        recommendations
    }
}

/// Scanner-aware memory manager that provides specialized methods for scanner operations
#[derive(Debug)]
pub struct ScannerMemoryManager {
    memory_manager: Arc<EnhancedMemoryManager>,
    config: ScannerMemoryConfig,
    stats: Arc<RwLock<IntegrationStats>>,
}

impl ScannerMemoryManager {
    fn new(
        memory_manager: Arc<EnhancedMemoryManager>,
        config: ScannerMemoryConfig,
        stats: Arc<RwLock<IntegrationStats>>,
    ) -> Self {
        Self {
            memory_manager,
            config,
            stats,
        }
    }
    
    /// Acquire a wallet info object for scanning
    pub fn acquire_wallet_info(&self) -> crate::utils::memory_pool::PooledItem<WalletInfo> {
        let wallet_info = self.memory_manager.acquire_wallet_info();
        
        if self.config.enable_scan_tracking {
            let mut stats = self.stats.write();
            stats.scanner_pool_operations += 1;
        }
        
        wallet_info
    }
    
    /// Acquire an empty account object for scanning
    pub fn acquire_empty_account(&self) -> crate::utils::memory_pool::PooledItem<EmptyAccount> {
        let empty_account = self.memory_manager.acquire_empty_account();
        
        if self.config.enable_scan_tracking {
            let mut stats = self.stats.write();
            stats.scanner_pool_operations += 1;
        }
        
        empty_account
    }
    
    /// Acquire a scan result object
    pub fn acquire_scan_result(&self) -> crate::utils::memory_pool::PooledItem<ScanResult> {
        let scan_result = self.memory_manager.acquire_scan_result();
        
        if self.config.enable_scan_tracking {
            let mut stats = self.stats.write();
            stats.scanner_pool_operations += 1;
        }
        
        scan_result
    }
    
    /// Acquire a batch scan result object
    pub fn acquire_batch_scan_result(&self) -> crate::utils::memory_pool::PooledItem<BatchScanResult> {
        let batch_result = self.memory_manager.acquire_batch_scan_result();
        
        if self.config.enable_scan_tracking {
            let mut stats = self.stats.write();
            stats.scanner_pool_operations += 1;
        }
        
        batch_result
    }
    
    /// Get scanner-specific memory statistics
    pub fn get_scanner_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "scanner_pool_operations": self.stats.read().scanner_pool_operations,
            "scanner_config": format!("{:?}", self.config),
            "memory_manager_stats": self.memory_manager.get_memory_stats(),
        })
    }
}

/// RPC-aware memory manager that provides specialized methods for RPC operations
#[derive(Debug)]
pub struct RpcMemoryManager {
    memory_manager: Arc<EnhancedMemoryManager>,
    buffer_pool: Arc<AdvancedBufferPool>,
    config: RpcMemoryConfig,
    stats: Arc<RwLock<IntegrationStats>>,
}

impl RpcMemoryManager {
    fn new(
        memory_manager: Arc<EnhancedMemoryManager>,
        buffer_pool: Arc<AdvancedBufferPool>,
        config: RpcMemoryConfig,
        stats: Arc<RwLock<IntegrationStats>>,
    ) -> Self {
        Self {
            memory_manager,
            buffer_pool,
            config,
            stats,
        }
    }
    
    /// Acquire a buffer for RPC request
    pub fn acquire_request_buffer(&self, size: usize) -> super::memory_pool::PooledItem<Vec<u8>> {
        let buffer_size = size.min(self.config.max_request_size);
        let buffer = self.buffer_pool.get_buffer(buffer_size);
        
        if self.config.enable_rpc_tracking {
            let mut stats = self.stats.write();
            stats.rpc_pool_operations += 1;
            stats.memory_saved_bytes += buffer_size / 2; // Estimate 50% savings
        }
        
        buffer
    }
    
    /// Acquire a specialized RPC request buffer
    pub fn acquire_rpc_request_buffer(&self, method: &str, request_id: &str) -> super::memory_pool::PooledItem<super::advanced_buffer_pools::RpcRequestBuffer> {
        let buffer = self.buffer_pool.get_rpc_request_buffer(method, request_id);
        
        if self.config.enable_rpc_tracking {
            let mut stats = self.stats.write();
            stats.rpc_pool_operations += 1;
        }
        
        buffer
    }
    
    /// Acquire a specialized RPC response buffer
    pub fn acquire_rpc_response_buffer(&self, request_id: &str) -> super::memory_pool::PooledItem<super::advanced_buffer_pools::RpcResponseBuffer> {
        let buffer = self.buffer_pool.get_rpc_response_buffer(request_id);
        
        if self.config.enable_rpc_tracking {
            let mut stats = self.stats.write();
            stats.rpc_pool_operations += 1;
        }
        
        buffer
    }
    
    /// Acquire a specialized account data buffer
    pub fn acquire_account_data_buffer(&self, address: &str, slot: u64) -> super::memory_pool::PooledItem<super::advanced_buffer_pools::AccountDataBuffer> {
        let buffer = self.buffer_pool.get_account_data_buffer(address, slot);
        
        if self.config.enable_rpc_tracking {
            let mut stats = self.stats.write();
            stats.rpc_pool_operations += 1;
        }
        
        buffer
    }
    
    /// Get RPC-specific memory statistics
    pub fn get_rpc_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "rpc_pool_operations": self.stats.read().rpc_pool_operations,
            "rpc_config": format!("{:?}", self.config),
            "buffer_pool_stats": self.buffer_pool.get_stats(),
            "memory_manager_stats": self.memory_manager.get_memory_stats(),
        })
    }
}

/// Global memory integration instance
use std::sync::OnceLock;

static GLOBAL_MEMORY_INTEGRATION: OnceLock<Arc<MemoryIntegrationLayer>> = OnceLock::new();

/// Get the global memory integration instance
pub fn get_global_memory_integration() -> Arc<MemoryIntegrationLayer> {
    GLOBAL_MEMORY_INTEGRATION.get_or_init(|| MemoryIntegrationLayer::new()).clone()
}

/// Initialize global memory integration with custom config
pub fn init_global_memory_integration(config: MemoryIntegrationConfig) -> Arc<MemoryIntegrationLayer> {
    let integration = MemoryIntegrationLayer::with_config(config);
    GLOBAL_MEMORY_INTEGRATION.set(integration.clone()).expect("Global memory integration already initialized");
    integration
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_integration_creation() {
        let integration = MemoryIntegrationLayer::new();
        let stats = integration.get_integration_stats();
        
        assert_eq!(stats.scanner_pool_operations, 0);
        assert_eq!(stats.rpc_pool_operations, 0);
    }
    
    #[tokio::test]
    async fn test_scanner_memory_manager() {
        let integration = MemoryIntegrationLayer::new();
        let scanner_manager = integration.create_scanner_memory_manager();
        
        let wallet_info = scanner_manager.acquire_wallet_info();
        assert!(wallet_info.address.is_empty());
        drop(wallet_info);
        
        let stats = integration.get_integration_stats();
        assert_eq!(stats.scanner_pool_operations, 1);
    }
    
    #[tokio::test]
    async fn test_rpc_memory_manager() {
        let integration = MemoryIntegrationLayer::new();
        let rpc_manager = integration.create_rpc_memory_manager();
        
        let buffer = rpc_manager.acquire_request_buffer(1024);
        assert_eq!(buffer.len(), 1024);
        drop(buffer);
        
        let stats = integration.get_integration_stats();
        assert_eq!(stats.rpc_pool_operations, 1);
    }
    
    #[tokio::test]
    async fn test_integration_report() {
        let integration = MemoryIntegrationLayer::new();
        
        // Generate some activity
        let scanner_manager = integration.create_scanner_memory_manager();
        let _wallet_info = scanner_manager.acquire_wallet_info();
        
        let rpc_manager = integration.create_rpc_memory_manager();
        let _buffer = rpc_manager.acquire_request_buffer(512);
        
        let report = integration.generate_integration_report().await;
        
        assert!(report.get("timestamp").is_some());
        assert!(report.get("integration_stats").is_some());
        assert!(report.get("memory_manager_report").is_some());
        assert!(report.get("recommendations").is_some());
    }
    
    #[tokio::test]
    async fn test_global_memory_integration() {
        let global_integration = get_global_memory_integration();
        let stats = global_integration.get_integration_stats();
        
        assert_eq!(stats.scanner_pool_operations, 0);
    }
}
