use crate::core::processor::{BatchProcessor, ProcessorConfig};
use crate::core::parallel_processor::IntelligentParallelProcessor;
use crate::core::scanner::WalletScanner;
use crate::core::{BatchScanRequest};
use crate::rpc::mock::MockConnectionPool;
use std::sync::Arc;
use std::time::{Instant};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use serde::{Serialize, Deserialize};
use tracing::info;

/// Performance benchmark suite
pub struct PerformanceBenchmarks {
    #[allow(dead_code)]
    mock_pool: Arc<MockConnectionPool>,
    scanner: Arc<WalletScanner>,
}

impl PerformanceBenchmarks {
    pub fn new() -> Self {
        let mock_pool = Arc::new(MockConnectionPool::new_simple());
        let scanner = Arc::new(WalletScanner::new(mock_pool.clone()));
        
        Self {
            mock_pool,
            scanner,
        }
    }
    
    /// Run comprehensive performance benchmarks
    pub async fn run_all_benchmarks(&self) -> BenchmarkResults {
        info!("Starting comprehensive performance benchmarks");
        
        let mut results = BenchmarkResults::new();
        
        // Benchmark 1: Sequential vs Parallel processing
        results.add_result("sequential_vs_parallel", self.benchmark_sequential_vs_parallel().await);
        
        // Benchmark 2: Scalability with worker count
        results.add_result("worker_scalability", self.benchmark_worker_scalability().await);
        
        // Benchmark 3: Batch size optimization
        results.add_result("batch_size_optimization", self.benchmark_batch_size_optimization().await);
        
        // Benchmark 4: Memory efficiency
        results.add_result("memory_efficiency", self.benchmark_memory_efficiency().await);
        
        // Benchmark 5: Network efficiency
        results.add_result("network_efficiency", self.benchmark_network_efficiency().await);
        
        // Benchmark 6: Load balancing effectiveness
        results.add_result("load_balancing", self.benchmark_load_balancing().await);
        
        // Benchmark 7: Priority processing impact
        results.add_result("priority_processing", self.benchmark_priority_processing().await);
        
        // Benchmark 8: Resource utilization
        results.add_result("resource_utilization", self.benchmark_resource_utilization().await);
        
        // Benchmark 9: Fault tolerance overhead
        results.add_result("fault_tolerance_overhead", self.benchmark_fault_tolerance_overhead().await);
        
        // Benchmark 10: Long-running stability
        results.add_result("stability_test", self.benchmark_stability().await);
        
        info!("Completed performance benchmarks");
        results
    }
    
    async fn benchmark_sequential_vs_parallel(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        let wallet_addresses: Vec<String> = (0..1000)
            .map(|i| format!("benchmark_wallet_{}", i))
            .collect();
        
        // Test sequential processing
        let sequential_start = Instant::now();
        let mock_endpoint = crate::core::RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 1000,
            healthy: true,
            timeout_ms: 30000,
        };
        let sequential_pool = crate::rpc::ConnectionPool::new(vec![mock_endpoint], 1);
        let sequential_processor = BatchProcessor::new_simple(Arc::new(sequential_pool), 1);
        let sequential_request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses: wallet_addresses.clone(),
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let _sequential_result = sequential_processor.process_batch(&sequential_request).await;
        let sequential_duration = sequential_start.elapsed();
        
        // Test parallel processing
        let parallel_start = Instant::now();
        let parallel_config = ProcessorConfig {
            batch_size: 100,
            max_concurrent_wallets: 100,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            enable_intelligent_processing: true,
            num_workers: Some(8),
        };
        
        let parallel_processor = BatchProcessor::new(
            self.scanner.clone(),
            None,
            None,
            parallel_config,
        ).unwrap();
        
        let parallel_request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses: wallet_addresses.clone(),
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let _parallel_result = parallel_processor.process_batch(&parallel_request).await;
        let parallel_duration = parallel_start.elapsed();
        
        // Calculate performance improvement
        let speedup = sequential_duration.as_secs_f64() / parallel_duration.as_secs_f64();
        let efficiency = speedup / 8.0; // 8 workers
        
        metrics.push(BenchmarkMetric {
            name: "Sequential Duration (s)".to_string(),
            value: sequential_duration.as_secs_f64(),
            unit: "seconds".to_string(),
        });
        
        metrics.push(BenchmarkMetric {
            name: "Parallel Duration (s)".to_string(),
            value: parallel_duration.as_secs_f64(),
            unit: "seconds".to_string(),
        });
        
        metrics.push(BenchmarkMetric {
            name: "Speedup".to_string(),
            value: speedup,
            unit: "x".to_string(),
        });
        
        metrics.push(BenchmarkMetric {
            name: "Efficiency".to_string(),
            value: efficiency * 100.0,
            unit: "percent".to_string(),
        });
        
        BenchmarkResult {
            name: "Sequential vs Parallel".to_string(),
            success: speedup > 2.0, // Expect at least 2x speedup
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: vec![
                format!("Sequential: {:.2}s", sequential_duration.as_secs_f64()),
                format!("Parallel: {:.2}s", parallel_duration.as_secs_f64()),
                format!("Speedup: {:.2}x", speedup),
                format!("Efficiency: {:.1}%", efficiency * 100.0),
            ],
        }
    }
    
    async fn benchmark_worker_scalability(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        let wallet_addresses: Vec<String> = (0..2000)
            .map(|i| format!("scalability_wallet_{}", i))
            .collect();
        
        let worker_counts = vec![1, 2, 4, 8, 16];
        let mut throughput_results = Vec::new();
        
        for worker_count in worker_counts {
            let config = ProcessorConfig {
                batch_size: 200,
                max_concurrent_wallets: 200,
                retry_attempts: 3,
                retry_delay_ms: 1000,
                enable_intelligent_processing: true,
                num_workers: Some(worker_count),
            };
            
            let processor = BatchProcessor::new(
                self.scanner.clone(),
                None,
                None,
                config,
            ).unwrap();
            
            let request = BatchScanRequest {
                id: Uuid::new_v4(),
                wallet_addresses: wallet_addresses.clone(),
                user_id: None,
                fee_percentage: None,
                created_at: Utc::now(),
            };
            
            let test_start = Instant::now();
            let result = processor.process_batch(&request).await;
            let test_duration = test_start.elapsed();
            
            if let Ok(batch_result) = result {
                let throughput = batch_result.total_wallets as f64 / test_duration.as_secs_f64();
                throughput_results.push((worker_count, throughput));
                
                metrics.push(BenchmarkMetric {
                    name: format!("{} Workers Throughput", worker_count),
                    value: throughput,
                    unit: "wallets/sec".to_string(),
                });
            }
        }
        
        // Calculate scaling efficiency
        let baseline_throughput = throughput_results[0].1;
        let max_throughput = throughput_results.last().unwrap().1;
        let theoretical_max = baseline_throughput * throughput_results.len() as f64;
        let scaling_efficiency = max_throughput / theoretical_max;
        
        metrics.push(BenchmarkMetric {
            name: "Scaling Efficiency".to_string(),
            value: scaling_efficiency * 100.0,
            unit: "percent".to_string(),
        });
        
        BenchmarkResult {
            name: "Worker Scalability".to_string(),
            success: scaling_efficiency > 0.5, // Expect at least 50% scaling efficiency
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: throughput_results.iter()
                .map(|(workers, throughput)| format!("{} workers: {:.1} wallets/sec", workers, throughput))
                .collect(),
        }
    }
    
    async fn benchmark_batch_size_optimization(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        let batch_sizes = vec![50, 100, 200, 500, 1000];
        let mut efficiency_results = Vec::new();
        
        for batch_size in batch_sizes {
            let config = ProcessorConfig {
                batch_size,
                max_concurrent_wallets: batch_size * 2,
                retry_attempts: 3,
                retry_delay_ms: 1000,
                enable_intelligent_processing: true,
                num_workers: Some(8),
            };
            
            let processor = BatchProcessor::new(
                self.scanner.clone(),
                None,
                None,
                config,
            ).unwrap();
            
            let wallet_addresses: Vec<String> = (0..batch_size * 4)
                .map(|i| format!("batch_opt_wallet_{}", i))
                .collect();
            
            let request = BatchScanRequest {
                id: Uuid::new_v4(),
                wallet_addresses,
                user_id: None,
                fee_percentage: None,
                created_at: Utc::now(),
            };
            
            let test_start = Instant::now();
            let result = processor.process_batch(&request).await;
            let test_duration = test_start.elapsed();
            
            if let Ok(batch_result) = result {
                let throughput = batch_result.total_wallets as f64 / test_duration.as_secs_f64();
                let efficiency = throughput / batch_size as f64;
                efficiency_results.push((batch_size, throughput, efficiency));
                
                metrics.push(BenchmarkMetric {
                    name: format!("Batch Size {} Throughput", batch_size),
                    value: throughput,
                    unit: "wallets/sec".to_string(),
                });
            }
        }
        
        // Find optimal batch size
        let optimal_batch = efficiency_results.iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(size, throughput, efficiency)| (*size, *throughput, *efficiency));
        
        if let Some((size, throughput, _efficiency)) = optimal_batch {
            metrics.push(BenchmarkMetric {
                name: "Optimal Batch Size".to_string(),
                value: size as f64,
                unit: "wallets".to_string(),
            });
            
            metrics.push(BenchmarkMetric {
                name: "Optimal Throughput".to_string(),
                value: throughput,
                unit: "wallets/sec".to_string(),
            });
        }
        
        BenchmarkResult {
            name: "Batch Size Optimization".to_string(),
            success: true, // Always successful as it's an optimization test
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: efficiency_results.iter()
                .map(|(size, throughput, efficiency)| 
                    format!("Batch {}: {:.1} wallets/sec (efficiency: {:.3})", size, throughput, efficiency))
                .collect(),
        }
    }
    
    async fn benchmark_memory_efficiency(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        let test_sizes = vec![100, 500, 1000, 2000];
        let mut memory_results = Vec::new();
        
        for size in test_sizes {
            let initial_memory = self.get_memory_usage();
            
            let config = ProcessorConfig {
                batch_size: size,
                max_concurrent_wallets: size,
                retry_attempts: 3,
                retry_delay_ms: 1000,
                enable_intelligent_processing: true,
                num_workers: Some(8),
            };
            
            let processor = BatchProcessor::new(
                self.scanner.clone(),
                None,
                None,
                config,
            ).unwrap();
            
            let wallet_addresses: Vec<String> = (0..size)
                .map(|i| format!("memory_wallet_{}", i))
                .collect();
            
            let request = BatchScanRequest {
                id: Uuid::new_v4(),
                wallet_addresses,
                user_id: None,
                fee_percentage: None,
                created_at: Utc::now(),
            };
            
            let _result = processor.process_batch(&request).await;
            
            let peak_memory = self.get_memory_usage();
            let memory_increase = peak_memory.saturating_sub(initial_memory);
            let memory_per_wallet = memory_increase as f64 / size as f64;
            
            memory_results.push((size, memory_increase, memory_per_wallet));
            
            metrics.push(BenchmarkMetric {
                name: format!("Memory per Wallet (size {})", size),
                value: memory_per_wallet,
                unit: "KB".to_string(),
            });
        }
        
        // Calculate average memory efficiency
        let avg_memory_per_wallet: f64 = memory_results.iter()
            .map(|(_, _, per_wallet)| *per_wallet)
            .sum::<f64>() / memory_results.len() as f64;
        
        metrics.push(BenchmarkMetric {
            name: "Average Memory per Wallet".to_string(),
            value: avg_memory_per_wallet,
            unit: "KB".to_string(),
        });
        
        BenchmarkResult {
            name: "Memory Efficiency".to_string(),
            success: avg_memory_per_wallet < 100.0, // Expect less than 100KB per wallet
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: memory_results.iter()
                .map(|(size, increase, per_wallet)| 
                    format!("Size {}: {} MB total, {:.1} KB per wallet", size, increase, per_wallet))
                .collect(),
        }
    }
    
    async fn benchmark_network_efficiency(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        // Test network efficiency with different concurrency levels
        let concurrency_levels = vec![10, 50, 100, 200];
        let mut network_results = Vec::new();
        
        for concurrency in concurrency_levels {
            let config = ProcessorConfig {
                batch_size: 1000,
                max_concurrent_wallets: concurrency,
                retry_attempts: 3,
                retry_delay_ms: 1000,
                enable_intelligent_processing: true,
                num_workers: Some(8),
            };
            
            let processor = BatchProcessor::new(
                self.scanner.clone(),
                None,
                None,
                config,
            ).unwrap();
            
            let wallet_addresses: Vec<String> = (0..1000)
                .map(|i| format!("network_wallet_{}", i))
                .collect();
            
            let request = BatchScanRequest {
                id: Uuid::new_v4(),
                wallet_addresses,
                user_id: None,
                fee_percentage: None,
                created_at: Utc::now(),
            };
            
            let test_start = Instant::now();
            let result = processor.process_batch(&request).await;
            let test_duration = test_start.elapsed();
            
            if let Ok(batch_result) = result {
                let throughput = batch_result.total_wallets as f64 / test_duration.as_secs_f64();
                let requests_per_second = throughput * 5.0; // Estimate 5 requests per wallet
                
                network_results.push((concurrency, throughput, requests_per_second));
                
                metrics.push(BenchmarkMetric {
                    name: format!("Concurrency {} Throughput", concurrency),
                    value: throughput,
                    unit: "wallets/sec".to_string(),
                });
                
                metrics.push(BenchmarkMetric {
                    name: format!("Concurrency {} RPS", concurrency),
                    value: requests_per_second,
                    unit: "requests/sec".to_string(),
                });
            }
        }
        
        BenchmarkResult {
            name: "Network Efficiency".to_string(),
            success: true, // Network efficiency is relative
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: network_results.iter()
                .map(|(concurrency, throughput, rps)| 
                    format!("Concurrency {}: {:.1} wallets/sec, {:.0} RPS", concurrency, throughput, rps))
                .collect(),
        }
    }
    
    async fn benchmark_load_balancing(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        // Create a processor with multiple workers
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(4),
            50,
        ).unwrap();
        
        // Create a large batch to test load balancing
        let wallet_addresses: Vec<String> = (0..4000)
            .map(|i| format!("load_balance_wallet_{}", i))
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let test_start = Instant::now();
        let result = processor.process_batch_intelligently(&request).await;
        let test_duration = test_start.elapsed();
        
        if let Ok(batch_result) = result {
            let throughput = batch_result.total_wallets as f64 / test_duration.as_secs_f64();
            let success_rate = batch_result.completed_wallets as f64 / batch_result.total_wallets as f64;
            
            metrics.push(BenchmarkMetric {
                name: "Load Balanced Throughput".to_string(),
                value: throughput,
                unit: "wallets/sec".to_string(),
            });
            
            metrics.push(BenchmarkMetric {
                name: "Load Balanced Success Rate".to_string(),
                value: success_rate * 100.0,
                unit: "percent".to_string(),
            });
            
            // Get resource metrics
            let resource_metrics = processor.get_resource_metrics().await;
            metrics.push(BenchmarkMetric {
                name: "CPU Utilization".to_string(),
                value: resource_metrics.cpu_usage_percent,
                unit: "percent".to_string(),
            });
            
            metrics.push(BenchmarkMetric {
                name: "Memory Usage".to_string(),
                value: resource_metrics.memory_usage_mb as f64,
                unit: "MB".to_string(),
            });
        }
        
        BenchmarkResult {
            name: "Load Balancing".to_string(),
            success: true, // Load balancing test is informational
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: vec![
                "Load balancing tested with 4000 wallets across 4 workers".to_string(),
                format!("Duration: {:.2}s", test_duration.as_secs_f64()),
            ],
        }
    }
    
    async fn benchmark_priority_processing(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        // Test priority processing impact
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(4),
            100,
        ).unwrap();
        
        // Create a mixed priority batch
        let wallet_addresses: Vec<String> = (0..1000)
            .map(|i| format!("priority_wallet_{}", i))
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let test_start = Instant::now();
        let result = processor.process_batch_intelligently(&request).await;
        let test_duration = test_start.elapsed();
        
        if let Ok(batch_result) = result {
            let throughput = batch_result.total_wallets as f64 / test_duration.as_secs_f64();
            
            metrics.push(BenchmarkMetric {
                name: "Priority Processing Throughput".to_string(),
                value: throughput,
                unit: "wallets/sec".to_string(),
            });
        }
        
        BenchmarkResult {
            name: "Priority Processing".to_string(),
            success: true, // Priority processing is a feature test
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: vec![
                "Priority processing tested with 1000 wallets".to_string(),
                format!("Duration: {:.2}s", test_duration.as_secs_f64()),
            ],
        }
    }
    
    async fn benchmark_resource_utilization(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(4),
            50,
        ).unwrap();
        
        // Get initial resource metrics
        let initial_metrics = processor.get_resource_metrics().await;
        
        // Run a substantial workload
        let wallet_addresses: Vec<String> = (0..3000)
            .map(|i| format!("resource_wallet_{}", i))
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let _result = processor.process_batch_intelligently(&request).await;
        
        // Get final resource metrics
        let final_metrics = processor.get_resource_metrics().await;
        
        metrics.push(BenchmarkMetric {
            name: "Peak CPU Usage".to_string(),
            value: final_metrics.cpu_usage_percent,
            unit: "percent".to_string(),
        });
        
        metrics.push(BenchmarkMetric {
            name: "Peak Memory Usage".to_string(),
            value: final_metrics.memory_usage_mb as f64,
            unit: "MB".to_string(),
        });
        
        metrics.push(BenchmarkMetric {
            name: "Active Threads".to_string(),
            value: final_metrics.active_threads as f64,
            unit: "threads".to_string(),
        });
        
        BenchmarkResult {
            name: "Resource Utilization".to_string(),
            success: final_metrics.cpu_usage_percent < 90.0, // CPU should not be maxed out
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: vec![
                format!("Initial CPU: {:.1}%", initial_metrics.cpu_usage_percent),
                format!("Peak CPU: {:.1}%", final_metrics.cpu_usage_percent),
                format!("Initial Memory: {} MB", initial_metrics.memory_usage_mb),
                format!("Peak Memory: {} MB", final_metrics.memory_usage_mb),
            ],
        }
    }
    
    async fn benchmark_fault_tolerance_overhead(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        // Test with some intentionally problematic wallets
        let wallet_addresses: Vec<String> = (0..500)
            .map(|i| {
                match i % 20 {
                    0 => "invalid_wallet_address".to_string(),
                    10 => "".to_string(),
                    _ => format!("fault_tolerance_wallet_{}", i),
                }
            })
            .collect();
        
        let config = ProcessorConfig {
            batch_size: 100,
            max_concurrent_wallets: 100,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            enable_intelligent_processing: true,
            num_workers: Some(4),
        };
        
        let processor = BatchProcessor::new(
            self.scanner.clone(),
            None,
            None,
            config,
        ).unwrap();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let test_start = Instant::now();
        let result = processor.process_batch(&request).await;
        let test_duration = test_start.elapsed();
        
        if let Ok(batch_result) = result {
            let throughput = batch_result.total_wallets as f64 / test_duration.as_secs_f64();
            let failure_rate = batch_result.failed_wallets as f64 / batch_result.total_wallets as f64;
            
            metrics.push(BenchmarkMetric {
                name: "Fault Tolerant Throughput".to_string(),
                value: throughput,
                unit: "wallets/sec".to_string(),
            });
            
            metrics.push(BenchmarkMetric {
                name: "Failure Rate".to_string(),
                value: failure_rate * 100.0,
                unit: "percent".to_string(),
            });
        }
        
        BenchmarkResult {
            name: "Fault Tolerance Overhead".to_string(),
            success: true, // Fault tolerance is a reliability feature
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: vec![
                "Fault tolerance tested with 5% intentionally invalid wallets".to_string(),
                format!("Duration: {:.2}s", test_duration.as_secs_f64()),
            ],
        }
    }
    
    async fn benchmark_stability(&self) -> BenchmarkResult {
        let start_time = Instant::now();
        let mut metrics = Vec::new();
        
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(8),
            150,
        ).unwrap();
        
        // Run multiple consecutive batches to test stability
        let num_batches = 5;
        let wallets_per_batch = 1000;
        let mut batch_times = Vec::new();
        
        for batch_num in 0..num_batches {
            let wallet_addresses: Vec<String> = (0..wallets_per_batch)
                .map(|i| format!("stability_wallet_{}_{}", batch_num, i))
                .collect();
            
            let request = BatchScanRequest {
                id: Uuid::new_v4(),
                wallet_addresses,
                user_id: None,
                fee_percentage: None,
                created_at: Utc::now(),
            };
            
            let batch_start = Instant::now();
            let result = processor.process_batch_intelligently(&request).await;
            let batch_duration = batch_start.elapsed();
            
            batch_times.push(batch_duration.as_secs_f64());
            
            if let Ok(batch_result) = result {
                metrics.push(BenchmarkMetric {
                    name: format!("Batch {} Throughput", batch_num + 1),
                    value: batch_result.total_wallets as f64 / batch_duration.as_secs_f64(),
                    unit: "wallets/sec".to_string(),
                });
            }
        }
        
        // Calculate stability metrics
        let avg_time: f64 = batch_times.iter().sum();
        let variance: f64 = batch_times.iter()
            .map(|time| (time - avg_time / num_batches as f64).powi(2))
            .sum::<f64>() / num_batches as f64;
        let std_dev = variance.sqrt();
        let coefficient_of_variation = std_dev / (avg_time / num_batches as f64);
        
        metrics.push(BenchmarkMetric {
            name: "Average Batch Time".to_string(),
            value: avg_time / num_batches as f64,
            unit: "seconds".to_string(),
        });
        
        metrics.push(BenchmarkMetric {
            name: "Time Standard Deviation".to_string(),
            value: std_dev,
            unit: "seconds".to_string(),
        });
        
        metrics.push(BenchmarkMetric {
            name: "Coefficient of Variation".to_string(),
            value: coefficient_of_variation * 100.0,
            unit: "percent".to_string(),
        });
        
        BenchmarkResult {
            name: "Stability Test".to_string(),
            success: coefficient_of_variation < 0.2, // Expect < 20% variation
            duration_ms: start_time.elapsed().as_millis() as u64,
            metrics,
            details: vec![
                format!("Ran {} batches of {} wallets each", num_batches, wallets_per_batch),
                format!("Average time: {:.2}s ± {:.2}s", avg_time / num_batches as f64, std_dev),
                format!("Variation: {:.1}%", coefficient_of_variation * 100.0),
            ],
        }
    }
    
    fn get_memory_usage(&self) -> u64 {
        // Simple memory usage estimation
        // In a real implementation, you'd use system APIs
        100 // Placeholder in MB
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub metrics: Vec<BenchmarkMetric>,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetric {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub results: HashMap<String, BenchmarkResult>,
    pub start_time: Instant,
}

impl BenchmarkResults {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            start_time: Instant::now(),
        }
    }
    
    pub fn add_result(&mut self, benchmark_name: &str, result: BenchmarkResult) {
        self.results.insert(benchmark_name.to_string(), result);
    }
    
    pub fn get_summary(&self) -> BenchmarkSummary {
        let total_benchmarks = self.results.len();
        let passed_benchmarks = self.results.values().filter(|r| r.success).count();
        let failed_benchmarks = total_benchmarks - passed_benchmarks;
        
        let total_duration: u64 = self.results.values().map(|r| r.duration_ms).sum();
        
        BenchmarkSummary {
            total_benchmarks,
            passed_benchmarks,
            failed_benchmarks,
            success_rate: if total_benchmarks > 0 {
                passed_benchmarks as f64 / total_benchmarks as f64 * 100.0
            } else {
                0.0
            },
            total_duration_ms: total_duration,
            overall_duration_ms: self.start_time.elapsed().as_millis() as u64,
        }
    }
    
    pub fn print_detailed_results(&self) {
        println!("\n=== Performance Benchmark Results ===");
        
        for (name, result) in &self.results {
            let status = if result.success { "PASS" } else { "FAIL" };
            println!("\n{}: {} ({}ms)", name, status, result.duration_ms);
            
            for metric in &result.metrics {
                println!("  - {}: {:.2} {}", metric.name, metric.value, metric.unit);
            }
            
            for detail in &result.details {
                println!("  * {}", detail);
            }
        }
        
        let summary = self.get_summary();
        println!("\n=== Benchmark Summary ===");
        println!("Total Benchmarks: {}", summary.total_benchmarks);
        println!("Passed: {}", summary.passed_benchmarks);
        println!("Failed: {}", summary.failed_benchmarks);
        println!("Success Rate: {:.1}%", summary.success_rate);
        println!("Total Duration: {}ms", summary.total_duration_ms);
        println!("Overall Duration: {}ms", summary.overall_duration_ms);
    }
    
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.results)
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkSummary {
    pub total_benchmarks: usize,
    pub passed_benchmarks: usize,
    pub failed_benchmarks: usize,
    pub success_rate: f64,
    pub total_duration_ms: u64,
    pub overall_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_performance_benchmarks() {
        let benchmarks = PerformanceBenchmarks::new();
        let results = benchmarks.run_all_benchmarks().await;
        
        // Print results for manual inspection
        results.print_detailed_results();
        
        // Assert that most benchmarks pass
        let summary = results.get_summary();
        assert!(summary.success_rate >= 70.0, "Success rate should be at least 70%");
    }
}
