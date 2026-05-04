use solana_recover::{
    core::{RpcEndpoint, OptimizedWalletScanner},
    optimized_scanner::{OptimizedScannerConfig, PerformanceMode},
    rpc::{EnhancedPoolConfig, BatchConfig},
    storage::multi_level_cache::CacheConfig,
    core::adaptive_parallel_processor::ProcessorConfig,
    utils::enhanced_memory_manager::MemoryManagerConfig,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Comprehensive performance benchmark suite
pub struct PerformanceBenchmark {
    config: BenchmarkConfig,
    results: Arc<RwLock<BenchmarkResults>>,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub test_wallets: Vec<String>,
    pub warmup_iterations: usize,
    pub benchmark_iterations: usize,
    pub concurrent_requests: usize,
    pub performance_mode: PerformanceMode,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            test_wallets: vec![
                "test_wallet_1".to_string(),
                "test_wallet_2".to_string(),
                "test_wallet_3".to_string(),
            ],
            warmup_iterations: 10,
            benchmark_iterations: 100,
            concurrent_requests: 10,
            performance_mode: PerformanceMode::Balanced,
        }
    }
}

/// Benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub total_duration: Duration,
    pub total_operations: usize,
    pub operations_per_second: f64,
    pub average_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub error_rate: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

/// Individual benchmark result
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub id: String,
    pub timestamp: Instant,
    pub operation_type: String,
    pub duration: Duration,
    pub success: bool,
    pub error_message: Option<String>,
    pub performance_mode: PerformanceMode,
}

impl PerformanceBenchmark {
    /// Create a new performance benchmark
    pub fn new(config: BenchmarkConfig) -> Self {
        Self {
            config,
            results: Arc::new(RwLock::new(BenchmarkResults {
                total_duration: Duration::from_secs(0),
                total_operations: 0,
                operations_per_second: 0.0,
                average_latency: Duration::from_secs(0),
                p95_latency: Duration::from_secs(0),
                p99_latency: Duration::from_secs(0),
                error_rate: 0.0,
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
            })),
        }
    }

    /// Create optimized scanner for benchmarking
    pub async fn create_scanner(&self) -> Result<Arc<OptimizedWalletScanner>, Box<dyn std::error::Error>> {
        let performance_mode = self.config.performance_mode.clone();
        
        let endpoints = vec![
            RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 1,
                rate_limit_rps: 100,
                timeout_ms: 30000,
                healthy: true,
            },
            RpcEndpoint {
                url: "https://solana-api.projectserum.com".to_string(),
                priority: 1,
                rate_limit_rps: 100,
                timeout_ms: 30000,
                healthy: true,
            },
        ];

        let config = OptimizedScannerConfig {
            connection_pool_config: EnhancedPoolConfig::default(),
            batch_config: BatchConfig::default(),
            cache_config: CacheConfig::default(),
            processor_config: ProcessorConfig::default(),
            memory_config: MemoryManagerConfig::default(),
            enable_all_optimizations: true,
            performance_mode: performance_mode.clone(),
            enable_predictive_prefetch: true,
            enable_connection_multiplexing: true,
            enable_smart_batching: true,
            enable_fast_path: true,
            max_concurrent_scans: 100,
            scan_timeout: Duration::from_secs(300),
            prefetch_window_size: 1000,
            batch_size_multiplier: 1.5,
        };

        Ok(Arc::new(OptimizedWalletScanner::new(endpoints, config)?))
    }

    /// Run warmup phase
    async fn run_warmup_phase(&self, scanner: &Arc<OptimizedWalletScanner>) -> Result<(), Box<dyn std::error::Error>> {
        println!("    Running warmup phase...");
        
        for i in 0..self.config.warmup_iterations {
            let wallet_index = i % self.config.test_wallets.len();
            let wallet_address = self.config.test_wallets[wallet_index].clone();
            
            if let Err(e) = scanner.scan_wallet_optimized(&wallet_address).await {
                println!("    Warmup scan {} failed: {}", i + 1, e);
            }
        }
        
        Ok(())
    }

    /// Run benchmark phase
    async fn run_benchmark_phase(&self, scanner: &Arc<OptimizedWalletScanner>) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        println!("    Running benchmark phase...");
        
        let mut results = Vec::new();
        let start_time = Instant::now();
        
        for i in 0..self.config.benchmark_iterations {
            let wallet_index = i % self.config.test_wallets.len();
            let wallet_address = self.config.test_wallets[wallet_index].clone();
            
            let operation_start = Instant::now();
            let result = scanner.scan_wallet_optimized(&wallet_address).await;
            let duration = operation_start.elapsed();
            
            let benchmark_result = BenchmarkResult {
                id: Uuid::new_v4().to_string(),
                timestamp: operation_start,
                operation_type: "wallet_scan".to_string(),
                duration,
                success: result.is_ok(),
                error_message: result.err().map(|e| e.to_string()),
                performance_mode: self.config.performance_mode.clone(),
            };
            
            results.push(benchmark_result);
        }
        
        let total_duration = start_time.elapsed();
        println!("    Benchmark completed in {:?}", total_duration);
        
        Ok(results)
    }

    /// Run complete benchmark
    pub async fn run_benchmark(&self) -> Result<BenchmarkResults, Box<dyn std::error::Error>> {
        println!("Starting performance benchmark...");
        println!("  Mode: {:?}", self.config.performance_mode);
        println!("  Iterations: {}", self.config.benchmark_iterations);
        println!("  Concurrent requests: {}", self.config.concurrent_requests);
        
        let scanner = self.create_scanner().await?;
        
        // Run warmup
        self.run_warmup_phase(&scanner).await?;
        
        // Run benchmark
        let results = self.run_benchmark_phase(&scanner).await?;
        
        // Calculate metrics
        let benchmark_results = self.calculate_metrics(&results).await?;
        
        // Store results
        {
            let mut results_lock = self.results.write().await;
            *results_lock = benchmark_results.clone();
        }
        
        println!("Benchmark completed successfully!");
        println!("  Operations per second: {:.2}", benchmark_results.operations_per_second);
        println!("  Average latency: {:?}", benchmark_results.average_latency);
        println!("  P95 latency: {:?}", benchmark_results.p95_latency);
        println!("  P99 latency: {:?}", benchmark_results.p99_latency);
        println!("  Error rate: {:.2}%", benchmark_results.error_rate * 100.0);
        
        Ok(benchmark_results)
    }

    /// Calculate benchmark metrics
    async fn calculate_metrics(&self, results: &[BenchmarkResult]) -> Result<BenchmarkResults, Box<dyn std::error::Error>> {
        if results.is_empty() {
            return Ok(BenchmarkResults {
                total_duration: Duration::from_secs(0),
                total_operations: 0,
                operations_per_second: 0.0,
                average_latency: Duration::from_secs(0),
                p95_latency: Duration::from_secs(0),
                p99_latency: Duration::from_secs(0),
                error_rate: 0.0,
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
            });
        }

        let successful_operations: Vec<_> = results.iter()
            .filter(|r| r.success)
            .collect();
        
        let total_operations = results.len();
        let successful_count = successful_operations.len();
        let error_rate = (total_operations - successful_count) as f64 / total_operations as f64;
        
        let durations: Vec<_> = successful_operations.iter()
            .map(|r| r.duration)
            .collect();
        
        let total_duration = results.iter()
            .map(|r| r.duration)
            .sum::<Duration>();
        
        let average_latency = if !durations.is_empty() {
            total_duration / durations.len() as u32
        } else {
            Duration::from_secs(0)
        };
        
        let operations_per_second = if total_duration.as_secs_f64() > 0.0 {
            total_operations as f64 / total_duration.as_secs_f64()
        } else {
            0.0
        };
        
        // Calculate percentiles
        let mut sorted_durations = durations.clone();
        sorted_durations.sort();
        
        let p95_latency = if !sorted_durations.is_empty() {
            let index = (sorted_durations.len() as f64 * 0.95) as usize;
            sorted_durations.get(index.min(sorted_durations.len() - 1)).copied()
                .unwrap_or(Duration::from_secs(0))
        } else {
            Duration::from_secs(0)
        };
        
        let p99_latency = if !sorted_durations.is_empty() {
            let index = (sorted_durations.len() as f64 * 0.99) as usize;
            sorted_durations.get(index.min(sorted_durations.len() - 1)).copied()
                .unwrap_or(Duration::from_secs(0))
        } else {
            Duration::from_secs(0)
        };
        
        Ok(BenchmarkResults {
            total_duration,
            total_operations,
            operations_per_second,
            average_latency,
            p95_latency,
            p99_latency,
            error_rate,
            memory_usage_mb: 0.0, // TODO: Implement memory tracking
            cpu_usage_percent: 0.0, // TODO: Implement CPU tracking
        })
    }

    /// Get benchmark results
    pub async fn get_results(&self) -> BenchmarkResults {
        self.results.read().await.clone()
    }

    /// Compare performance modes
    pub async fn compare_performance_modes(&self) -> Result<Vec<(PerformanceMode, BenchmarkResults)>, Box<dyn std::error::Error>> {
        let modes = vec![
            PerformanceMode::Throughput,
            PerformanceMode::Latency,
            PerformanceMode::Balanced,
            PerformanceMode::ResourceEfficient,
        ];
        
        let mut results = Vec::new();
        
        for mode in modes {
            println!("Testing mode: {:?}", mode);
            let mut config = self.config.clone();
            config.performance_mode = mode.clone();
            
            let _benchmark = PerformanceBenchmark::new(config);
            let result = _benchmark.run_benchmark().await?;
            results.push((mode, result));
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_benchmark_creation() {
        let config = BenchmarkConfig::default();
        let _benchmark = PerformanceBenchmark::new(config);
        // Test creation succeeds
    }

    #[tokio::test]
    async fn test_benchmark_execution() {
        let config = BenchmarkConfig {
            benchmark_iterations: 5,
            warmup_iterations: 2,
            ..Default::default()
        };
        
        let _benchmark = PerformanceBenchmark::new(config);
        let results = _benchmark.run_benchmark().await;
        
        assert!(results.is_ok());
        let benchmark_results = results.unwrap();
        assert!(benchmark_results.total_operations > 0);
    }
}
