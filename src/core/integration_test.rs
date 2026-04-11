use crate::core::parallel_processor::IntelligentParallelProcessor;
use crate::core::parallel_tests::ParallelProcessingTests;
use crate::core::processor::{BatchProcessor, ProcessorConfig};
use crate::core::benchmarks::PerformanceBenchmarks;
use crate::core::resource_monitor::SystemResourceMonitor;
use crate::core::thread_pool_optimizer::{OptimizedThreadPool, OptimizedThreadPoolBuilder};
use crate::core::scanner::WalletScanner;
use crate::core::{BatchScanRequest, ScanResult, ScanStatus};
use crate::rpc::mock::MockConnectionPool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, debug, warn, error};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

/// Integration test demonstrating the complete parallel processing system
pub struct ParallelProcessingIntegrationTest;

impl ParallelProcessingIntegrationTest {
    /// Run complete integration test suite
    pub async fn run_complete_integration_test() -> IntegrationTestResults {
        info!("Starting complete parallel processing integration test");
        
        let mut results = IntegrationTestResults::new();
        
        // Test 1: Basic parallel processing functionality
        results.add_result("basic_parallel_processing", Self::test_basic_parallel_processing().await);
        
        // Test 2: Intelligent processor vs legacy processor
        results.add_result("processor_comparison", Self::test_processor_comparison().await);
        
        // Test 3: Resource monitoring integration
        results.add_result("resource_monitoring", Self::test_resource_monitoring().await);
        
        // Test 4: Thread pool optimization
        results.add_result("thread_pool_optimization", Self::test_thread_pool_optimization().await);
        
        // Test 5: Full system performance test
        results.add_result("full_system_performance", Self::test_full_system_performance().await);
        
        // Test 6: Error handling and recovery
        results.add_result("error_handling", Self::test_error_handling().await);
        
        // Test 7: Configuration validation
        results.add_result("configuration_validation", Self::test_configuration_validation().await);
        
        // Test 8: Metrics and monitoring
        results.add_result("metrics_monitoring", Self::test_metrics_monitoring().await);
        
        info!("Completed parallel processing integration test");
        results
    }
    
    async fn test_basic_parallel_processing() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Create mock connection pool and scanner
        let mock_pool = Arc::new(MockConnectionPool::new_simple());
        let scanner = Arc::new(WalletScanner::new(mock_pool));
        
        // Create intelligent parallel processor
        let mut processor = IntelligentParallelProcessor::new(
            scanner.clone(),
            Some(4),
            100,
        ).unwrap();
        
        // Create test batch
        let wallet_addresses: Vec<String> = (0..500)
            .map(|i| format!("integration_wallet_{}", i))
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        // Process batch
        let result = processor.process_batch_intelligently(&request).await;
        
        match result {
            Ok(batch_result) => {
                details.push(format!("Processed {} wallets successfully", batch_result.total_wallets));
                details.push(format!("Completed: {}, Failed: {}", 
                                   batch_result.completed_wallets, batch_result.failed_wallets));
                
                // Verify results
                if batch_result.total_wallets == 500 {
                    details.push("Correct wallet count processed".to_string());
                } else {
                    success = false;
                    details.push(format!("Incorrect wallet count: expected 500, got {}", batch_result.total_wallets));
                }
                
                // Check processing time
                if let Some(duration) = batch_result.duration_ms {
                    let throughput = batch_result.total_wallets as f64 / (duration as f64 / 1000.0);
                    details.push(format!("Throughput: {:.1} wallets/sec", throughput));
                    
                    if throughput > 10.0 {
                        details.push("Good throughput achieved".to_string());
                    } else {
                        success = false;
                        details.push("Throughput too low".to_string());
                    }
                }
                
                // Get resource metrics
                let resource_metrics = processor.get_resource_metrics().await;
                details.push(format!("CPU usage: {:.1}%", resource_metrics.cpu_usage_percent));
                details.push(format!("Memory usage: {} MB", resource_metrics.memory_usage_mb));
                details.push(format!("Active threads: {}", resource_metrics.active_threads));
            }
            Err(e) => {
                success = false;
                details.push(format!("Processing failed: {}", e));
            }
        }
        
        IntegrationTestResult {
            name: "Basic Parallel Processing".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_processor_comparison() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let mock_pool = Arc::new(MockConnectionPool::new_simple());
        let scanner = Arc::new(WalletScanner::new(mock_pool));
        
        let wallet_addresses: Vec<String> = (0..300)
            .map(|i| format!("comparison_wallet_{}", i))
            .collect();
        
        // Test legacy processor
        let legacy_start = Instant::now();
        let mock_endpoint = crate::core::RpcEndpoint {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            priority: 1,
            rate_limit_rps: 1000,
            healthy: true,
            timeout_ms: 30000,
        };
        let legacy_pool = crate::rpc::ConnectionPool::new(vec![mock_endpoint], 50);
        let legacy_processor = BatchProcessor::new_simple(Arc::new(legacy_pool), 50);
        let legacy_request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses: wallet_addresses.clone(),
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let legacy_result = legacy_processor.process_batch(&legacy_request).await;
        let legacy_duration = legacy_start.elapsed();
        
        // Test intelligent processor
        let intelligent_start = Instant::now();
        let intelligent_config = ProcessorConfig {
            batch_size: 100,
            max_concurrent_wallets: 100,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            enable_intelligent_processing: true,
            num_workers: Some(4),
        };
        
        let intelligent_processor = BatchProcessor::new(
            scanner.clone(),
            None,
            None,
            intelligent_config,
        ).unwrap();
        
        let intelligent_request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses: wallet_addresses.clone(),
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let intelligent_result = intelligent_processor.process_batch(&intelligent_request).await;
        let intelligent_duration = intelligent_start.elapsed();
        
        // Compare results
        match (legacy_result, intelligent_result) {
            (Ok(legacy_batch), Ok(intelligent_batch)) => {
                let legacy_throughput = legacy_batch.total_wallets as f64 / legacy_duration.as_secs_f64();
                let intelligent_throughput = intelligent_batch.total_wallets as f64 / intelligent_duration.as_secs_f64();
                let improvement = intelligent_throughput / legacy_throughput;
                
                details.push(format!("Legacy throughput: {:.1} wallets/sec", legacy_throughput));
                details.push(format!("Intelligent throughput: {:.1} wallets/sec", intelligent_throughput));
                details.push(format!("Improvement: {:.2}x", improvement));
                
                if improvement > 1.5 {
                    details.push("Significant improvement achieved".to_string());
                } else if improvement > 1.0 {
                    details.push("Moderate improvement achieved".to_string());
                } else {
                    success = false;
                    details.push("No improvement observed".to_string());
                }
                
                // Verify both processed same number of wallets
                if legacy_batch.total_wallets == intelligent_batch.total_wallets {
                    details.push("Both processors processed same number of wallets".to_string());
                } else {
                    success = false;
                    details.push("Processors handled different numbers of wallets".to_string());
                }
            }
            (Err(e), _) => {
                success = false;
                details.push(format!("Legacy processor failed: {}", e));
            }
            (_, Err(e)) => {
                success = false;
                details.push(format!("Intelligent processor failed: {}", e));
            }
        }
        
        IntegrationTestResult {
            name: "Processor Comparison".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_resource_monitoring() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Create system resource monitor
        let config = crate::core::resource_monitor::MonitorConfig {
            sampling_interval_ms: 500,
            history_size: 100,
            enable_cpu_monitoring: true,
            enable_memory_monitoring: true,
            enable_network_monitoring: true,
            enable_disk_monitoring: false,
            enable_process_monitoring: true,
            ..Default::default()
        };
        
        let monitor = SystemResourceMonitor::new(config);
        
        // Start monitoring
        let _ = monitor.start_monitoring().await;
        
        // Let it collect some data
        tokio::time::sleep(Duration::from_millis(1500)).await;
        
        // Get current metrics
        let current_metrics = monitor.get_current_metrics().await;
        details.push(format!("Current CPU usage: {:.1}%", current_metrics.cpu.total_usage));
        details.push(format!("Current memory usage: {} MB", current_metrics.memory.used_memory_mb));
        details.push(format!("Current network RPS: {}", current_metrics.network.requests_per_second));
        
        // Get history
        let history = monitor.get_metrics_history(Some(1)).await;
        details.push(format!("Collected {} data points", history.len()));
        
        if history.len() > 0 {
            details.push("Resource monitoring is working".to_string());
        } else {
            success = false;
            details.push("No monitoring data collected".to_string());
        }
        
        // Get average metrics
        if let Some(avg_metrics) = monitor.get_average_metrics(1).await {
            details.push(format!("Average CPU usage: {:.1}%", avg_metrics.cpu.total_usage));
            details.push("Average metrics calculation working".to_string());
        } else {
            success = false;
            details.push("Failed to calculate average metrics".to_string());
        }
        
        IntegrationTestResult {
            name: "Resource Monitoring".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_thread_pool_optimization() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Test optimized thread pool builder
        let pool = OptimizedThreadPoolBuilder::new()
            .num_threads(4)
            .enable_cpu_affinity(true)
            .enable_numa_awareness(false)
            .build();
        
        match pool {
            Ok(optimized_pool) => {
                let metrics = optimized_pool.get_metrics();
                details.push(format!("Created optimized pool with {} threads", metrics.total_threads));
                details.push(format!("CPU affinity enabled: {}", metrics.cpu_affinity_enabled));
                details.push(format!("NUMA awareness enabled: {}", metrics.numa_awareness_enabled));
                
                if metrics.total_threads == 4 {
                    details.push("Correct number of threads created".to_string());
                } else {
                    success = false;
                    details.push(format!("Expected 4 threads, got {}", metrics.total_threads));
                }
                
                // Test thread pool functionality
                let pool_ref = optimized_pool.pool();
                let results: Vec<_> = pool_ref.install(|| {
                    (0..100).into_par_iter().map(|i| i * 2).collect()
                });
                
                if results.len() == 100 {
                    details.push("Thread pool processing works correctly".to_string());
                } else {
                    success = false;
                    details.push(format!("Expected 100 results, got {}", results.len()));
                }
                
                // Verify results
                if results.iter().enumerate().all(|(i, &result)| result == i as i32 * 2) {
                    details.push("Thread pool computation correct".to_string());
                } else {
                    success = false;
                    details.push("Thread pool computation incorrect".to_string());
                }
            }
            Err(e) => {
                success = false;
                details.push(format!("Failed to create optimized thread pool: {}", e));
            }
        }
        
        IntegrationTestResult {
            name: "Thread Pool Optimization".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_full_system_performance() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Run performance benchmarks
        let benchmarks = PerformanceBenchmarks::new();
        let benchmark_results = benchmarks.run_all_benchmarks().await;
        
        let summary = benchmark_results.get_summary();
        details.push(format!("Ran {} benchmarks", summary.total_benchmarks));
        details.push(format!("Success rate: {:.1}%", summary.success_rate));
        details.push(format!("Total duration: {}ms", summary.total_duration_ms));
        
        if summary.success_rate >= 70.0 {
            details.push("Good benchmark performance achieved".to_string());
        } else {
            success = false;
            details.push("Benchmark performance below expectations".to_string());
        }
        
        // Run parallel processing tests
        let tests = ParallelProcessingTests::new();
        let test_results = tests.run_all_tests().await;
        
        let test_summary = test_results.get_summary();
        details.push(format!("Ran {} tests", test_summary.total_tests));
        details.push(format!("Test success rate: {:.1}%", test_summary.success_rate));
        
        if test_summary.success_rate >= 80.0 {
            details.push("Good test performance achieved".to_string());
        } else {
            success = false;
            details.push("Test performance below expectations".to_string());
        }
        
        IntegrationTestResult {
            name: "Full System Performance".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_error_handling() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let mock_pool = Arc::new(MockConnectionPool::new_simple());
        let scanner = Arc::new(WalletScanner::new(mock_pool));
        
        // Create processor with error handling
        let mut processor = IntelligentParallelProcessor::new(
            scanner.clone(),
            Some(4),
            50,
        ).unwrap();
        
        // Create batch with some invalid addresses
        let wallet_addresses: Vec<String> = (0..100)
            .map(|i| {
                match i % 25 {
                    0 => "invalid_wallet_address".to_string(),
                    10 => "".to_string(),
                    _ => format!("error_test_wallet_{}", i),
                }
            })
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let result = processor.process_batch_intelligently(&request).await;
        
        match result {
            Ok(batch_result) => {
                details.push(format!("Processed {} wallets with error handling", batch_result.total_wallets));
                details.push(format!("Completed: {}, Failed: {}", 
                                   batch_result.completed_wallets, batch_result.failed_wallets));
                
                // Should have some failures but still complete processing
                if batch_result.failed_wallets > 0 {
                    details.push("Error handling working correctly".to_string());
                } else {
                    details.push("No failures detected (may be expected)".to_string());
                }
                
                // Should still have some successes
                if batch_result.completed_wallets > 0 {
                    details.push("Partial success achieved".to_string());
                } else {
                    success = false;
                    details.push("No successful processing".to_string());
                }
                
                // Total should equal input
                if batch_result.completed_wallets + batch_result.failed_wallets == batch_result.total_wallets {
                    details.push("All wallets accounted for".to_string());
                } else {
                    success = false;
                    details.push("Wallet count mismatch".to_string());
                }
            }
            Err(e) => {
                success = false;
                details.push(format!("Error handling test failed: {}", e));
            }
        }
        
        IntegrationTestResult {
            name: "Error Handling".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_configuration_validation() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let mock_pool = Arc::new(MockConnectionPool::new_simple());
        let scanner = Arc::new(WalletScanner::new(mock_pool));
        
        // Test various configurations
        let configs = vec![
            ProcessorConfig {
                batch_size: 50,
                max_concurrent_wallets: 50,
                retry_attempts: 3,
                retry_delay_ms: 1000,
                enable_intelligent_processing: true,
                num_workers: Some(2),
            },
            ProcessorConfig {
                batch_size: 200,
                max_concurrent_wallets: 200,
                retry_attempts: 5,
                retry_delay_ms: 2000,
                enable_intelligent_processing: true,
                num_workers: Some(8),
            },
            ProcessorConfig {
                batch_size: 100,
                max_concurrent_wallets: 100,
                retry_attempts: 1,
                retry_delay_ms: 500,
                enable_intelligent_processing: false,
                num_workers: None,
            },
        ];
        
        for (i, config) in configs.iter().enumerate() {
            match BatchProcessor::new(scanner.clone(), None, None, config.clone()) {
                Ok(processor) => {
                    details.push(format!("Configuration {} created successfully", i + 1));
                    
                    // Test basic functionality
                    let wallet_addresses: Vec<String> = (0..50)
                        .map(|j| format!("config_test_{}_{}", i, j))
                        .collect();
                    
                    let request = BatchScanRequest {
                        id: Uuid::new_v4(),
                        wallet_addresses,
                        user_id: None,
                        fee_percentage: None,
                        created_at: Utc::now(),
                    };
                    
                    let result = processor.process_batch(&request).await;
                    match result {
                        Ok(_batch_result) => {
                            details.push(format!("Configuration {} processing successful", i + 1));
                        }
                        Err(e) => {
                            success = false;
                            details.push(format!("Configuration {} processing failed: {}", i + 1, e));
                        }
                    }
                }
                Err(e) => {
                    success = false;
                    details.push(format!("Configuration {} creation failed: {}", i + 1, e));
                }
            }
        }
        
        IntegrationTestResult {
            name: "Configuration Validation".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_metrics_monitoring() -> IntegrationTestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let mock_pool = Arc::new(MockConnectionPool::new_simple());
        let scanner = Arc::new(WalletScanner::new(mock_pool));
        
        // Create processor with metrics
        let config = ProcessorConfig {
            batch_size: 100,
            max_concurrent_wallets: 100,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            enable_intelligent_processing: true,
            num_workers: Some(4),
        };
        
        let processor = BatchProcessor::new(
            scanner.clone(),
            None,
            None,
            config,
        ).unwrap();
        
        // Get initial metrics
        let _initial_metrics = processor.get_metrics().await;
        details.push("Initial metrics collected".to_string());
        
        // Process a batch
        let wallet_addresses: Vec<String> = (0..200)
            .map(|i| format!("metrics_test_wallet_{}", i))
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let _result = processor.process_batch(&request).await;
        
        // Get updated metrics
        let updated_metrics = processor.get_metrics().await;
        details.push("Updated metrics collected".to_string());
        
        // Verify metrics structure
        if updated_metrics.active_scans > 0 {
            details.push("Active scans metric present".to_string());
        } else {
            success = false;
            details.push("Active scans metric missing".to_string());
        }
        
        if updated_metrics.total_wallets_processed > 0 {
            details.push("Wallets processed metric present".to_string());
        } else {
            success = false;
            details.push("Wallets processed metric missing".to_string());
        }
        
        if updated_metrics.throughput_wallets_per_second > 0.0 {
            details.push("Throughput metric present".to_string());
        } else {
            success = false;
            details.push("Throughput metric missing".to_string());
        }
        
        IntegrationTestResult {
            name: "Metrics Monitoring".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntegrationTestResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub details: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IntegrationTestResults {
    pub results: HashMap<String, IntegrationTestResult>,
    pub start_time: Instant,
}

impl IntegrationTestResults {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            start_time: Instant::now(),
        }
    }
    
    pub fn add_result(&mut self, test_name: &str, result: IntegrationTestResult) {
        self.results.insert(test_name.to_string(), result);
    }
    
    pub fn get_summary(&self) -> IntegrationTestSummary {
        let total_tests = self.results.len();
        let passed_tests = self.results.values().filter(|r| r.success).count();
        let failed_tests = total_tests - passed_tests;
        
        let total_duration: u64 = self.results.values().map(|r| r.duration_ms).sum();
        
        IntegrationTestSummary {
            total_tests,
            passed_tests,
            failed_tests,
            success_rate: if total_tests > 0 {
                passed_tests as f64 / total_tests as f64 * 100.0
            } else {
                0.0
            },
            total_duration_ms: total_duration,
            overall_duration_ms: self.start_time.elapsed().as_millis() as u64,
        }
    }
    
    pub fn print_detailed_results(&self) {
        println!("\n=== Parallel Processing Integration Test Results ===");
        
        for (name, result) in &self.results {
            let status = if result.success { "PASS" } else { "FAIL" };
            println!("\n{}: {} ({}ms)", name, status, result.duration_ms);
            
            for detail in &result.details {
                println!("  - {}", detail);
            }
        }
        
        let summary = self.get_summary();
        println!("\n=== Integration Test Summary ===");
        println!("Total Tests: {}", summary.total_tests);
        println!("Passed: {}", summary.passed_tests);
        println!("Failed: {}", summary.failed_tests);
        println!("Success Rate: {:.1}%", summary.success_rate);
        println!("Total Duration: {}ms", summary.total_duration_ms);
        println!("Overall Duration: {}ms", summary.overall_duration_ms);
    }
}

#[derive(Debug, Clone)]
pub struct IntegrationTestSummary {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub success_rate: f64,
    pub total_duration_ms: u64,
    pub overall_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_parallel_processing_integration() {
        let results = ParallelProcessingIntegrationTest::run_complete_integration_test().await;
        
        // Print results for manual inspection
        results.print_detailed_results();
        
        // Assert that most tests pass
        let summary = results.get_summary();
        assert!(summary.success_rate >= 80.0, "Success rate should be at least 80%");
    }
}
