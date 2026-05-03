//! # NFT Module Error Types
//!
//! Comprehensive error handling for the NFT module with detailed error
//! classification and recovery strategies.

use crate::core::SolanaRecoverError;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Comprehensive error type for NFT operations
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum NftError {
    /// Configuration-related errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Network/RPC related errors
    #[error("Network error: {message}")]
    Network { message: String, source_info: Option<String> },

    /// Metadata fetching errors
    #[error("Metadata error: {message}")]
    Metadata { 
        message: String,
        mint_address: Option<String>,
        metadata_uri: Option<String>,
    },

    /// Validation errors
    #[error("Validation error: {message}")]
    Validation { 
        message: String,
        field: Option<String>,
        value: Option<String>,
    },

    /// Security-related errors
    #[error("Security error: {message}")]
    Security { 
        message: String,
        risk_level: RiskLevel,
        details: Option<String>,
    },

    /// Valuation errors
    #[error("Valuation error: {message}")]
    Valuation { 
        message: String,
        mint_address: Option<String>,
        method: Option<String>,
    },

    /// Cache errors
    #[error("Cache error: {message}")]
    Cache { message: String, operation: Option<String> },

    /// Batch processing errors
    #[error("Batch error: {message}")]
    Batch { 
        message: String,
        batch_id: Option<String>,
        failed_items: Option<u32>,
        total_items: Option<u32>,
    },

    /// Portfolio analysis errors
    #[error("Portfolio error: {message}")]
    Portfolio { message: String, wallet_address: Option<String> },

    /// Strategy errors
    #[error("Strategy error: {message}")]
    Strategy { 
        message: String,
        strategy_name: Option<String>,
        context: Option<String>,
    },

    /// Feature disabled error
    #[error("Feature '{feature}' is disabled. Enable with cargo feature flag.")]
    FeatureDisabled { feature: String },

    /// Timeout errors
    #[error("Operation timed out after {timeout_ms}ms: {message}")]
    Timeout { 
        message: String,
        timeout_ms: u64,
        operation: Option<String>,
    },

    /// Rate limiting errors
    #[error("Rate limit exceeded: {message}")]
    RateLimit { 
        message: String,
        retry_after_ms: Option<u64>,
        limit_type: Option<String>,
    },

    /// Resource exhaustion errors
    #[error("Resource exhausted: {message}")]
    ResourceExhausted { 
        message: String,
        resource_type: String,
        current_usage: Option<u64>,
        limit: Option<u64>,
    },

    /// Serialization/deserialization errors
    #[error("Serialization error: {message}")]
    Serialization { 
        message: String,
        format: Option<String>,
        data_type: Option<String>,
    },

    /// Authentication/authorization errors
    #[error("Authentication error: {message}")]
    Authentication { 
        message: String,
        service: Option<String>,
        required_permissions: Option<Vec<String>>,
    },

    /// Generic error with context
    #[error("NFT operation failed: {message}")]
    Generic { 
        message: String,
        context: Option<String>,
        error_code: Option<String>,
    },
}

/// Risk level classification for security issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub enum RiskLevel {
    /// No risk detected
    None,
    /// Low risk, minor concerns
    Low,
    /// Medium risk, caution advised
    Medium,
    /// High risk, dangerous
    High,
    /// Critical risk, extremely dangerous
    Critical,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::None => write!(f, "None"),
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}

/// Error recovery strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Retry the operation with exponential backoff
    RetryWithBackoff { base_delay_ms: u64, max_retries: u32 },
    /// Use alternative endpoint/method
    UseAlternative,
    /// Skip the problematic item and continue
    SkipAndContinue,
    /// Abort the entire operation
    Abort,
    /// Fall back to simpler method
    Fallback { method: String },
}

impl NftError {
    /// Get the risk level for security-related errors
    pub fn risk_level(&self) -> RiskLevel {
        match self {
            NftError::Security { risk_level, .. } => *risk_level,
            NftError::Validation { .. } => RiskLevel::Medium,
            NftError::Authentication { .. } => RiskLevel::High,
            NftError::Network { .. } => RiskLevel::Low,
            _ => RiskLevel::None,
        }
    }

    /// Determine if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            NftError::Network { .. } |
            NftError::Timeout { .. } |
            NftError::RateLimit { .. } |
            NftError::ResourceExhausted { .. }
        )
    }

    /// Get suggested recovery strategy
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            NftError::Network { .. } | NftError::Timeout { .. } => {
                RecoveryStrategy::RetryWithBackoff { base_delay_ms: 1000, max_retries: 3 }
            },
            NftError::RateLimit { retry_after_ms, .. } => {
                RecoveryStrategy::RetryWithBackoff { 
                    base_delay_ms: retry_after_ms.unwrap_or(5000), 
                    max_retries: 2 
                }
            },
            NftError::Metadata { .. } | NftError::Valuation { .. } => {
                RecoveryStrategy::SkipAndContinue
            },
            NftError::Security { risk_level, .. } => {
                if *risk_level >= RiskLevel::High {
                    RecoveryStrategy::Abort
                } else {
                    RecoveryStrategy::SkipAndContinue
                }
            },
            NftError::Configuration { .. } | NftError::FeatureDisabled { .. } => {
                RecoveryStrategy::Abort
            },
            _ => RecoveryStrategy::RetryWithBackoff { base_delay_ms: 500, max_retries: 2 },
        }
    }

    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            NftError::Configuration { .. } => "configuration",
            NftError::Network { .. } => "network",
            NftError::Metadata { .. } => "metadata",
            NftError::Validation { .. } => "validation",
            NftError::Security { .. } => "security",
            NftError::Valuation { .. } => "valuation",
            NftError::Cache { .. } => "cache",
            NftError::Batch { .. } => "batch",
            NftError::Portfolio { .. } => "portfolio",
            NftError::Strategy { .. } => "strategy",
            NftError::FeatureDisabled { .. } => "feature_disabled",
            NftError::Timeout { .. } => "timeout",
            NftError::RateLimit { .. } => "rate_limit",
            NftError::ResourceExhausted { .. } => "resource_exhausted",
            NftError::Serialization { .. } => "serialization",
            NftError::Authentication { .. } => "authentication",
            NftError::Generic { .. } => "generic",
        }
    }

    /// Convert to core SolanaRecoverError
    pub fn to_core_error(self) -> SolanaRecoverError {
        SolanaRecoverError::NftError(format!("{}", self))
    }
}

/// Result type for NFT operations
pub type NftResult<T> = Result<T, NftError>;

/// Error metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub error_count: u64,
    pub error_rate: f64,
    pub errors_by_category: std::collections::HashMap<String, u64>,
    pub retryable_errors: u64,
    pub critical_errors: u64,
    pub last_error_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl ErrorMetrics {
    pub fn new() -> Self {
        Self {
            error_count: 0,
            error_rate: 0.0,
            errors_by_category: std::collections::HashMap::new(),
            retryable_errors: 0,
            critical_errors: 0,
            last_error_time: None,
        }
    }

    pub fn record_error(&mut self, error: &NftError) {
        self.error_count += 1;
        self.last_error_time = Some(chrono::Utc::now());
        
        let category = error.category().to_string();
        *self.errors_by_category.entry(category).or_insert(0) += 1;
        
        if error.is_retryable() {
            self.retryable_errors += 1;
        }
        
        if error.risk_level() >= RiskLevel::High {
            self.critical_errors += 1;
        }
    }

    pub fn calculate_rate(&mut self, total_operations: u64) {
        self.error_rate = if total_operations > 0 {
            self.error_count as f64 / total_operations as f64
        } else {
            0.0
        };
    }
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self::new()
    }
}
