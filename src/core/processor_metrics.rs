use serde::{Deserialize, Serialize};

/// Metrics for batch processor performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorMetrics {
    pub active_scans: usize,
    pub completed_scans: usize,
    pub failed_scans: usize,
    pub average_scan_time_ms: f64,
    pub total_recovered_sol: f64,
    pub cache_hit_rate: f64,
    pub connection_pool_health: f64,
    pub total_wallets_processed: usize,
    pub throughput_wallets_per_second: f64,
}

impl Default for ProcessorMetrics {
    fn default() -> Self {
        Self {
            active_scans: 0,
            completed_scans: 0,
            failed_scans: 0,
            average_scan_time_ms: 0.0,
            total_recovered_sol: 0.0,
            cache_hit_rate: 0.0,
            connection_pool_health: 100.0,
            total_wallets_processed: 0,
            throughput_wallets_per_second: 0.0,
        }
    }
}
