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
pub use processor::*;
pub use recovery::*;
pub use scanner::*;
pub use error_handling::*;
pub use unified_scanner::*;
pub use scanner_builder::*;
pub use error_recovery::*;
pub use config_management::*;

// Legacy exports (deprecated)
#[deprecated(note = "Use unified_scanner module instead")]
pub use enhanced_scanner::*;
#[deprecated(note = "Use unified_scanner module instead")]
pub use adaptive_parallel_processor::*;
#[deprecated(note = "Use unified_scanner module instead")]
pub use optimized_scanner::*;
#[deprecated(note = "Use unified_scanner module instead")]
pub use ultra_fast_scanner::*;
