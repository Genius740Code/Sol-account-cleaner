use crate::core::{RpcEndpoint, Result, SolanaRecoverError};
use crate::rpc::{ConnectionPoolTrait, RpcClientWrapper};
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[cfg(test)]
mod enhanced_pool_tests;

/// Enhanced connection pool with multiple endpoints and health checks
pub struct EnhancedConnectionPool {
    endpoints: Arc<RwLock<Vec<WeightedEndpoint>>>,
    connection_pools: Arc<DashMap<String, Arc<BasicConnectionPool>>>,
    health_checker: Arc<HealthChecker>,
    #[allow(dead_code)]
    load_balancer: Arc<LoadBalancer>,
    circuit_breakers: Arc<DashMap<String, Arc<CircuitBreaker>>>,
    metrics: Arc<RwLock<EnhancedPoolMetrics>>,
    config: PoolConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeightedEndpoint {
    pub endpoint: RpcEndpoint,
    pub weight: f64,  // Based on response time and success rate
    pub priority: u8,
    pub region: String,
    pub response_time_ms: f64,
    pub success_rate: f64,
    pub last_health_check_ms: Option<u64>, // Unix timestamp in ms
    pub consecutive_failures: u32,
}

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_connections_per_endpoint: usize,
    pub health_check_interval: Duration,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout: Duration,
    pub load_balance_strategy: LoadBalanceStrategy,
    pub enable_connection_multiplexing: bool,
    pub enable_compression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    ResponseTime,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct EnhancedPoolMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub active_connections: u64,
    pub avg_response_time_ms: f64,
    pub endpoint_metrics: HashMap<String, EndpointMetrics>,
    pub circuit_breaker_activations: u64,
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointMetrics {
    pub requests: u64,
    pub successes: u64,
    pub failures: u64,
    pub avg_response_time_ms: f64,
    pub last_success_ms: Option<u64>, // Unix timestamp in ms
    pub last_failure_ms: Option<u64>, // Unix timestamp in ms
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_endpoint: 50,
            health_check_interval: Duration::from_secs(30),
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(60),
            load_balance_strategy: LoadBalanceStrategy::WeightedRoundRobin,
            enable_connection_multiplexing: true,
            enable_compression: true,
        }
    }
}

impl EnhancedConnectionPool {
    pub fn new(endpoints: Vec<RpcEndpoint>, config: PoolConfig) -> Self {
        let weighted_endpoints: Vec<WeightedEndpoint> = endpoints
            .into_iter()
            .enumerate()
            .map(|(i, endpoint)| WeightedEndpoint {
                endpoint: endpoint.clone(),
                weight: 1.0 / (i as f64 + 1.0), // Initial weight based on priority
                priority: endpoint.priority,
                region: Self::extract_region(&endpoint.url),
                response_time_ms: 100.0,
                success_rate: 1.0,
                last_health_check_ms: None,
                consecutive_failures: 0,
            })
            .collect();

        let pool = Self {
            endpoints: Arc::new(RwLock::new(weighted_endpoints)),
            connection_pools: Arc::new(DashMap::new()),
            health_checker: Arc::new(HealthChecker::new(config.health_check_interval)),
            load_balancer: Arc::new(LoadBalancer::new(config.load_balance_strategy.clone())),
            circuit_breakers: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(EnhancedPoolMetrics::default())),
            config,
        };

        // Initialize connection pools and circuit breakers for each endpoint
        pool.initialize_components();

        pool
    }

    fn extract_region(url: &str) -> String {
        // Simple region extraction from URL - in production, use more sophisticated logic
        if url.contains("us-east") {
            "us-east".to_string()
        } else if url.contains("us-west") {
            "us-west".to_string()
        } else if url.contains("eu") {
            "eu-west".to_string()
        } else {
            "global".to_string()
        }
    }

    fn initialize_components(&self) {
        let endpoints = self.endpoints.blocking_read();
        for endpoint in endpoints.iter() {
            // Create connection pool for this endpoint
            let pool = Arc::new(BasicConnectionPool::new(
                endpoint.endpoint.clone(),
                self.config.max_connections_per_endpoint,
            ));
            self.connection_pools.insert(endpoint.endpoint.url.clone(), pool);

            // Create circuit breaker for this endpoint
            let circuit_breaker = Arc::new(CircuitBreaker::new(
                endpoint.endpoint.url.clone(),
                self.config.circuit_breaker_threshold,
                self.config.circuit_breaker_timeout,
            ));
            self.circuit_breakers.insert(endpoint.endpoint.url.clone(), circuit_breaker);
        }
    }

    pub async fn start_health_checks(self: Arc<Self>) {
        self.health_checker.start(self.clone()).await;
    }

    async fn select_endpoint(&self) -> Result<String> {
        let endpoints = self.endpoints.read().await;
        self.load_balancer.select_endpoint(&*endpoints).await
    }

    async fn get_client_for_endpoint(&self, endpoint_url: &str) -> Result<Arc<RpcClientWrapper>> {
        let circuit_breaker = self.circuit_breakers.get(endpoint_url)
            .ok_or_else(|| SolanaRecoverError::ConfigError("No circuit breaker for endpoint".to_string()))?;

        if !circuit_breaker.allow_request().await {
            return Err(SolanaRecoverError::NetworkError("Circuit breaker is open".to_string()));
        }

        let pool = self.connection_pools.get(endpoint_url)
            .ok_or_else(|| SolanaRecoverError::ConfigError("No connection pool for endpoint".to_string()))?;

        let client = pool.get_client().await?;
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
            
            let endpoint_metrics = metrics.endpoint_metrics
                .entry(endpoint_url.to_string())
                .or_insert_with(|| EndpointMetrics {
                    requests: 0,
                    successes: 0,
                    failures: 0,
                    avg_response_time_ms: 0.0,
                    last_success_ms: None,
                    last_failure_ms: None,
                });
            endpoint_metrics.requests += 1;
        }

        Ok(client)
    }

    pub async fn update_endpoint_metrics(&self, endpoint_url: &str, success: bool, response_time_ms: f64) {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.iter_mut().find(|e| e.endpoint.url == endpoint_url) {
            if success {
                endpoint.success_rate = (endpoint.success_rate * 0.9) + (1.0 * 0.1); // EMA
                endpoint.response_time_ms = (endpoint.response_time_ms * 0.9) + (response_time_ms * 0.1);
                endpoint.consecutive_failures = 0;
                endpoint.weight = 1.0 / (1.0 + endpoint.response_time_ms / 1000.0) * endpoint.success_rate;
                endpoint.last_health_check_ms = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64
                );
            } else {
                endpoint.consecutive_failures += 1;
                endpoint.success_rate = endpoint.success_rate * 0.9;
                endpoint.weight *= 0.8;
                endpoint.last_health_check_ms = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64
                );
            }
        }

        // Update global metrics
        {
            let mut metrics = self.metrics.write().await;
            if success {
                metrics.successful_requests += 1;
                if let Some(endpoint_metrics) = metrics.endpoint_metrics.get_mut(endpoint_url) {
                    endpoint_metrics.successes += 1;
                    endpoint_metrics.last_success_ms = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64
                    );
                    endpoint_metrics.avg_response_time_ms = 
                        (endpoint_metrics.avg_response_time_ms * (endpoint_metrics.successes - 1) as f64 + response_time_ms)
                        / endpoint_metrics.successes as f64;
                }
            } else {
                metrics.failed_requests += 1;
                if let Some(endpoint_metrics) = metrics.endpoint_metrics.get_mut(endpoint_url) {
                    endpoint_metrics.failures += 1;
                    endpoint_metrics.last_failure_ms = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64
                    );
                }
            }
        }
    }

    pub async fn get_metrics(&self) -> EnhancedPoolMetrics {
        let metrics = self.metrics.read().await;
        EnhancedPoolMetrics {
            total_requests: metrics.total_requests,
            successful_requests: metrics.successful_requests,
            failed_requests: metrics.failed_requests,
            active_connections: metrics.active_connections,
            avg_response_time_ms: metrics.avg_response_time_ms,
            endpoint_metrics: metrics.endpoint_metrics.clone(),
            circuit_breaker_activations: metrics.circuit_breaker_activations,
            last_health_check: metrics.last_health_check,
        }
    }
}

#[async_trait]
impl ConnectionPoolTrait for EnhancedConnectionPool {
    async fn get_client(&self) -> Result<Arc<RpcClientWrapper>> {
        let endpoint_url = self.select_endpoint().await?;
        let client = self.get_client_for_endpoint(&endpoint_url).await?;
        
        // Wrap client to automatically update metrics
        // For now, return the base client without metrics wrapping
        // TODO: Implement proper metrics-aware client wrapper
        Ok(client)
    }
}


/// Health checker for monitoring endpoint health
pub struct HealthChecker {
    check_interval: Duration,
}

impl HealthChecker {
    pub fn new(check_interval: Duration) -> Self {
        Self { check_interval }
    }

    pub async fn start(&self, pool: Arc<EnhancedConnectionPool>) {
        let pool_clone = pool.clone();
        let interval = self.check_interval;
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                Self::perform_health_check(&pool_clone).await;
            }
        });
    }

    async fn perform_health_check(pool: &EnhancedConnectionPool) {
        let endpoints = pool.endpoints.read().await;
        let endpoint_urls: Vec<String> = endpoints.iter().map(|e| e.endpoint.url.clone()).collect();
        drop(endpoints);
        
        for endpoint_url in endpoint_urls {
            let is_healthy = Self::check_endpoint_health(&endpoint_url).await;
            
            // Update endpoint health status
            {
                let mut endpoints_guard = pool.endpoints.write().await;
                if let Some(ep) = endpoints_guard.iter_mut().find(|e| e.endpoint.url == endpoint_url) {
                    if is_healthy {
                        ep.consecutive_failures = 0;
                        ep.endpoint.healthy = true;
                    } else {
                        ep.consecutive_failures += 1;
                        if ep.consecutive_failures >= 3 {
                            ep.endpoint.healthy = false;
                        }
                    }
                }
            }
            
            // Update circuit breaker
            if let Some(circuit_breaker) = pool.circuit_breakers.get(&endpoint_url) {
                if is_healthy {
                    circuit_breaker.record_success().await;
                } else {
                    circuit_breaker.record_failure().await;
                }
            }
        }
        
        // Update metrics
        {
            let mut metrics = pool.metrics.write().await;
            metrics.last_health_check = Some(chrono::Utc::now());
        }
    }

    async fn check_endpoint_health(url: &str) -> bool {
        let client = RpcClient::new_with_timeout(
            url.to_string(),
            Duration::from_millis(5000),
        );
        
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::task::spawn_blocking(move || {
                client.get_latest_blockhash().is_ok()
            }).await.unwrap_or(false)
        }).await.unwrap_or(false)
    }
}

/// Load balancer for endpoint selection
pub struct LoadBalancer {
    #[allow(dead_code)]
    strategy: LoadBalanceStrategy,
    #[allow(dead_code)]
    round_robin_counter: tokio::sync::Mutex<usize>,
}

impl LoadBalancer {
    pub fn new(strategy: LoadBalanceStrategy) -> Self {
        Self {
            strategy,
            round_robin_counter: tokio::sync::Mutex::new(0),
        }
    }

    #[allow(dead_code)]
    async fn select_endpoint(&self, endpoints: &[WeightedEndpoint]) -> Result<String> {
        let healthy_endpoints: Vec<&WeightedEndpoint> = endpoints
            .iter()
            .filter(|e| e.endpoint.healthy)
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(SolanaRecoverError::ConfigError("No healthy endpoints available".to_string()));
        }

        match self.strategy {
            LoadBalanceStrategy::RoundRobin => {
                let mut counter = self.round_robin_counter.lock().await;
                let index = *counter % healthy_endpoints.len();
                *counter += 1;
                Ok(healthy_endpoints[index].endpoint.url.clone())
            }
            LoadBalanceStrategy::WeightedRoundRobin => {
                let total_weight: f64 = healthy_endpoints.iter().map(|e| e.weight).sum();
                let mut random_weight = rand::random::<f64>() * total_weight;
                
                for endpoint in &healthy_endpoints {
                    random_weight -= endpoint.weight;
                    if random_weight <= 0.0 {
                        return Ok(endpoint.endpoint.url.clone());
                    }
                }
                
                // Fallback to first endpoint
                Ok(healthy_endpoints[0].endpoint.url.clone())
            }
            LoadBalanceStrategy::LeastConnections => {
                // For simplicity, use endpoint with lowest response time
                let endpoint = healthy_endpoints
                    .iter()
                    .min_by(|a, b| a.response_time_ms.partial_cmp(&b.response_time_ms).unwrap())
                    .unwrap();
                Ok(endpoint.endpoint.url.clone())
            }
            LoadBalanceStrategy::ResponseTime => {
                let endpoint = healthy_endpoints
                    .iter()
                    .min_by(|a, b| a.response_time_ms.partial_cmp(&b.response_time_ms).unwrap())
                    .unwrap();
                Ok(endpoint.endpoint.url.clone())
            }
        }
    }
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    #[allow(dead_code)]
    endpoint_url: String,
    #[allow(dead_code)]
    failure_threshold: u32,
    timeout: Duration,
    state: tokio::sync::Mutex<CircuitBreakerState>,
    last_state_change: tokio::sync::Mutex<Instant>,
}

#[derive(Debug, Clone)]
enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(endpoint_url: String, failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            endpoint_url,
            failure_threshold,
            timeout,
            state: tokio::sync::Mutex::new(CircuitBreakerState::Closed),
            last_state_change: tokio::sync::Mutex::new(Instant::now()),
        }
    }

    pub async fn allow_request(&self) -> bool {
        let state = self.state.lock().await;
        let last_change = self.last_state_change.lock().await;
        
        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if last_change.elapsed() > self.timeout {
                    drop(state);
                    drop(last_change);
                    self.transition_to_half_open().await;
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    pub async fn record_success(&self) {
        let state = self.state.lock().await;
        match *state {
            CircuitBreakerState::HalfOpen => {
                drop(state);
                self.transition_to_closed().await;
            }
            _ => {}
        }
    }

    pub async fn record_failure(&self) {
        let state = self.state.lock().await;
        match *state {
            CircuitBreakerState::Closed => {
                // In a real implementation, we'd track failure count
                drop(state);
                self.transition_to_open().await;
            }
            CircuitBreakerState::HalfOpen => {
                drop(state);
                self.transition_to_open().await;
            }
            _ => {}
        }
    }

    async fn transition_to_closed(&self) {
        *self.state.lock().await = CircuitBreakerState::Closed;
        *self.last_state_change.lock().await = Instant::now();
    }

    async fn transition_to_open(&self) {
        *self.state.lock().await = CircuitBreakerState::Open;
        *self.last_state_change.lock().await = Instant::now();
    }

    async fn transition_to_half_open(&self) {
        *self.state.lock().await = CircuitBreakerState::HalfOpen;
        *self.last_state_change.lock().await = Instant::now();
    }
}

/// Basic connection pool for individual endpoints
pub struct BasicConnectionPool {
    endpoint: RpcEndpoint,
    clients: tokio::sync::Mutex<Vec<Arc<RpcClientWrapper>>>,
    max_connections: usize,
}

impl BasicConnectionPool {
    pub fn new(endpoint: RpcEndpoint, max_connections: usize) -> Self {
        Self {
            endpoint,
            clients: tokio::sync::Mutex::new(Vec::with_capacity(max_connections)),
            max_connections,
        }
    }

    pub async fn get_client(&self) -> Result<Arc<RpcClientWrapper>> {
        let mut clients = self.clients.lock().await;
        
        if let Some(client) = clients.pop() {
            Ok(client)
        } else {
            drop(clients);
            self.create_client().await
        }
    }

    async fn create_client(&self) -> Result<Arc<RpcClientWrapper>> {
        let client = Arc::new(RpcClientWrapper::from_url(
            &self.endpoint.url,
            self.endpoint.timeout_ms,
        )?);
        Ok(client)
    }

    pub async fn return_client(&self, _client: Arc<RpcClientWrapper>) {
        let clients = self.clients.lock().await;
        if clients.len() < self.max_connections {
            // In a real implementation, we'd track client health
            // clients.push(client);
        }
    }
}
