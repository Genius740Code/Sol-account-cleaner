use crate::core::{Result, SolanaRecoverError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Rejecting all requests - service is considered down
    Open,
    /// Testing if service has recovered - allows limited requests
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// How long to wait before transitioning from Open to HalfOpen
    pub timeout: Duration,
    /// How long to wait in HalfOpen state before giving up
    pub recovery_timeout: Duration,
    /// Number of successful requests needed to close the circuit from HalfOpen
    pub success_threshold: u32,
    /// Maximum number of requests to allow in HalfOpen state
    pub max_half_open_requests: u32,
    /// Whether to track individual request types separately
    pub track_per_request_type: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 3,
            max_half_open_requests: 10,
            track_per_request_type: false,
        }
    }
}

/// Circuit breaker metrics
#[derive(Debug)]
pub struct CircuitBreakerMetrics {
    pub total_requests: AtomicU64,
    pub successful_requests: AtomicU64,
    pub failed_requests: AtomicU64,
    pub rejected_requests: AtomicU64,
    pub circuit_open_count: AtomicU64,
    pub last_failure_time: RwLock<Option<Instant>>,
    pub last_success_time: RwLock<Option<Instant>>,
}

impl Default for CircuitBreakerMetrics {
    fn default() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            rejected_requests: AtomicU64::new(0),
            circuit_open_count: AtomicU64::new(0),
            last_failure_time: RwLock::new(None),
            last_success_time: RwLock::new(None),
        }
    }
}

impl CircuitBreakerMetrics {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            let successful = self.successful_requests.load(Ordering::Relaxed);
            (successful as f64 / total as f64) * 100.0
        }
    }

    /// Calculate failure rate
    pub fn failure_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            let failed = self.failed_requests.load(Ordering::Relaxed);
            (failed as f64 / total as f64) * 100.0
        }
    }

    /// Get all metrics as a snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            successful_requests: self.successful_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            rejected_requests: self.rejected_requests.load(Ordering::Relaxed),
            circuit_open_count: self.circuit_open_count.load(Ordering::Relaxed),
            success_rate: self.success_rate(),
            failure_rate: self.failure_rate(),
            last_failure_time: None, // Cannot get snapshot of async lock
            last_success_time: None, // Cannot get snapshot of async lock
        }
    }
    
    /// Create a clone of the metrics (for sharing)
    pub async fn clone_metrics(&self) -> CircuitBreakerMetrics {
        let last_failure = self.last_failure_time.read().await;
        let last_success = self.last_success_time.read().await;
        CircuitBreakerMetrics {
            total_requests: AtomicU64::new(self.total_requests.load(Ordering::Relaxed)),
            successful_requests: AtomicU64::new(self.successful_requests.load(Ordering::Relaxed)),
            failed_requests: AtomicU64::new(self.failed_requests.load(Ordering::Relaxed)),
            rejected_requests: AtomicU64::new(self.rejected_requests.load(Ordering::Relaxed)),
            circuit_open_count: AtomicU64::new(self.circuit_open_count.load(Ordering::Relaxed)),
            last_failure_time: RwLock::new(*last_failure),
            last_success_time: RwLock::new(*last_success),
        }
    }
}

/// Metrics snapshot
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rejected_requests: u64,
    pub circuit_open_count: u64,
    pub success_rate: f64,
    pub failure_rate: f64,
    pub last_failure_time: Option<Instant>,
    pub last_success_time: Option<Instant>,
}

/// Circuit breaker for fault tolerance
pub struct CircuitBreaker {
    /// Current circuit state
    state: Arc<RwLock<CircuitState>>,
    /// Circuit breaker configuration
    config: CircuitBreakerConfig,
    /// Failure count
    failure_count: AtomicU32,
    /// Success count in HalfOpen state
    half_open_success_count: AtomicU32,
    /// HalfOpen request count
    half_open_request_count: AtomicU32,
    /// Last failure time
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    /// Circuit open time
    circuit_open_time: Arc<RwLock<Option<Instant>>>,
    /// Metrics
    metrics: Arc<CircuitBreakerMetrics>,
    /// Request type tracking (if enabled)
    request_type_breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            config,
            failure_count: AtomicU32::new(0),
            half_open_success_count: AtomicU32::new(0),
            half_open_request_count: AtomicU32::new(0),
            last_failure_time: Arc::new(RwLock::new(None)),
            circuit_open_time: Arc::new(RwLock::new(None)),
            metrics: Arc::new(CircuitBreakerMetrics::default()),
            request_type_breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Execute an operation with circuit breaker protection
    pub async fn execute<F, T>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        Box::pin(self.execute_with_type("default", operation)).await
    }

    /// Execute an operation with circuit breaker protection for a specific request type
    pub async fn execute_with_type<F, T>(&self, request_type: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        // Check if we should use a request-type-specific circuit breaker
        if self.config.track_per_request_type && request_type != "default" {
            let mut breakers = self.request_type_breakers.write().await;
            let _breaker = breakers.entry(request_type.to_string())
                .or_insert_with(|| Arc::new(CircuitBreaker::with_config(self.config.clone())));
            
            // Drop the write lock before executing
            drop(breakers);
            
            // Get the breaker and execute
            let breakers = self.request_type_breakers.read().await;
            if let Some(breaker) = breakers.get(request_type) {
                return breaker.execute(operation).await;
            }
        }

        // Check circuit state
        let state = self.state.read().await;
        
        let should_transition = match *state {
            CircuitState::Open => {
                // Check if we should transition to HalfOpen
                let last_failure = self.last_failure_time.read().await;
                if let Some(last) = *last_failure {
                    if last.elapsed() > self.config.timeout {
                        drop(last_failure);
                        true
                    } else {
                        self.metrics.rejected_requests.fetch_add(1, Ordering::Relaxed);
                        return Err(SolanaRecoverError::CircuitBreakerOpen(
                            "Circuit breaker is open - service unavailable".to_string()
                        ));
                    }
                } else {
                    false
                }
            }
            _ => false,
        };
        
        // Check if we're in HalfOpen and have exceeded max requests
        let is_half_open = *state == CircuitState::HalfOpen;
        drop(state);
        
        if should_transition {
            self.transition_to_half_open().await;
        }
        
        if is_half_open {
            let half_open_requests = self.half_open_request_count.load(Ordering::Relaxed);
            if half_open_requests >= self.config.max_half_open_requests {
                self.metrics.rejected_requests.fetch_add(1, Ordering::Relaxed);
                return Err(SolanaRecoverError::CircuitBreakerOpen(
                    "HalfOpen request limit exceeded".to_string()
                ));
            }
        }
        
        // Execute the operation
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);
        
        if is_half_open {
            self.half_open_request_count.fetch_add(1, Ordering::Relaxed);
        }
        
        let result = operation.await;
        
        match result {
            Ok(value) => {
                self.on_success().await;
                Ok(value)
            }
            Err(error) => {
                self.on_failure().await;
                Err(error)
            }
        }
    }

    /// Handle successful operation
    async fn on_success(&self) {
        let mut state = self.state.write().await;
        
        self.metrics.successful_requests.fetch_add(1, Ordering::Relaxed);
        {
            let mut last_success = self.metrics.last_success_time.write().await;
            *last_success = Some(Instant::now());
        }
        
        match *state {
            CircuitState::HalfOpen => {
                let success_count = self.half_open_success_count.fetch_add(1, Ordering::Relaxed) + 1;
                
                if success_count >= self.config.success_threshold {
                    // Close the circuit
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.half_open_success_count.store(0, Ordering::Relaxed);
                    self.half_open_request_count.store(0, Ordering::Relaxed);
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Should not happen, but handle gracefully
                *state = CircuitState::Closed;
                self.failure_count.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Handle failed operation
    async fn on_failure(&self) {
        let mut state = self.state.write().await;
        
        self.metrics.failed_requests.fetch_add(1, Ordering::Relaxed);
        {
            let mut last_failure = self.metrics.last_failure_time.write().await;
            *last_failure = Some(Instant::now());
        }
        
        match *state {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                
                if count >= self.config.failure_threshold {
                    // Open the circuit
                    *state = CircuitState::Open;
                    let mut circuit_open_time = self.circuit_open_time.write().await;
                    *circuit_open_time = Some(Instant::now());
                    drop(circuit_open_time);
                    self.metrics.circuit_open_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            CircuitState::HalfOpen => {
                // Immediately open on failure in HalfOpen
                *state = CircuitState::Open;
                let mut circuit_open_time = self.circuit_open_time.write().await;
                *circuit_open_time = Some(Instant::now());
                drop(circuit_open_time);
                self.metrics.circuit_open_count.fetch_add(1, Ordering::Relaxed);
                self.half_open_success_count.store(0, Ordering::Relaxed);
                self.half_open_request_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }

    /// Transition to HalfOpen state
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::HalfOpen;
        self.half_open_success_count.store(0, Ordering::Relaxed);
        self.half_open_request_count.store(0, Ordering::Relaxed);
    }

    /// Force the circuit breaker to open (for testing/admin)
    pub async fn force_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open;
        let mut circuit_open_time = self.circuit_open_time.write().await;
        *circuit_open_time = Some(Instant::now());
        drop(circuit_open_time);
        self.metrics.circuit_open_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Force the circuit breaker to close (for testing/admin)
    pub async fn force_close(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_success_count.store(0, Ordering::Relaxed);
        self.half_open_request_count.store(0, Ordering::Relaxed);
    }

    /// Get current circuit state
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Get circuit breaker metrics
    pub fn get_metrics(&self) -> Arc<CircuitBreakerMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Reset all metrics and state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_success_count.store(0, Ordering::Relaxed);
        self.half_open_request_count.store(0, Ordering::Relaxed);
        
        self.metrics.total_requests.store(0, Ordering::Relaxed);
        self.metrics.successful_requests.store(0, Ordering::Relaxed);
        self.metrics.failed_requests.store(0, Ordering::Relaxed);
        self.metrics.rejected_requests.store(0, Ordering::Relaxed);
        self.metrics.circuit_open_count.store(0, Ordering::Relaxed);
        
        let mut last_failure_time = self.metrics.last_failure_time.write().await;
        *last_failure_time = None;
        drop(last_failure_time);
        let mut last_success_time = self.metrics.last_success_time.write().await;
        *last_success_time = None;
        drop(last_success_time);
        let mut last_failure_time2 = self.last_failure_time.write().await;
        *last_failure_time2 = None;
        drop(last_failure_time2);
        let mut circuit_open_time = self.circuit_open_time.write().await;
        *circuit_open_time = None;
        drop(circuit_open_time);
    }

    /// Check if the circuit breaker is currently allowing requests
    pub async fn is_allowing_requests(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => {
                let half_open_requests = self.half_open_request_count.load(Ordering::Relaxed);
                half_open_requests < self.config.max_half_open_requests
            }
            CircuitState::Open => false,
        }
    }

    /// Get time until next state change (if applicable)
    pub async fn time_to_next_state(&self) -> Option<Duration> {
        let state = self.state.read().await;
        match *state {
            CircuitState::Open => {
                let last_failure = self.last_failure_time.read().await;
                last_failure.map(|last| {
                    let elapsed = last.elapsed();
                    if elapsed < self.config.timeout {
                        self.config.timeout - elapsed
                    } else {
                        Duration::ZERO
                    }
                })
            }
            _ => None,
        }
    }
}

/// Circuit breaker manager for multiple breakers
pub struct CircuitBreakerManager {
    breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    /// Create a new circuit breaker manager
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config,
        }
    }

    /// Get or create a circuit breaker for a specific service
    pub async fn get_breaker(&self, service_name: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.write().await;
        
        breakers.entry(service_name.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::with_config(self.default_config.clone())))
            .clone()
    }

    /// Execute operation with service-specific circuit breaker
    pub async fn execute<F, T>(&self, service_name: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let breaker = self.get_breaker(service_name).await;
        breaker.execute(operation).await
    }

    /// Get all circuit breaker states
    pub async fn get_all_states(&self) -> std::collections::HashMap<String, CircuitState> {
        let breakers = self.breakers.read().await;
        let mut states = std::collections::HashMap::new();
        
        for (name, breaker) in breakers.iter() {
            states.insert(name.clone(), breaker.get_state().await);
        }
        
        states
    }

    /// Reset all circuit breakers
    pub async fn reset_all(&self) {
        let breakers = self.breakers.read().await;
        for breaker in breakers.values() {
            breaker.reset().await;
        }
    }

    /// Close all circuit breakers
    pub async fn close_all(&self) {
        let breakers = self.breakers.read().await;
        for breaker in breakers.values() {
            breaker.force_close().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_basic_operation() {
        let breaker = CircuitBreaker::with_config(CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_millis(100),
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 2,
            max_half_open_requests: 5,
            track_per_request_type: false,
        });

        // Should succeed initially
        let result = breaker.execute(async { Ok(42) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        // Should be in Closed state
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_failure_threshold() {
        let breaker = CircuitBreaker::with_config(CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 2,
            max_half_open_requests: 5,
            track_per_request_type: false,
        });

        // Fail twice to open the circuit
        for _ in 0..2 {
            let result: Result<String> = breaker.execute(async { 
                Err(SolanaRecoverError::NetworkError("Test error".to_string()))
            }).await;
            assert!(result.is_err());
        }

        // Circuit should be open
        assert_eq!(breaker.get_state().await, CircuitState::Open);

        // Next request should be rejected
        let result = breaker.execute(async { Ok(42) }).await;
        assert!(matches!(result, Err(SolanaRecoverError::CircuitBreakerOpen(_))));
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let breaker = CircuitBreaker::with_config(CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(50),
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 2,
            max_half_open_requests: 5,
            track_per_request_type: false,
        });

        // Fail twice to open the circuit
        for _ in 0..2 {
            let _: Result<String> = breaker.execute(async { 
                Err(SolanaRecoverError::NetworkError("Test error".to_string()))
            }).await;
        }

        // Circuit should be open
        assert_eq!(breaker.get_state().await, CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(60)).await;

        // Next request should succeed (HalfOpen -> success -> Closed)
        let result = breaker.execute(async { Ok(42) }).await;
        assert!(result.is_ok());

        // Should still be in HalfOpen, need more successes
        assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);

        // Another success should close the circuit
        let result = breaker.execute(async { Ok(42) }).await;
        assert!(result.is_ok());

        // Should be closed now
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_manager() {
        let manager = CircuitBreakerManager::new(CircuitBreakerConfig::default());

        // Execute operations for different services
        let result1 = manager.execute("service1", async { Ok(1) }).await;
        let result2 = manager.execute("service2", async { Ok(2) }).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Check states
        let states = manager.get_all_states().await;
        assert_eq!(states.get("service1"), Some(&CircuitState::Closed));
        assert_eq!(states.get("service2"), Some(&CircuitState::Closed));
    }

    #[tokio::test]
    async fn test_circuit_breaker_metrics() {
        let breaker = CircuitBreaker::new();

        // Execute some operations
        for i in 0..5 {
            let result = if i < 3 {
                Ok(i)
            } else {
                Err(SolanaRecoverError::NetworkError("Test error".to_string()))
            };
            
            let _ = breaker.execute(async { result }).await;
        }

        let metrics = breaker.get_metrics();
        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.total_requests, 5);
        assert_eq!(snapshot.successful_requests, 3);
        assert_eq!(snapshot.failed_requests, 2);
        assert_eq!(snapshot.success_rate, 60.0);
        assert_eq!(snapshot.failure_rate, 40.0);
    }
}
