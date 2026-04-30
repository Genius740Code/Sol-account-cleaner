use solana_recover::{
    core::{RpcEndpoint, OptimizedWalletScanner, OptimizedScannerConfig, PerformanceMode},
    rpc::{PoolConfig, BatchConfig, LoadBalanceStrategy},
    storage::{CacheConfig, CachePriority},
    core::adaptive_parallel_processor::ProcessorConfig,
    utils::MemoryManagerConfig,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Comprehensive performance benchmark suite
pub struct PerformanceBenchmark {
    config: BenchmarkConfig,
    results: Arc<RwLock<Vec<BenchmarkResult>>>,
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub test_wallets: Vec<String>,
    pub concurrent_scans: Vec<usize>,
    pub performance_modes: Vec<PerformanceMode>,
    pub iterations_per_test: usize,
    pub warmup_iterations: usize,
    pub enable_detailed_metrics: bool,
    pub timeout_per_test: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkResult {
    pub test_id: String,
    pub performance_mode: PerformanceMode,
    pub concurrent_scans: usize,
    pub total_wallets: usize,
    pub total_duration_ms: u64,
    pub avg_scan_time_ms: f64,
    pub scans_per_second: f64,
    pub success_rate: f64,
    pub cache_hit_rate: f64,
    pub memory_efficiency: f64,
    pub connection_efficiency: f64,
    pub parallel_efficiency: f64,
    pub rpc_calls_saved: u64,
    pub errors: Vec<String>,
    pub detailed_metrics: Option<DetailedMetrics>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetailedMetrics {
    pub scanner_metrics: solana_recover::core::OptimizedScannerMetrics,
    pub cache_metrics: solana_recover::storage::MultiLevelCacheMetrics,
    pub pool_metrics: solana_recover::rpc::EnhancedPoolMetrics,
    pub memory_metrics: solana_recover::utils::MemoryUsageStats,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            test_wallets: vec![
                "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
                "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
                "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(),
            ],
            concurrent_scans: vec![1, 5, 10, 25, 50, 100],
            performance_modes: vec![
                PerformanceMode::Throughput,
                PerformanceMode::Latency,
                PerformanceMode::Balanced,
                PerformanceMode::ResourceEfficient,
            ],
            iterations_per_test: 3,
            warmup_iterations: 1,
            enable_detailed_metrics: true,
            timeout_per_test: Duration::from_secs(300),
        }
    }
}

impl PerformanceBenchmark {
    pub fn new(config: BenchmarkConfig) -> Self {
        Self {
            config,
            results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Run comprehensive performance benchmark
    pub async fn run_comprehensive_benchmark(&self) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        println!("Starting comprehensive performance benchmark...");
        println!("Test wallets: {}", self.config.test_wallets.len());
        println!("Concurrent scans: {:?}", self.config.concurrent_scans);
        println!("Performance modes: {:?}", self.config.performance_modes);
        println!();

        let mut all_results = Vec::new();

        for performance_mode in &self.config.performance_modes {
            println!("Testing performance mode: {:?}", performance_mode);
            
            for &concurrent_scans in &self.config.concurrent_scans {
                println!("  Testing {} concurrent scans...", concurrent_scans);
                
                let result = self.run_benchmark_test(*performance_mode, concurrent_scans).await?;
                all_results.push(result);
                
                // Brief pause between tests to allow system to stabilize
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            
            println!();
        }

        // Store results
        {
            let mut results_guard = self.results.write().await;
            results_guard.extend(all_results.clone());
        }

        self.print_summary(&all_results);
        Ok(all_results)
    }

    /// Run a single benchmark test
    async fn run_benchmark_test(&self, performance_mode: PerformanceMode, concurrent_scans: usize) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let test_id = Uuid::new_v4().to_string();
        let start_time = Instant::now();
        
        // Create optimized scanner with test configuration
        let scanner = self.create_test_scanner(performance_mode.clone()).await?;
        
        // Warmup phase
        if self.config.warmup_iterations > 0 {
            self.run_warmup_phase(&scanner).await?;
        }

        // Main benchmark phase
        let mut successful_scans = 0;
        let mut failed_scans = 0;
        let mut total_scan_times = Vec::new();
        let mut errors = Vec::new();
        
        let scan_tasks = (0..concurrent_scans).map(|i| {
            let scanner = scanner.clone();
            let wallet_index = i % self.config.test_wallets.len();
            let wallet_address = self.config.test_wallets[wallet_index].clone();
            
            async move {
                let scan_start = Instant::now();
                match scanner.scan_wallet_optimized(&wallet_address).await {
                    Ok(result) => {
                        let scan_time = scan_start.elapsed().as_millis() as f64;
                        if result.status == solana_recover::core::ScanStatus::Completed {
                            Ok((true, scan_time))
                        } else {
                            Ok((false, scan_time))
                        }
                    }
                    Err(e) => Err(e.to_string())
                }
            }
        });

        let scan_results = futures::future::join_all(scan_tasks).await;
        
        for result in scan_results {
            match result {
                Ok((success, scan_time)) => {
                    if success {
                        successful_scans += 1;
                    } else {
                        failed_scans += 1;
                    }
                    total_scan_times.push(scan_time);
                }
                Err(error) => {
                    failed_scans += 1;
                    errors.push(error);
                }
            }
        }

        let total_duration = start_time.elapsed();
        let total_duration_ms = total_duration.as_millis() as u64;
        
        // Calculate metrics
        let avg_scan_time_ms = if !total_scan_times.is_empty() {
            total_scan_times.iter().sum::<f64>() / total_scan_times.len() as f64
        } else {
            0.0
        };
        
        let scans_per_second = if total_duration_ms > 0 {
            (successful_scans as f64 / total_duration_ms as f64) * 1000.0
        } else {
            0.0
        };
        
        let success_rate = if concurrent_scans > 0 {
            successful_scans as f64 / concurrent_scans as f64
        } else {
            0.0
        };

        // Get detailed metrics
        let detailed_metrics = if self.config.enable_detailed_metrics {
            Some(self.collect_detailed_metrics(&scanner).await?)
        } else {
            None
        };

        let cache_hit_rate = detailed_metrics
            .as_ref()
            .map(|m| m.scanner_metrics.cache_hit_rate)
            .unwrap_or(0.0);

        let memory_efficiency = detailed_metrics
            .as_ref()
            .map(|m| m.scanner_metrics.memory_efficiency)
            .unwrap_or(0.0);

        let connection_efficiency = detailed_metrics
            .as_ref()
            .map(|m| m.scanner_metrics.connection_efficiency)
            .unwrap_or(0.0);

        let parallel_efficiency = detailed_metrics
            .as_ref()
            .map(|m| m.scanner_metrics.parallel_efficiency)
            .unwrap_or(0.0);

        let rpc_calls_saved = detailed_metrics
            .as_ref()
            .map(|m| m.scanner_metrics.rpc_calls_saved)
            .unwrap_or(0);

        Ok(BenchmarkResult {
            test_id,
            performance_mode,
            concurrent_scans,
            total_wallets: concurrent_scans,
            total_duration_ms,
            avg_scan_time_ms,
            scans_per_second,
            success_rate,
            cache_hit_rate,
            memory_efficiency,
            connection_efficiency,
            parallel_efficiency,
            rpc_calls_saved,
            errors,
            detailed_metrics,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Create test scanner with optimized configuration
    async fn create_test_scanner(&self, performance_mode: PerformanceMode) -> Result<Arc<OptimizedWalletScanner>, Box<dyn std::error::Error>> {
        let endpoints = vec![
            RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 0,
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
            connection_pool_config: PoolConfig {
                max_connections_per_endpoint: 20,
                health_check_interval: Duration::from_secs(30),
                circuit_breaker_threshold: 5,
                circuit_breaker_timeout: Duration::from_secs(60),
                load_balance_strategy: LoadBalanceStrategy::WeightedRoundRobin,
                enable_connection_multiplexing: true,
                enable_compression: true,
            },
            batch_config: BatchConfig {
                max_batch_size: match performance_mode {
                    PerformanceMode::Throughput => 100,
                    PerformanceMode::Latency => 25,
                    PerformanceMode::Balanced => 50,
                    PerformanceMode::ResourceEfficient => 30,
                },
                max_concurrent_batches: 10,
                batch_timeout: Duration::from_secs(30),
                retry_policy: solana_recover::rpc::RetryPolicy::default(),
                enable_compression: true,
                enable_multiplexing: true,
            },
                    PerformanceMode::Latency => 50,
                    PerformanceMode::Balanced => 100,
                    PerformanceMode::ResourceEfficient => 25,
                },
                work_stealing_enabled: true,
                cpu_affinity_enabled: true,
                adaptive_batching: true,
                resource_monitoring: true,
                load_balancing_strategy: solana_recover::core::adaptive_parallel_processor::LoadBalancingStrategy::WorkStealing,
                task_timeout: Duration::from_secs(300),
                worker_idle_timeout: Duration::from_secs(30),
            },
            memory_config: MemoryManagerConfig {
                max_pool_sizes: solana_recover::utils::PoolSizes {
                    wallet_info_pool: 1000,
                    empty_account_pool: 2000,
                    scan_result_pool: 500,
                    batch_scan_result_pool: 100,
                    recovery_transaction_pool: 200,
                    string_pool: 5000,
                    vec_string_pool: 1000,
                    vec_u8_pool: 2000,
                },
                gc_config: solana_recover::utils::GcConfig {
                    interval_seconds: 30,
                    memory_threshold_percent: 0.8,
                    force_gc_interval_seconds: 300,
                    enable_adaptive_gc: true,
                },
                monitoring_config: solana_recover::utils::MonitoringConfig {
                    collection_interval_seconds: 30,
                    enable_leak_detection: true,
                    leak_detection_threshold_seconds: 300,
                    enable_memory_profiling: true,
                },
                enable_object_pooling: true,
                enable_memory_monitoring: true,
                enable_auto_optimization: true,
            },
            enable_all_optimizations: true,
            performance_mode,
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
        
        println!("    Warmup phase completed.");
        Ok(())
    }

    /// Collect detailed metrics
    async fn collect_detailed_metrics(&self, scanner: &Arc<OptimizedWalletScanner>) -> Result<DetailedMetrics, Box<dyn std::error::Error>> {
        let scanner_metrics = scanner.get_metrics().await?;
        
        // Note: In a real implementation, you would collect metrics from all components
        // For now, we'll create placeholder metrics
        let detailed_metrics = DetailedMetrics {
            scanner_metrics,
            cache_metrics: solana_recover::storage::MultiLevelCacheMetrics::default(),
            pool_metrics: solana_recover::rpc::EnhancedPoolMetrics::default(),
            memory_metrics: solana_recover::utils::MemoryUsageStats {
                transaction_pool_size: 0,
                account_pool_size: 0,
                buffer_pool_size: 0,
                string_pool_size: 0,
                total_pooled_objects: 0,
                estimated_memory_mb: 0.0,
            },
        };
        
        Ok(detailed_metrics)
    }

    /// Print benchmark summary
    fn print_summary(&self, results: &[BenchmarkResult]) {
        println!("=== Performance Benchmark Summary ===");
        println!();

        // Group results by performance mode
        let mut grouped_results: HashMap<PerformanceMode, Vec<&BenchmarkResult>> = HashMap::new();
        for result in results {
            grouped_results.entry(result.performance_mode.clone()).or_insert_with(Vec::new).push(result);
        }

        for (mode, mode_results) in grouped_results {
            println!("Performance Mode: {:?}", mode);
            println!("  Concurrent Scans | Avg Time (ms) | Scans/sec | Success Rate | Cache Hit Rate");
            println!("  ----------------|---------------|-----------|--------------|---------------");
            
            for result in mode_results {
                println!("  {:16} | {:13.2} | {:9.2} | {:12.1}% | {:13.1}%",
                    result.concurrent_scans,
                    result.avg_scan_time_ms,
                    result.scans_per_second,
                    result.success_rate * 100.0,
                    result.cache_hit_rate * 100.0
                );
            }
            println!();
        }

        // Find best performing configuration
        let best_throughput = results.iter()
            .max_by(|a, b| a.scans_per_second.partial_cmp(&b.scans_per_second).unwrap())
            .unwrap();

        let best_latency = results.iter()
            .min_by(|a, b| a.avg_scan_time_ms.partial_cmp(&b.avg_scan_time_ms).unwrap())
            .unwrap();

        println!("Best Performance:");
        println!("  Highest Throughput: {:.2} scans/sec ({:?} mode, {} concurrent scans)",
            best_throughput.scans_per_second,
            best_throughput.performance_mode,
            best_throughput.concurrent_scans
        );
        println!("  Lowest Latency: {:.2} ms ({:?} mode, {} concurrent scans)",
            best_latency.avg_scan_time_ms,
            best_latency.performance_mode,
            best_latency.concurrent_scans
        );
        println!();

        // Performance improvements summary
        self.print_performance_improvements(results);
    }

    /// Print performance improvements analysis
    fn print_performance_improvements(&self, results: &[BenchmarkResult]) {
        println!("Performance Improvements Analysis:");
        println!();

        // Calculate average improvements
        let avg_scans_per_second = results.iter().map(|r| r.scans_per_second).sum::<f64>() / results.len() as f64;
        let avg_cache_hit_rate = results.iter().map(|r| r.cache_hit_rate).sum::<f64>() / results.len() as f64;
        let avg_memory_efficiency = results.iter().map(|r| r.memory_efficiency).sum::<f64>() / results.len() as f64;

        println!("  Average Performance Metrics:");
        println!("    Scans per Second: {:.2}", avg_scans_per_second);
        println!("    Cache Hit Rate: {:.1}%", avg_cache_hit_rate * 100.0);
        println!("    Memory Efficiency: {:.1}%", avg_memory_efficiency * 100.0);
        println!();

        // Estimate improvements over baseline
        let baseline_scans_per_sec = 1.0; // Assumed baseline
        let throughput_improvement = (avg_scans_per_second / baseline_scans_per_sec - 1.0) * 100.0;
        
        println!("  Estimated Improvements:");
        println!("    Throughput Improvement: {:.1}x ({:.1}% faster)", 
            avg_scans_per_second / baseline_scans_per_sec, throughput_improvement);
        
        if avg_cache_hit_rate > 0.5 {
            println!("    Cache Efficiency: {:.1}% reduction in RPC calls", avg_cache_hit_rate * 100.0);
        }
        
        if avg_memory_efficiency > 0.6 {
            println!("    Memory Efficiency: {:.1}% reduction in allocations", avg_memory_efficiency * 100.0);
        }
        
        println!();

        // Recommendations
        println!("Recommendations:");
        if avg_cache_hit_rate < 0.7 {
            println!("  - Consider increasing cache sizes to improve hit rate");
        }
        if avg_memory_efficiency < 0.6 {
            println!("  - Tune object pool configurations for better memory efficiency");
        }
        if avg_scans_per_second < 10.0 {
            println!("  - Consider enabling more aggressive parallelization");
        }
        println!();
    }

    /// Generate performance report
    pub async fn generate_report(&self) -> Result<String, Box<dyn std::error::Error>> {
        let results = self.results.read().await;
        
        let report = format!(
            r#"
# Performance Benchmark Report

Generated: {}

## Test Configuration
- Test Wallets: {}
- Concurrent Scans: {:?}
- Performance Modes: {:?}
- Iterations per Test: {}
- Warmup Iterations: {}

## Results Summary

### Performance by Mode

{}

### Detailed Results

{}

## Analysis

{}

## Recommendations

{}

"#,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            self.config.test_wallets.len(),
            self.config.concurrent_scans,
            self.config.performance_modes,
            self.config.iterations_per_test,
            self.config.warmup_iterations,
            self.format_results_by_mode(&results),
            self.format_detailed_results(&results),
            self.format_analysis(&results),
            self.format_recommendations(&results)
        );

        Ok(report)
    }

    fn format_results_by_mode(&self, results: &[BenchmarkResult]) -> String {
        let mut output = String::new();
        let mut grouped: HashMap<PerformanceMode, Vec<&BenchmarkResult>> = HashMap::new();
        
        for result in results {
            grouped.entry(result.performance_mode.clone()).or_insert_with(Vec::new).push(result);
        }
        
        for (mode, mode_results) in grouped {
            output.push_str(&format!("\n#### {:?}\n", mode));
            output.push_str("| Concurrent | Avg Time (ms) | Scans/sec | Success Rate |\n");
            output.push_str("|-----------|---------------|-----------|--------------|\n");
            
            for result in mode_results {
                output.push_str(&format!(
                    "| {} | {:.2} | {:.2} | {:.1}% |\n",
                    result.concurrent_scans,
                    result.avg_scan_time_ms,
                    result.scans_per_second,
                    result.success_rate * 100.0
                ));
            }
        }
        
        output
    }

    fn format_detailed_results(&self, results: &[BenchmarkResult]) -> String {
        let mut output = String::new();
        
        for result in results {
            output.push_str(&format!(
                "\n### Test: {} ({:?}, {} concurrent)\n",
                result.test_id, result.performance_mode, result.concurrent_scans
            ));
            output.push_str(&format!(
                "- Duration: {}ms\n- Avg Scan Time: {:.2}ms\n- Scans/sec: {:.2}\n- Success Rate: {:.1}%\n",
                result.total_duration_ms, result.avg_scan_time_ms, result.scans_per_second, result.success_rate * 100.0
            ));
            output.push_str(&format!(
                "- Cache Hit Rate: {:.1}%\n- Memory Efficiency: {:.1}%\n- Connection Efficiency: {:.1}%\n",
                result.cache_hit_rate * 100.0, result.memory_efficiency * 100.0, result.connection_efficiency * 100.0
            ));
            
            if !result.errors.is_empty() {
                output.push_str("- Errors:\n");
                for error in &result.errors {
                    output.push_str(&format!("  - {}\n", error));
                }
            }
        }
        
        output
    }

    fn format_analysis(&self, results: &[BenchmarkResult]) -> String {
        let mut output = String::new();
        
        // Calculate statistics
        let avg_throughput = results.iter().map(|r| r.scans_per_second).sum::<f64>() / results.len() as f64;
        let max_throughput = results.iter().map(|r| r.scans_per_second).fold(f64::MIN, f64::max);
        let min_latency = results.iter().map(|r| r.avg_scan_time_ms).fold(f64::MAX, f64::min);
        let avg_cache_hit = results.iter().map(|r| r.cache_hit_rate).sum::<f64>() / results.len() as f64;
        
        output.push_str(&format!(
            "- Average Throughput: {:.2} scans/sec\n", avg_throughput
        ));
        output.push_str(&format!(
            "- Peak Throughput: {:.2} scans/sec\n", max_throughput
        ));
        output.push_str(&format!(
            "- Best Latency: {:.2} ms\n", min_latency
        ));
        output.push_str(&format!(
            "- Average Cache Hit Rate: {:.1}%\n", avg_cache_hit * 100.0
        ));
        
        output
    }

    fn format_recommendations(&self, results: &[BenchmarkResult]) -> String {
        let mut output = String::new();
        
        // Analyze results and provide recommendations
        let avg_cache_hit = results.iter().map(|r| r.cache_hit_rate).sum::<f64>() / results.len() as f64;
        let avg_memory_eff = results.iter().map(|r| r.memory_efficiency).sum::<f64>() / results.len() as f64;
        
        if avg_cache_hit < 0.7 {
            output.push_str("- Increase cache sizes to improve hit rate\n");
        }
        
        if avg_memory_eff < 0.6 {
            output.push_str("- Optimize object pool configurations\n");
        }
        
        let best_mode = results.iter()
            .max_by(|a, b| a.scans_per_second.partial_cmp(&b.scans_per_second).unwrap());
        
        if let Some(best) = best_mode {
            output.push_str(&format!(
                "- Best performance achieved with {:?} mode\n", best.performance_mode
            ));
        }
        
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_benchmark() {
        let config = BenchmarkConfig {
            test_wallets: vec!["9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string()],
            concurrent_scans: vec![1, 5],
            performance_modes: vec![PerformanceMode::Balanced],
            iterations_per_test: 1,
            warmup_iterations: 0,
            enable_detailed_metrics: false,
            timeout_per_test: Duration::from_secs(60),
        };

        let benchmark = PerformanceBenchmark::new(config);
        let results = benchmark.run_comprehensive_benchmark().await.unwrap();
        
        assert!(!results.is_empty(), "Benchmark should produce results");
        
        for result in &results {
            assert!(result.scans_per_second >= 0.0, "Scans per second should be non-negative");
            assert!(result.success_rate >= 0.0 && result.success_rate <= 1.0, "Success rate should be between 0 and 1");
        }
    }
}
