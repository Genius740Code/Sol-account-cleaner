use crate::core::SolanaRecoverError;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{warn, error, debug, info};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub jitter: bool,
    pub retryable_errors: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            jitter: true,
            retryable_errors: vec![
                "timeout".to_string(),
                "connection".to_string(),
                "network".to_string(),
                "rate_limit".to_string(),
                "temporary".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout_ms: u64,
    pub recovery_timeout_ms: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_ms: 60000,
            recovery_timeout_ms: 30000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub total_errors: u64,
    pub retryable_errors: u64,
    pub non_retryable_errors: u64,
    pub circuit_breaker_trips: u64,
    pub average_retry_attempts: f64,
    pub error_rate: f64,
    pub last_error_time: Option<DateTime<Utc>>,
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self {
            total_errors: 0,
            retryable_errors: 0,
            non_retryable_errors: 0,
            circuit_breaker_trips: 0,
            average_retry_attempts: 0.0,
            error_rate: 0.0,
            last_error_time: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerInternal>>,
}

#[derive(Debug)]
struct CircuitBreakerInternal {
    state: CircuitBreakerState,
    failures: u32,
    successes: u32,
    last_failure_time: Option<DateTime<Utc>>,
    last_state_change: DateTime<Utc>,
}

impl CircuitBreaker {
    pub fn new(name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            name,
            config,
            state: Arc::new(RwLock::new(CircuitBreakerInternal {
                state: CircuitBreakerState::Closed,
                failures: 0,
                successes: 0,
                last_failure_time: None,
                last_state_change: Utc::now(),
            })),
        }
    }
    
    pub async fn execute<F, T>(&self, operation: F) -> crate::core::Result<T>
    where
        F: FnOnce() -> BoxFuture<'static, crate::core::Result<T>>,
    {
        // Check circuit breaker state
        let mut state_guard = self.state.write().await;
        
        match state_guard.state {
            CircuitBreakerState::Open => {
                // Check if we should transition to half-open
                if state_guard.last_failure_time
                    .map(|time| Utc::now().signed_duration_since(time).num_milliseconds() > self.config.recovery_timeout_ms as i64)
                    .unwrap_or(false)
                {
                    state_guard.state = CircuitBreakerState::HalfOpen;
                    state_guard.last_state_change = Utc::now();
                    debug!("Circuit breaker '{}' transitioning to half-open", self.name);
                } else {
                    return Err(self.create_circuit_breaker_error("Circuit breaker is open"));
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Allow a single request through to test recovery
            }
            CircuitBreakerState::Closed => {
                // Normal operation
            }
        }
        
        drop(state_guard);
        
        // Execute the operation
        let result = operation().await;
        
        // Update circuit breaker state based on result
        let mut state_guard = self.state.write().await;
        
        match result {
            Ok(value) => {
                match state_guard.state {
                    CircuitBreakerState::HalfOpen => {
                        state_guard.successes += 1;
                        if state_guard.successes >= self.config.success_threshold {
                            state_guard.state = CircuitBreakerState::Closed;
                            state_guard.failures = 0;
                            state_guard.successes = 0;
                            state_guard.last_state_change = Utc::now();
                            info!("Circuit breaker '{}' closed after successful recovery", self.name);
                        }
                    }
                    CircuitBreakerState::Closed => {
                        // Reset failure count on success
                        state_guard.failures = 0;
                    }
                    CircuitBreakerState::Open => {
                        // Shouldn't happen, but handle gracefully
                    }
                }
                Ok(value)
            }
            Err(error) => {
                state_guard.failures += 1;
                state_guard.last_failure_time = Some(Utc::now());
                
                match state_guard.state {
                    CircuitBreakerState::Closed => {
                        if state_guard.failures >= self.config.failure_threshold {
                            state_guard.state = CircuitBreakerState::Open;
                            state_guard.last_state_change = Utc::now();
                            warn!("Circuit breaker '{}' opened after {} failures", self.name, state_guard.failures);
                        }
                    }
                    CircuitBreakerState::HalfOpen => {
                        state_guard.state = CircuitBreakerState::Open;
                        state_guard.last_state_change = Utc::now();
                        warn!("Circuit breaker '{}' re-opened during half-open state", self.name);
                    }
                    CircuitBreakerState::Open => {
                        // Shouldn't happen, but handle gracefully
                    }
                }
                
                Err(error)
            }
        }
    }
    
    fn create_circuit_breaker_error<T>(&self, message: &str) -> T {
        // This would need to be adapted based on your error type
        // For now, we'll create a generic error
        panic!("Circuit breaker error: {}", message);
    }
    
    pub async fn get_state(&self) -> CircuitBreakerState {
        self.state.read().await.state.clone()
    }
    
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        let state_guard = self.state.read().await;
        CircuitBreakerStats {
            name: self.name.clone(),
            state: state_guard.state.clone(),
            failures: state_guard.failures,
            successes: state_guard.successes,
            last_failure_time: state_guard.last_failure_time,
            last_state_change: state_guard.last_state_change,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStats {
    pub name: String,
    pub state: CircuitBreakerState,
    pub failures: u32,
    pub successes: u32,
    pub last_failure_time: Option<DateTime<Utc>>,
    pub last_state_change: DateTime<Utc>,
}

pub struct RetryHandler {
    config: RetryConfig,
    metrics: Arc<RwLock<ErrorMetrics>>,
}

impl RetryHandler {
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(RwLock::new(ErrorMetrics::default())),
        }
    }
    
    pub async fn execute_with_retry<F, T>(&self, operation: F) -> crate::core::Result<T>
    where
        F: Fn() -> BoxFuture<'static, crate::core::Result<T>>,
    {
        let mut last_error = None;
        let mut _total_attempts = 0;
        
        for attempt in 1..=self.config.max_attempts {
            _total_attempts += 1;
            
            debug!("Executing operation, attempt {}/{}", attempt, self.config.max_attempts);
            
            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        info!("Operation succeeded on attempt {}", attempt);
                    }
                    
                    // Update metrics
                    let mut metrics = self.metrics.write().await;
                    if attempt > 1 {
                        let total_retries = metrics.total_errors + (attempt - 1) as u64;
                        metrics.average_retry_attempts = (metrics.average_retry_attempts * metrics.total_errors as f64 + (attempt - 1) as f64) / total_retries as f64;
                    }
                    
                    return Ok(result);
                }
                Err(error) => {
                    let error_str = error.to_string();
                    last_error = Some(error_str.clone());
                    
                    // Update metrics
                    let mut metrics = self.metrics.write().await;
                    metrics.total_errors += 1;
                    metrics.last_error_time = Some(Utc::now());
                    
                    if self.is_retryable_error(&error_str) {
                        metrics.retryable_errors += 1;
                        warn!("Operation failed on attempt {}: {}", attempt, error_str);
                        
                        if attempt < self.config.max_attempts {
                            let delay = self.calculate_delay(attempt);
                            debug!("Waiting {:?} before retry", delay);
                            sleep(delay).await;
                        }
                    } else {
                        metrics.non_retryable_errors += 1;
                        error!("Non-retryable error: {}", error_str);
                        return Err(error);
                    }
                }
            }
        }
        
        error!("Operation failed after {} attempts", self.config.max_attempts);
        Err(SolanaRecoverError::InternalError(last_error.unwrap()))
    }
    
    fn is_retryable_error(&self, error: &str) -> bool {
        self.config.retryable_errors.iter().any(|retryable| {
            error.to_lowercase().contains(&retryable.to_lowercase())
        })
    }
    
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let mut delay = self.config.base_delay_ms as f64 * self.config.backoff_multiplier.powi(attempt as i32 - 1);
        delay = delay.min(self.config.max_delay_ms as f64);
        
        if self.config.jitter {
            // Add ±25% jitter
            let jitter_factor = 0.75 + (fastrand::f64() * 0.5);
            delay *= jitter_factor;
        }
        
        Duration::from_millis(delay as u64)
    }
    
    pub async fn get_metrics(&self) -> ErrorMetrics {
        self.metrics.read().await.clone()
    }
}

pub struct ErrorHandler {
    retry_handler: RetryHandler,
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    metrics: Arc<RwLock<ErrorMetrics>>,
}

impl ErrorHandler {
    pub fn new(retry_config: RetryConfig) -> Self {
        Self {
            retry_handler: RetryHandler::new(retry_config),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(ErrorMetrics::default())),
        }
    }
    
    pub async fn get_or_create_circuit_breaker(&self, name: &str, config: CircuitBreakerConfig) -> Arc<CircuitBreaker> {
        let mut breakers = self.circuit_breakers.write().await;
        
        if let Some(breaker) = breakers.get(name) {
            breaker.clone()
        } else {
            let breaker = Arc::new(CircuitBreaker::new(name.to_string(), config));
            breakers.insert(name.to_string(), breaker.clone());
            breaker
        }
    }
    
    pub async fn execute_with_protection<F, T>(
        &self,
        operation_name: &str,
        circuit_breaker_config: Option<CircuitBreakerConfig>,
        operation: F,
    ) -> crate::core::Result<T>
    where
        F: Fn() -> BoxFuture<'static, crate::core::Result<T>>,
    {
        // Get circuit breaker if configured
        let circuit_breaker = if let Some(config) = circuit_breaker_config {
            Some(self.get_or_create_circuit_breaker(operation_name, config).await)
        } else {
            None
        };
        
        // Execute with circuit breaker and retry
        let result = if let Some(breaker) = circuit_breaker {
            breaker.execute(|| operation()).await
        } else {
            self.retry_handler.execute_with_retry(|| operation()).await
        };
        
        // Update metrics
        let mut metrics = self.metrics.write().await;
        match &result {
            Ok(_) => {
                // Success - no error metrics update needed
            }
            Err(_) => {
                metrics.total_errors += 1;
                metrics.last_error_time = Some(Utc::now());
            }
        }
        
        result
    }
    
    pub async fn get_circuit_breaker_stats(&self) -> Vec<CircuitBreakerStats> {
        let breakers = self.circuit_breakers.read().await;
        let mut stats = Vec::new();
        
        for breaker in breakers.values() {
            stats.push(breaker.get_stats().await);
        }
        
        stats
    }
    
    pub async fn get_error_metrics(&self) -> ErrorMetrics {
        self.metrics.read().await.clone()
    }
    
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = ErrorMetrics::default();
    }
    
    pub async fn reset_circuit_breaker(&self, name: &str) -> bool {
        let breakers = self.circuit_breakers.write().await;
        
        if let Some(breaker) = breakers.get(name) {
            let mut state = breaker.state.write().await;
            state.state = CircuitBreakerState::Closed;
            state.failures = 0;
            state.successes = 0;
            state.last_failure_time = None;
            state.last_state_change = Utc::now();
            info!("Circuit breaker '{}' reset to closed state", name);
            true
        } else {
            false
        }
    }
}

// Utility functions for common error handling patterns
pub async fn with_retry<F, T>(config: RetryConfig, operation: F) -> crate::core::Result<T>
where
    F: Fn() -> BoxFuture<'static, crate::core::Result<T>>,
{
    let handler = RetryHandler::new(config);
    handler.execute_with_retry(operation).await
}

pub async fn with_circuit_breaker<F, T>(
    name: &str,
    config: CircuitBreakerConfig,
    operation: F,
) -> crate::core::Result<T>
where
    F: Fn() -> BoxFuture<'static, crate::core::Result<T>>,
{
    let breaker = CircuitBreaker::new(name.to_string(), config);
    breaker.execute(operation).await
}

pub async fn with_full_protection<F, T>(
    operation_name: &str,
    retry_config: RetryConfig,
    circuit_breaker_config: CircuitBreakerConfig,
    operation: F,
) -> crate::core::Result<T>
where
    F: Fn() -> BoxFuture<'static, crate::core::Result<T>>,
{
    let handler = ErrorHandler::new(retry_config);
    handler.execute_with_protection::<F, T>(operation_name, Some(circuit_breaker_config), operation).await
}

// Error classification utilities
pub fn classify_error(error: &str) -> ErrorClassification {
    let error_lower = error.to_lowercase();
    
    if error_lower.contains("timeout") || error_lower.contains("deadline") {
        ErrorClassification::Timeout
    } else if error_lower.contains("connection") || error_lower.contains("network") {
        ErrorClassification::Network
    } else if error_lower.contains("rate limit") || error_lower.contains("quota") {
        ErrorClassification::RateLimit
    } else if error_lower.contains("authentication") || error_lower.contains("unauthorized") {
        ErrorClassification::Authentication
    } else if error_lower.contains("permission") || error_lower.contains("forbidden") {
        ErrorClassification::Authorization
    } else if error_lower.contains("not found") || error_lower.contains("missing") {
        ErrorClassification::NotFound
    } else if error_lower.contains("validation") || error_lower.contains("invalid") {
        ErrorClassification::Validation
    } else if error_lower.contains("database") || error_lower.contains("storage") {
        ErrorClassification::Database
    } else if error_lower.contains("temporary") || error_lower.contains("transient") {
        ErrorClassification::Temporary
    } else {
        ErrorClassification::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum ErrorClassification {
    Timeout,
    Network,
    RateLimit,
    Authentication,
    Authorization,
    NotFound,
    Validation,
    Database,
    Temporary,
    Unknown,
}

impl ErrorClassification {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ErrorClassification::Timeout |
            ErrorClassification::Network |
            ErrorClassification::RateLimit |
            ErrorClassification::Database |
            ErrorClassification::Temporary
        )
    }
    
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            ErrorClassification::Authentication |
            ErrorClassification::Authorization |
            ErrorClassification::NotFound |
            ErrorClassification::Validation
        )
    }
    
    pub fn is_server_error(&self) -> bool {
        !self.is_client_error()
    }
}

// Error reporting and analytics
pub struct ErrorReporter {
    errors: Arc<RwLock<Vec<ErrorReport>>>,
    max_reports: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub error_type: String,
    pub error_message: String,
    pub classification: ErrorClassification,
    pub context: serde_json::Value,
    pub stack_trace: Option<String>,
    pub user_id: Option<String>,
    pub request_id: Option<String>,
}

impl ErrorReporter {
    pub fn new(max_reports: usize) -> Self {
        Self {
            errors: Arc::new(RwLock::new(Vec::with_capacity(max_reports))),
            max_reports,
        }
    }
    
    pub async fn report_error(
        &self,
        error_type: &str,
        error_message: &str,
        classification: ErrorClassification,
        context: serde_json::Value,
    ) {
        let report = ErrorReport {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            error_type: error_type.to_string(),
            error_message: error_message.to_string(),
            classification,
            context,
            stack_trace: None, // Would capture actual stack trace in production
            user_id: None,
            request_id: None,
        };
        
        let mut errors = self.errors.write().await;
        errors.push(report);
        
        // Keep only the most recent reports
        if errors.len() > self.max_reports {
            let drain_count = errors.len() - self.max_reports;
            errors.drain(0..drain_count);
        }
    }
    
    pub async fn get_recent_errors(&self, limit: usize) -> Vec<ErrorReport> {
        let errors = self.errors.read().await;
        errors.iter().rev().take(limit).cloned().collect()
    }
    
    pub async fn get_error_summary(&self) -> ErrorSummary {
        let errors = self.errors.read().await;
        
        let mut classification_counts = HashMap::new();
        let mut type_counts = HashMap::new();
        
        for error in errors.iter() {
            *classification_counts.entry(error.classification.clone()).or_insert(0) += 1;
            *type_counts.entry(error.error_type.clone()).or_insert(0) += 1;
        }
        
        ErrorSummary {
            total_errors: errors.len(),
            classification_counts,
            type_counts,
            most_recent_error: errors.last().cloned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub total_errors: usize,
    pub classification_counts: HashMap<ErrorClassification, usize>,
    pub type_counts: HashMap<String, usize>,
    pub most_recent_error: Option<ErrorReport>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_retry_handler() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay_ms: 10,
            ..Default::default()
        };
        
        let handler = RetryHandler::new(config);
        let mut attempt_count = 0;
        
        let result = handler.execute_with_retry(|| {
            Box::pin(async move {
                attempt_count += 1;
                if attempt_count < 3 {
                    Err::<&str, SolanaRecoverError>(SolanaRecoverError::NetworkError("temporary error".to_string()))
                } else {
                    Ok("success")
                }
            })
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_count, 3);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        
        let breaker = CircuitBreaker::new("test".to_string(), config);
        
        // First failure
        let result = breaker.execute(|| {
            Box::pin(async { Err::<(), SolanaRecoverError>(SolanaRecoverError::NetworkError("error".to_string())) })
        }).await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);
        
        // Second failure - should open circuit
        let result = breaker.execute(|| {
            Box::pin(async { Err::<(), SolanaRecoverError>(SolanaRecoverError::NetworkError("error".to_string())) })
        }).await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);
        
        // Third attempt - should be rejected
        let result = breaker.execute(|| {
            Box::pin(async { Ok(()) })
        }).await;
        assert!(result.is_err()); // Rejected by circuit breaker
    }
    
    #[tokio::test]
    async fn test_error_classification() {
        assert_eq!(classify_error("timeout occurred"), ErrorClassification::Timeout);
        assert_eq!(classify_error("connection refused"), ErrorClassification::Network);
        assert_eq!(classify_error("rate limit exceeded"), ErrorClassification::RateLimit);
        assert_eq!(classify_error("unauthorized access"), ErrorClassification::Authentication);
        assert_eq!(classify_error("permission denied"), ErrorClassification::Authorization);
        assert_eq!(classify_error("user not found"), ErrorClassification::NotFound);
        assert_eq!(classify_error("invalid input"), ErrorClassification::Validation);
        assert_eq!(classify_error("database error"), ErrorClassification::Database);
        assert_eq!(classify_error("temporary failure"), ErrorClassification::Temporary);
        assert_eq!(classify_error("unknown error"), ErrorClassification::Unknown);
    }
    
    #[tokio::test]
    async fn test_error_reporter() {
        let reporter = ErrorReporter::new(100);
        
        reporter.report_error(
            "TestError",
            "Test error message",
            ErrorClassification::Temporary,
            serde_json::json!({"key": "value"}),
        ).await;
        
        let summary = reporter.get_error_summary().await;
        assert_eq!(summary.total_errors, 1);
        assert_eq!(summary.classification_counts.get(&ErrorClassification::Temporary), Some(&1));
        assert_eq!(summary.type_counts.get("TestError"), Some(&1));
    }
}
