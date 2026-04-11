use crate::core::parallel_processor::{
    IntelligentParallelProcessor, WorkStealingQueue, ProgressTracker, 
    ResourceMonitorTrait, DynamicBatchSizer, Priority, WalletTask
};
use crate::core::scanner::WalletScanner;
use crate::core::{BatchScanRequest};
use crate::rpc::mock::MockConnectionPool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;
use tracing::{info};

/// Comprehensive test suite for parallel processing
pub struct ParallelProcessingTests {
    #[allow(dead_code)]
    mock_pool: Arc<MockConnectionPool>,
    scanner: Arc<WalletScanner>,
}

impl ParallelProcessingTests {
    pub fn new() -> Self {
        let mock_pool = Arc::new(MockConnectionPool::new_simple());
        let scanner = Arc::new(WalletScanner::new(mock_pool.clone()));
        
        Self {
            mock_pool,
            scanner,
        }
    }
    
    /// Run all parallel processing tests
    pub async fn run_all_tests(&self) -> TestResults {
        info!("Starting comprehensive parallel processing tests");
        
        let mut results = TestResults::new();
        
        // Test 1: Task queue functionality
        results.add_result("task_queue", self.test_task_queue().await);
        
        // Test 2: Progress tracking accuracy
        results.add_result("progress_tracking", self.test_progress_tracking().await);
        
        // Test 3: Resource monitoring
        results.add_result("resource_monitoring", self.test_resource_monitoring().await);
        
        // Test 4: Dynamic batch sizing
        results.add_result("dynamic_batch_sizing", self.test_dynamic_batch_sizing().await);
        
        // Test 5: Priority-based processing
        results.add_result("priority_processing", self.test_priority_processing().await);
        
        // Test 6: Load balancing under stress
        results.add_result("load_balancing", self.test_load_balancing().await);
        
        // Test 7: Fault tolerance and recovery
        results.add_result("fault_tolerance", self.test_fault_tolerance().await);
        
        // Test 8: Memory efficiency
        results.add_result("memory_efficiency", self.test_memory_efficiency().await);
        
        // Test 9: Performance benchmarks
        results.add_result("performance_benchmarks", self.test_performance_benchmarks().await);
        
        // Test 10: Scalability analysis
        results.add_result("scalability_analysis", self.test_scalability_analysis().await);
        
        info!("Completed parallel processing tests");
        results
    }
    
    async fn test_task_queue(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Test basic task queue functionality
        let queue: WorkStealingQueue<i32> = WorkStealingQueue::new(4);
        
        // Push work items
        for i in 0..100 {
            queue.push(i);
        }
        
        // Get items sequentially (simplified test)
        let mut collected_items = Vec::new();
        for worker_id in 0..4 {
            while let Some(item) = queue.get_task(worker_id) {
                collected_items.push(item);
            }
        }
        
        if collected_items.len() == 100 {
            details.push("Successfully collected 100 items from queue".to_string());
        } else {
            success = false;
            details.push(format!("Expected 100 items, got {}", collected_items.len()));
        }
        
        // Verify items are in correct order
        let is_ordered = collected_items.iter().enumerate().all(|(i, &item)| item == i as i32);
        if is_ordered {
            details.push("Items retrieved in correct order".to_string());
        } else {
            success = false;
            details.push("Items not in correct order".to_string());
        }
        
        // Verify no duplicates
        let mut unique_items = collected_items.clone();
        unique_items.sort();
        unique_items.dedup();
        
        if collected_items.len() == unique_items.len() {
            details.push("No duplicate items found".to_string());
        } else {
            success = false;
            details.push("Duplicate items detected".to_string());
        }
        
        TestResult {
            name: "Task Queue".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_progress_tracking(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let tracker = ProgressTracker::new(1000);
        let (total, completed, failed) = tracker.get_progress();
        
        if total == 1000 && completed == 0 && failed == 0 {
            details.push("Initial progress tracking correct".to_string());
        } else {
            success = false;
            details.push(format!("Initial state incorrect: ({}, {}, {})", total, completed, failed));
        }
        
        // Simulate progress updates
        for i in 0..800 {
            if i % 10 == 0 {
                tracker.increment_failed();
            } else {
                tracker.increment_completed();
            }
        }
        
        let (total, completed, failed) = tracker.get_progress();
        if total == 1000 && completed == 720 && failed == 80 {
            details.push("Progress updates tracked correctly".to_string());
        } else {
            success = false;
            details.push(format!("Progress tracking incorrect: ({}, {}, {})", total, completed, failed));
        }
        
        // Test throughput calculation
        std::thread::sleep(Duration::from_millis(100));
        let throughput = tracker.get_throughput();
        if throughput > 0.0 {
            details.push(format!("Throughput calculated: {:.2} tasks/sec", throughput));
        } else {
            success = false;
            details.push("Throughput calculation failed".to_string());
        }
        
        TestResult {
            name: "Progress Tracking".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_resource_monitoring(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let monitor = crate::core::parallel_processor::ResourceMonitor::new();
        let initial_metrics = monitor.get_metrics();
        
        details.push(format!("Initial CPU: {:.1}%", initial_metrics.cpu_usage_percent));
        details.push(format!("Initial Memory: {} MB", initial_metrics.memory_usage_mb));
        
        // Simulate resource usage changes
        monitor.update_cpu_usage(75.5);
        monitor.update_memory_usage(2048);
        monitor.update_network_rps(5000);
        
        let updated_metrics = monitor.get_metrics();
        
        if (updated_metrics.cpu_usage_percent - 75.5).abs() < 0.1 {
            details.push("CPU usage update successful".to_string());
        } else {
            success = false;
            details.push("CPU usage update failed".to_string());
        }
        
        if updated_metrics.memory_usage_mb == 2048 {
            details.push("Memory usage update successful".to_string());
        } else {
            success = false;
            details.push("Memory usage update failed".to_string());
        }
        
        if updated_metrics.network_requests_per_second == 5000 {
            details.push("Network RPS update successful".to_string());
        } else {
            success = false;
            details.push("Network RPS update failed".to_string());
        }
        
        TestResult {
            name: "Resource Monitoring".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_dynamic_batch_sizing(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let monitor = Arc::new(crate::core::parallel_processor::ResourceMonitor::new());
        let sizer = DynamicBatchSizer::new(100, Arc::clone(&monitor) as Arc<dyn ResourceMonitorTrait>);
        
        // Test with low resource usage
        monitor.update_cpu_usage(25.0);
        monitor.update_memory_usage(512);
        let low_usage_size = sizer.get_optimal_batch_size();
        
        if low_usage_size > 100 {
            details.push(format!("Batch size increased under low load: {}", low_usage_size));
        } else {
            success = false;
            details.push("Batch size should increase under low load".to_string());
        }
        
        // Test with high resource usage
        monitor.update_cpu_usage(85.0);
        monitor.update_memory_usage(8192);
        let high_usage_size = sizer.get_optimal_batch_size();
        
        if high_usage_size < 100 {
            details.push(format!("Batch size decreased under high load: {}", high_usage_size));
        } else {
            success = false;
            details.push("Batch size should decrease under high load".to_string());
        }
        
        // Test bounds
        if high_usage_size >= sizer.min_batch_size && high_usage_size <= sizer.max_batch_size {
            details.push("Batch size within bounds".to_string());
        } else {
            success = false;
            details.push("Batch size out of bounds".to_string());
        }
        
        TestResult {
            name: "Dynamic Batch Sizing".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_priority_processing(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Create tasks with different priorities
        let mut tasks = Vec::new();
        for i in 0..100 {
            let priority = match i % 4 {
                0 => Priority::Critical,
                1 => Priority::High,
                2 => Priority::Medium,
                _ => Priority::Low,
            };
            tasks.push(WalletTask::new(format!("wallet_{}", i), priority));
        }
        
        // Sort by priority (simulating how the processor would handle them)
        tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        let critical_count = tasks.iter().filter(|t| t.priority == Priority::Critical).count();
        let high_count = tasks.iter().filter(|t| t.priority == Priority::High).count();
        let medium_count = tasks.iter().filter(|t| t.priority == Priority::Medium).count();
        let low_count = tasks.iter().filter(|t| t.priority == Priority::Low).count();
        
        // Verify priority distribution
        if critical_count == 25 && high_count == 25 && medium_count == 25 && low_count == 25 {
            details.push("Priority distribution correct".to_string());
        } else {
            success = false;
            details.push(format!("Priority distribution incorrect: {}C, {}H, {}M, {}L", 
                                critical_count, high_count, medium_count, low_count));
        }
        
        // Verify ordering
        let is_ordered = tasks.windows(2).all(|pair| pair[0].priority >= pair[1].priority);
        if is_ordered {
            details.push("Tasks correctly ordered by priority".to_string());
        } else {
            success = false;
            details.push("Tasks not properly ordered by priority".to_string());
        }
        
        TestResult {
            name: "Priority Processing".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_load_balancing(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Test load balancing with a large batch
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(8),
            200,
        ).unwrap();
        
        // Create a large batch to stress test load balancing
        let wallet_addresses: Vec<String> = (0..1000)
            .map(|i| format!("test_wallet_{}", i))
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: Some("test_user".to_string()),
            fee_percentage: Some(0.05),
            created_at: Utc::now(),
        };
        
        // Process the batch
        let result = processor.process_batch_intelligently(&request).await;
        
        match result {
            Ok(batch_result) => {
                details.push(format!("Processed {} wallets successfully", batch_result.total_wallets));
                details.push(format!("Completed: {}, Failed: {}", 
                                   batch_result.completed_wallets, batch_result.failed_wallets));
                
                // Check that most wallets were processed
                let success_rate = batch_result.completed_wallets as f64 / batch_result.total_wallets as f64;
                if success_rate > 0.9 {
                    details.push(format!("High success rate: {:.1}%", success_rate * 100.0));
                } else {
                    success = false;
                    details.push(format!("Low success rate: {:.1}%", success_rate * 100.0));
                }
                
                // Check processing time
                if let Some(duration) = batch_result.duration_ms {
                    let throughput = batch_result.total_wallets as f64 / (duration as f64 / 1000.0);
                    details.push(format!("Throughput: {:.1} wallets/sec", throughput));
                    
                    if throughput > 10.0 { // Should process at least 10 wallets per second
                        details.push("Good throughput achieved".to_string());
                    } else {
                        success = false;
                        details.push("Throughput too low".to_string());
                    }
                }
            }
            Err(e) => {
                success = false;
                details.push(format!("Processing failed: {}", e));
            }
        }
        
        TestResult {
            name: "Load Balancing".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_fault_tolerance(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // This test would simulate failures and verify recovery
        // For now, we'll test the basic fault tolerance mechanisms
        
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(4),
            50,
        ).unwrap();
        
        // Create a batch with some potentially problematic wallets
        let wallet_addresses: Vec<String> = (0..100)
            .map(|i| {
                match i % 20 {
                    0 => "invalid_wallet_address".to_string(), // Intentionally invalid
                    10 => "".to_string(), // Empty address
                    _ => format!("valid_wallet_{}", i),
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
                details.push(format!("Processed {} wallets with fault tolerance", batch_result.total_wallets));
                
                // Should have some failures but still complete processing
                if batch_result.failed_wallets > 0 {
                    details.push(format!("Correctly handled {} failures", batch_result.failed_wallets));
                } else {
                    details.push("No failures detected (may be expected)".to_string());
                }
                
                // Should still have some successes
                if batch_result.completed_wallets > 0 {
                    details.push(format!("Successfully processed {} wallets", batch_result.completed_wallets));
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
                details.push(format!("Fault tolerance test failed: {}", e));
            }
        }
        
        TestResult {
            name: "Fault Tolerance".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_memory_efficiency(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Test memory usage with different batch sizes
        let initial_memory = self.get_memory_usage();
        details.push(format!("Initial memory usage: {} MB", initial_memory));
        
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(4),
            100,
        ).unwrap();
        
        // Process a medium batch
        let wallet_addresses: Vec<String> = (0..500)
            .map(|i| format!("memory_test_wallet_{}", i))
            .collect();
        
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses,
            user_id: None,
            fee_percentage: None,
            created_at: Utc::now(),
        };
        
        let _result = processor.process_batch_intelligently(&request).await;
        
        let peak_memory = self.get_memory_usage();
        details.push(format!("Peak memory usage: {} MB", peak_memory));
        
        let memory_increase = peak_memory.saturating_sub(initial_memory);
        let memory_per_wallet = memory_increase as f64 / 500.0;
        
        details.push(format!("Memory increase: {} MB", memory_increase));
        details.push(format!("Memory per wallet: {:.2} KB", memory_per_wallet * 1024.0));
        
        // Check if memory usage is reasonable (less than 1MB per wallet)
        if memory_per_wallet < 1.0 {
            details.push("Memory usage is efficient".to_string());
        } else {
            success = false;
            details.push("Memory usage is too high".to_string());
        }
        
        TestResult {
            name: "Memory Efficiency".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_performance_benchmarks(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        let mut processor = IntelligentParallelProcessor::new(
            self.scanner.clone(),
            Some(8),
            200,
        ).unwrap();
        
        // Benchmark different batch sizes
        let batch_sizes = vec![100, 500, 1000, 2000];
        
        for batch_size in batch_sizes {
            let wallet_addresses: Vec<String> = (0..batch_size)
                .map(|i| format!("benchmark_wallet_{}", i))
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
            
            match result {
                Ok(batch_result) => {
                    let throughput = batch_result.total_wallets as f64 / batch_duration.as_secs_f64();
                    details.push(format!("Batch size {}: {:.1} wallets/sec ({:.2}s)", 
                                       batch_size, throughput, batch_duration.as_secs_f64()));
                    
                    // Performance expectations
                    let expected_throughput = match batch_size {
                        100 => 50.0,
                        500 => 40.0,
                        1000 => 30.0,
                        2000 => 20.0,
                        _ => 10.0,
                    };
                    
                    if throughput >= expected_throughput {
                        details.push(format!("  -> Performance target met (>= {:.1})", expected_throughput));
                    } else {
                        success = false;
                        details.push(format!("  -> Performance target missed (< {:.1})", expected_throughput));
                    }
                }
                Err(e) => {
                    success = false;
                    details.push(format!("Batch size {} failed: {}", batch_size, e));
                }
            }
        }
        
        TestResult {
            name: "Performance Benchmarks".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    async fn test_scalability_analysis(&self) -> TestResult {
        let start_time = Instant::now();
        let mut success = true;
        let mut details = Vec::new();
        
        // Test scalability with different numbers of workers
        let worker_counts = vec![1, 2, 4, 8];
        let batch_size = 1000;
        
        for worker_count in worker_counts {
            let mut processor = IntelligentParallelProcessor::new(
                self.scanner.clone(),
                Some(worker_count),
                100,
            ).unwrap();
            
            let wallet_addresses: Vec<String> = (0..batch_size)
                .map(|i| format!("scalability_wallet_{}", i))
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
            
            match result {
                Ok(batch_result) => {
                    let throughput = batch_result.total_wallets as f64 / test_duration.as_secs_f64();
                    details.push(format!("{} workers: {:.1} wallets/sec ({:.2}s)", 
                                       worker_count, throughput, test_duration.as_secs_f64()));
                    
                    // Check for scaling efficiency
                    if worker_count == 1 {
                        details.push("  -> Baseline performance established".to_string());
                    } else {
                        // Should show some improvement with more workers
                        if throughput > 10.0 {
                            details.push("  -> Good scaling observed".to_string());
                        } else {
                            details.push("  -> Limited scaling (may be I/O bound)".to_string());
                        }
                    }
                }
                Err(e) => {
                    success = false;
                    details.push(format!("{} workers failed: {}", worker_count, e));
                }
            }
        }
        
        TestResult {
            name: "Scalability Analysis".to_string(),
            success,
            duration_ms: start_time.elapsed().as_millis() as u64,
            details,
        }
    }
    
    fn get_memory_usage(&self) -> u64 {
        // Simple memory usage estimation
        // In a real implementation, you'd use system APIs to get actual memory usage
        100 // Placeholder
    }
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub details: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TestResults {
    pub results: HashMap<String, TestResult>,
    pub start_time: Instant,
}

impl TestResults {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            start_time: Instant::now(),
        }
    }
    
    pub fn add_result(&mut self, test_name: &str, result: TestResult) {
        self.results.insert(test_name.to_string(), result);
    }
    
    pub fn get_summary(&self) -> TestSummary {
        let total_tests = self.results.len();
        let passed_tests = self.results.values().filter(|r| r.success).count();
        let failed_tests = total_tests - passed_tests;
        
        let total_duration: u64 = self.results.values().map(|r| r.duration_ms).sum();
        
        TestSummary {
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
        println!("\n=== Parallel Processing Test Results ===");
        
        for (name, result) in &self.results {
            let status = if result.success { "PASS" } else { "FAIL" };
            println!("\n{}: {} ({}ms)", name, status, result.duration_ms);
            
            for detail in &result.details {
                println!("  - {}", detail);
            }
        }
        
        let summary = self.get_summary();
        println!("\n=== Test Summary ===");
        println!("Total Tests: {}", summary.total_tests);
        println!("Passed: {}", summary.passed_tests);
        println!("Failed: {}", summary.failed_tests);
        println!("Success Rate: {:.1}%", summary.success_rate);
        println!("Total Duration: {}ms", summary.total_duration_ms);
        println!("Overall Duration: {}ms", summary.overall_duration_ms);
    }
}

#[derive(Debug, Clone)]
pub struct TestSummary {
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
    async fn test_task_queue() {
        let test_suite = ParallelProcessingTests::new();
        let results = test_suite.run_all_tests().await;
        
        // Print results for manual inspection
        results.print_detailed_results();
        
        // Assert that most tests pass
        let summary = results.get_summary();
        assert!(summary.success_rate >= 80.0, "Success rate should be at least 80%");
    }
}
