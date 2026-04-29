// Core modules
pub mod error_handling;
pub mod errors;
pub mod fee_calculator;
pub mod processor;
pub mod processor_metrics;
pub mod recovery;
pub mod scanner;
pub mod types;

// Unified scanner architecture (consolidates multiple scanner modules)
pub mod unified_scanner;
pub mod scanner_builder;
pub mod error_recovery;
pub mod config_management;

// Legacy scanner modules (deprecated - use unified_scanner instead)
// These are kept for backward compatibility but will be removed in future versions
pub mod enhanced_scanner;
pub mod adaptive_parallel_processor;
pub mod optimized_scanner;
pub mod ultra_fast_scanner;

#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod scanner_tests;
#[cfg(test)]
mod unified_scanner_tests;
#[cfg(test)]
mod tests;

// Public exports - unified architecture
pub use types::*;
pub use errors::*;
pub use fee_calculator::*;
// Import specific items from processor to avoid conflicts
pub use processor::{BatchProcessor, ProcessorConfig};
pub use recovery::*;
pub use scanner::*;
// Import specific items from error_handling to avoid conflicts
pub use error_handling::{RetryConfig, CircuitBreakerConfig, ErrorMetrics, CircuitBreakerState, CircuitBreaker, ErrorHandler, ErrorClassification, ErrorReporter, ErrorReport, ErrorSummary};
// Import specific items from unified_scanner to avoid conflicts
pub use unified_scanner::{PerformanceMode as UnifiedPerformanceMode, ScanStrategy, UnifiedScannerConfig, ScanContext, UnifiedWalletScanner};
pub use scanner_builder::*;
// Import specific items from error_recovery to avoid conflicts  
pub use error_recovery::{CircuitState, CircuitBreakerConfig as RecoveryCircuitConfig, RetryPolicy, CircuitBreaker as RecoveryCircuitBreaker, RetryMechanism, ResilientScanner};
pub use config_management::*;

// Legacy exports (deprecated)
#[deprecated(note = "Use unified_scanner module instead")]
pub use enhanced_scanner::*;
#[deprecated(note = "Use unified_scanner module instead")]
pub use adaptive_parallel_processor::{AdaptiveParallelProcessor, ProcessorConfig as LegacyProcessorConfig};
#[deprecated(note = "Use unified_scanner module instead")]
pub use optimized_scanner::{OptimizedWalletScanner, PerformanceMode as OptimizedPerformanceMode};
#[deprecated(note = "Use unified_scanner module instead")]
pub use ultra_fast_scanner::*;
