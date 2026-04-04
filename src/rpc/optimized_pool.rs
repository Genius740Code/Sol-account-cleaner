use crate::core::{Result, SolanaRecoverError, RpcEndpoint};
use crate::rpc::RpcClientWrapper;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use crossbeam::queue::SegQueue;
use flume::{Sender, Receiver, bounded};
use smallvec::SmallVec;
use once_cell::sync::Lazy;
use dashmap::DashMap;

#[derive(Debug, Clone)]
pub struct OptimizedPoolConfig {
    pub max_connections_per_endpoint: usize,
    pub min_connections_per_endpoint: usize,
    pub connection_timeout_ms: u64,
    pub idle_timeout_seconds: u64,
    pub health_check_interval_seconds: u64,
    pub enable_connection_reuse: bool,
    pub enable_load_balancing: bool,
    pub enable_circuit_breaker: bool,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_seconds: u64,
    pub enable_metrics: bool,
}

impl Default for OptimizedPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_endpoint: 10,
            min_connections_per_endpoint: 2,
            connection_timeout_ms: 5000,
            idle_timeout_seconds: 300, // 5 minutes
            health_check_interval_seconds: 30, // 30 seconds
            enable_connection_reuse: true,
            enable_load_balancing: true,
            enable_circuit_breaker: true,
            circuit_breaker_threshold: 5, // 5 failures
            circuit_breaker_timeout_seconds: 60, // 1 minute
            enable_metrics: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    pub total_connections: usize,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub failed_connections: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
    pub circuit_breaker_trips: u64,
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            idle_connections: 0,
            failed_connections: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_response_time_ms: 0.0,
            circuit_breaker_trips: 0,
        }
    }
}

#[derive(Debug)]
struct PooledConnection {
    client: RpcClientWrapper,
    endpoint: RpcEndpoint,
    created_at: Instant,
    last_used: Instant,
    is_active: bool,
    request_count: u64,
    failure_count: u64,
}

#[derive(Debug)]
struct CircuitBreaker {
    failure_count: u32,
    last_failure_time: Option<Instant>,
    state: CircuitBreakerState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

pub struct OptimizedConnectionPool {
    // Connection pools per endpoint
    connection_pools: DashMap<String, Arc<RwLock<Vec<PooledConnection>>>>,
    
    // Available connections queue
    available_connections: Arc<SegQueue<(String, PooledConnection)>>,
    
    // Semaphores for connection limits
    semaphores: DashMap<String, Arc<Semaphore>>,
    
    // Circuit breakers per endpoint
    circuit_breakers: DashMap<String, Arc<RwLock<CircuitBreaker>>>,
    
    // Configuration
    config: OptimizedPoolConfig,
    
    // Metrics
    metrics: Arc<RwLock<ConnectionMetrics>>,
    
    // Background tasks
    health_check_task: Option<tokio::task::JoinHandle<()>>,
    cleanup_task: Option<tokio::task::JoinHandle<()>>,
    
    // Request queue for load balancing
    request_queue: Option<(Sender<String>, Receiver<String>)>,
    
    // Endpoint health status
    endpoint_health: DashMap<String, bool>,
}

impl OptimizedConnectionPool {
    pub fn new(endpoints: Vec<RpcEndpoint>, pool_size: usize) -> Result<Self> {
        let config = OptimizedPoolConfig {
            max_connections_per_endpoint: pool_size,
            ..Default::default()
        };
        
        Self::with_config(endpoints, config)
    }
    
    pub fn with_config(endpoints: Vec<RpcEndpoint>, config: OptimizedPoolConfig) -> Result<Self> {
        let connection_pools = DashMap::new();
        let semaphores = DashMap::new();
        let circuit_breakers = DashMap::new();
        let endpoint_health = DashMap::new();
        
        // Initialize pools for each endpoint
        for endpoint in endpoints {
            let url = endpoint.url.clone();
            
            // Create connection pool
            connection_pools.insert(url.clone(), Arc::new(RwLock::new(Vec::new())));
            
            // Create semaphore for connection limits
            semaphores.insert(
                url.clone(),
                Arc::new(Semaphore::new(config.max_connections_per_endpoint))
            );
            
            // Create circuit breaker
            circuit_breakers.insert(
                url.clone(),
                Arc::new(RwLock::new(CircuitBreaker {
                    failure_count: 0,
                    last_failure_time: None,
                    state: CircuitBreakerState::Closed,
                }))
            );
            
            // Initialize endpoint as healthy
            endpoint_health.insert(url.clone(), true);
            
            // Pre-create minimum connections
            if config.min_connections_per_endpoint > 0 {
                let pool = connection_pools.get(&url).unwrap();
                let mut pool_guard = pool.write();
                
                for _ in 0..config.min_connections_per_endpoint {
                    if let Ok(client) = RpcClientWrapper::new(&url, config.connection_timeout_ms) {
                        let connection = PooledConnection {
                            client,
                            endpoint: endpoint.clone(),
                            created_at: Instant::now(),
                            last_used: Instant::now(),
                            is_active: false,
                            request_count: 0,
                            failure_count: 0,
                        };
                        pool_guard.push(connection);
                    }
                }
            }
        }
        
        let request_queue = if config.enable_load_balancing {
            Some(bounded(1000))
        } else {
            None
        };
        
        Ok(Self {
            connection_pools,
            available_connections: Arc::new(SegQueue::new()),
            semaphores,
            circuit_breakers,
            config,
            metrics: Arc::new(RwLock::new(ConnectionMetrics::default())),
            health_check_task: None,
            cleanup_task: None,
            request_queue,
            endpoint_health,
        })
    }
    
    pub async fn start_background_tasks(&mut self) -> Result<()> {
        // Start health check task
        let health_check_pools = self.connection_pools.clone();
        let health_check_config = self.config.clone();
        let health_check_metrics = self.metrics.clone();
        let circuit_breakers = self.circuit_breakers.clone();
        let endpoint_health = self.endpoint_health.clone();
        
        let health_check_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(health_check_config.health_check_interval_seconds)
            );
            
            loop {
                interval.tick().await;
                
                for pool_entry in health_check_pools.iter() {
                    let url = pool_entry.key().clone();
                    let pool = pool_entry.value().clone();
                    
                    // Check circuit breaker
                    let circuit_breaker = circuit_breakers.get(&url).unwrap().clone();
                    let mut cb_guard = circuit_breaker.write();
                    
                    match cb_guard.state {
                        CircuitBreakerState::Open => {
                            // Check if we should transition to half-open
                            if let Some(last_failure) = cb_guard.last_failure_time {
                                if last_failure.elapsed() > Duration::from_secs(health_check_config.circuit_breaker_timeout_seconds) {
                                    cb_guard.state = CircuitBreakerState::HalfOpen;
                                    info!("Circuit breaker for {} transitioning to half-open", url);
                                }
                            }
                        }
                        CircuitBreakerState::HalfOpen => {
                            // Try a single request to see if the endpoint is healthy
                            if let Ok(client) = RpcClientWrapper::new(&url, health_check_config.connection_timeout_ms) {
                                match client.get_health().await {
                                    Ok(_) => {
                                        cb_guard.state = CircuitBreakerState::Closed;
                                        cb_guard.failure_count = 0;
                                        endpoint_health.insert(url.clone(), true);
                                        info!("Circuit breaker for {} closed - endpoint is healthy", url);
                                    }
                                    Err(_) => {
                                        cb_guard.failure_count += 1;
                                        if cb_guard.failure_count >= health_check_config.circuit_breaker_threshold {
                                            cb_guard.state = CircuitBreakerState::Open;
                                            cb_guard.last_failure_time = Some(Instant::now());
                                            endpoint_health.insert(url.clone(), false);
                                            info!("Circuit breaker for {} opened - endpoint is unhealthy", url);
                                        }
                                    }
                                }
                            }
                        }
                        CircuitBreakerState::Closed => {
                            // Perform health check
                            let pool_guard = pool.read();
                            if let Some(connection) = pool_guard.first() {
                                match connection.client.get_health().await {
                                    Ok(_) => {
                                        endpoint_health.insert(url.clone(), true);
                                    }
                                    Err(_) => {
                                        cb_guard.failure_count += 1;
                                        if cb_guard.failure_count >= health_check_config.circuit_breaker_threshold {
                                            cb_guard.state = CircuitBreakerState::Open;
                                            cb_guard.last_failure_time = Some(Instant::now());
                                            endpoint_health.insert(url.clone(), false);
                                            info!("Circuit breaker for {} opened due to health check failure", url);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        
        // Start cleanup task
        let cleanup_pools = self.connection_pools.clone();
        let cleanup_config = self.config.clone();
        let cleanup_metrics = self.metrics.clone();
        let available_connections = self.available_connections.clone();
        
        let cleanup_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Run every minute
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let idle_timeout = Duration::from_secs(cleanup_config.idle_timeout_seconds);
                
                // Clean up idle connections
                for pool_entry in cleanup_pools.iter() {
                    let pool = pool_entry.value().clone();
                    let mut pool_guard = pool.write();
                    
                    let initial_size = pool_guard.len();
                    pool_guard.retain(|conn| {
                        let is_idle = now.duration_since(conn.last_used) > idle_timeout;
                        let is_min_exceeded = pool_guard.len() > cleanup_config.min_connections_per_endpoint;
                        
                        !is_idle || !is_min_exceeded
                    });
                    
                    let cleaned_up = initial_size - pool_guard.len();
                    if cleaned_up > 0 {
                        debug!("Cleaned up {} idle connections", cleaned_up);
                    }
                }
                
                // Clean up available connections queue
                let mut temp_connections = Vec::new();
                while let Some((url, conn)) = available_connections.pop() {
                    if now.duration_since(conn.last_used) <= idle_timeout {
                        temp_connections.push((url, conn));
                    }
                }
                
                // Push back valid connections
                for (url, conn) in temp_connections {
                    available_connections.push((url, conn));
                }
            }
        });
        
        self.health_check_task = Some(health_check_task);
        self.cleanup_task = Some(cleanup_task);
        
        Ok(())
    }
    
    pub async fn get_connection(&self) -> Result<(String, RpcClientWrapper)> {
        // Find the best endpoint based on health and load
        let best_endpoint = self.select_best_endpoint()?;
        
        // Check circuit breaker
        let circuit_breaker = self.circuit_breakers.get(&best_endpoint).unwrap().clone();
        let cb_guard = circuit_breaker.read();
        
        if cb_guard.state == CircuitBreakerState::Open {
            return Err(SolanaRecoverError::NetworkError(
                format!("Circuit breaker is open for endpoint: {}", best_endpoint)
            ));
        }
        
        drop(cb_guard);
        
        // Get semaphore permit
        let semaphore = self.semaphores.get(&best_endpoint).unwrap().clone();
        let _permit = semaphore.acquire().await.map_err(|_| {
            SolanaRecoverError::NetworkError(
                format!("Failed to acquire connection permit for endpoint: {}", best_endpoint)
            )
        })?;
        
        // Try to get an existing connection
        if let Some((url, mut connection)) = self.get_available_connection(&best_endpoint) {
            connection.last_used = Instant::now();
            connection.is_active = true;
            
            self.update_metrics(|m| m.active_connections += 1);
            
            return Ok((url, connection.client));
        }
        
        // Create new connection
        let client = RpcClientWrapper::new(&best_endpoint, self.config.connection_timeout_ms)?;
        
        let connection = PooledConnection {
            client: client.clone(),
            endpoint: RpcEndpoint {
                url: best_endpoint.clone(),
                priority: 0,
                rate_limit_rps: 100,
                timeout_ms: self.config.connection_timeout_ms,
                healthy: true,
            },
            created_at: Instant::now(),
            last_used: Instant::now(),
            is_active: true,
            request_count: 0,
            failure_count: 0,
        };
        
        self.update_metrics(|m| {
            m.total_connections += 1;
            m.active_connections += 1;
        });
        
        Ok((best_endpoint, client))
    }
    
    pub async fn return_connection(&self, url: String, client: RpcClientWrapper, success: bool) {
        // Update metrics
        self.update_metrics(|m| {
            m.active_connections -= 1;
            if success {
                m.successful_requests += 1;
            } else {
                m.failed_requests += 1;
            }
        });
        
        if !self.config.enable_connection_reuse {
            return;
        }
        
        // Update circuit breaker if request failed
        if !success {
            if let Some(circuit_breaker) = self.circuit_breakers.get(&url) {
                let mut cb_guard = circuit_breaker.write();
                cb_guard.failure_count += 1;
                
                if cb_guard.failure_count >= self.config.circuit_breaker_threshold {
                    cb_guard.state = CircuitBreakerState::Open;
                    cb_guard.last_failure_time = Some(Instant::now());
                    self.endpoint_health.insert(url.clone(), false);
                    
                    self.update_metrics(|m| m.circuit_breaker_trips += 1);
                }
            }
        }
        
        // Create pooled connection
        let connection = PooledConnection {
            client,
            endpoint: RpcEndpoint {
                url: url.clone(),
                priority: 0,
                rate_limit_rps: 100,
                timeout_ms: self.config.connection_timeout_ms,
                healthy: true,
            },
            created_at: Instant::now(),
            last_used: Instant::now(),
            is_active: false,
            request_count: 1,
            failure_count: if success { 0 } else { 1 },
        };
        
        // Return to available connections
        self.available_connections.push((url, connection));
        
        self.update_metrics(|m| m.idle_connections += 1);
    }
    
    fn select_best_endpoint(&self) -> Result<String> {
        let mut healthy_endpoints: SmallVec<[String; 8]> = self.endpoint_health
            .iter()
            .filter(|entry| *entry.value())
            .map(|entry| entry.key().clone())
            .collect();
        
        if healthy_endpoints.is_empty() {
            return Err(SolanaRecoverError::NetworkError(
                "No healthy endpoints available".to_string()
            ));
        }
        
        // Simple round-robin selection (could be enhanced with load-based selection)
        let index = fastrand::usize(0..healthy_endpoints.len());
        Ok(healthy_endpoints.swap_remove(index))
    }
    
    fn get_available_connection(&self, endpoint: &str) -> Option<(String, PooledConnection)> {
        // Try to find a connection for the specific endpoint
        let mut temp_connections = Vec::new();
        let mut found_connection = None;
        
        while let Some((url, connection)) = self.available_connections.pop() {
            if url == endpoint {
                found_connection = Some((url, connection));
                break;
            } else {
                temp_connections.push((url, connection));
            }
        }
        
        // Push back the temporary connections
        for (url, conn) in temp_connections {
            self.available_connections.push((url, conn));
        }
        
        found_connection
    }
    
    fn update_metrics<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut ConnectionMetrics),
    {
        let mut metrics = self.metrics.write();
        update_fn(&mut metrics);
    }
    
    pub fn get_metrics(&self) -> ConnectionMetrics {
        self.metrics.read().clone()
    }
    
    pub fn get_endpoint_health(&self) -> Vec<(String, bool)> {
        self.endpoint_health
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect()
    }
    
    pub fn get_circuit_breaker_status(&self) -> Vec<(String, String)> {
        self.circuit_breakers
            .iter()
            .map(|entry| {
                let url = entry.key().clone();
                let cb_guard = entry.value().read();
                let state = match cb_guard.state {
                    CircuitBreakerState::Closed => "closed".to_string(),
                    CircuitBreakerState::Open => "open".to_string(),
                    CircuitBreakerState::HalfOpen => "half-open".to_string(),
                };
                (url, state)
            })
            .collect()
    }
}

impl Drop for OptimizedConnectionPool {
    fn drop(&mut self) {
        if let Some(task) = self.health_check_task.take() {
            task.abort();
        }
        if let Some(task) = self.cleanup_task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_pool_creation() {
        let endpoints = vec![
            RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 1,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            }
        ];
        
        let pool = OptimizedConnectionPool::new(endpoints, 5).unwrap();
        let metrics = pool.get_metrics();
        
        assert_eq!(metrics.total_connections, 0); // No connections created yet
    }
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let endpoints = vec![
            RpcEndpoint {
                url: "https://invalid-endpoint.com".to_string(),
                priority: 1,
                rate_limit_rps: 100,
                timeout_ms: 1000,
                healthy: true,
            }
        ];
        
        let config = OptimizedPoolConfig {
            circuit_breaker_threshold: 2,
            circuit_breaker_timeout_seconds: 1,
            ..Default::default()
        };
        
        let mut pool = OptimizedConnectionPool::with_config(endpoints, config).unwrap();
        pool.start_background_tasks().await.unwrap();
        
        // Simulate failed requests to trigger circuit breaker
        for _ in 0..3 {
            let result = pool.get_connection().await;
            assert!(result.is_err());
        }
        
        // Check circuit breaker status
        let status = pool.get_circuit_breaker_status();
        assert!(!status.is_empty());
        
        // The circuit breaker should be open after failures
        let (_, state) = &status[0];
        assert_eq!(state, "open");
    }
    
    #[tokio::test]
    async fn test_endpoint_health() {
        let endpoints = vec![
            RpcEndpoint {
                url: "https://api.mainnet-beta.solana.com".to_string(),
                priority: 1,
                rate_limit_rps: 100,
                timeout_ms: 5000,
                healthy: true,
            },
            RpcEndpoint {
                url: "https://invalid-endpoint.com".to_string(),
                priority: 2,
                rate_limit_rps: 100,
                timeout_ms: 1000,
                healthy: true,
            }
        ];
        
        let pool = OptimizedConnectionPool::new(endpoints, 5).unwrap();
        let health = pool.get_endpoint_health();
        
        assert_eq!(health.len(), 2);
        // All endpoints should start as healthy
        assert!(health[0].1);
        assert!(health[1].1);
    }
}
