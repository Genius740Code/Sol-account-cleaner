use crate::core::{RpcEndpoint, Result, SolanaRecoverError};
use crate::rpc::{ConnectionPoolTrait, RpcClientWrapper};
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore, RwLock};
use std::collections::VecDeque;
use std::time::Duration;
use async_trait::async_trait;

pub struct ConnectionPool {
    endpoints: Arc<RwLock<Vec<RpcEndpoint>>>,
    clients: Arc<Mutex<VecDeque<Arc<RpcClientWrapper>>>>,
    semaphore: Arc<Semaphore>,
    max_connections: usize,
    health_check_interval: Duration,
    connection_timeout: Duration,
    metrics: Arc<RwLock<PoolMetrics>>,
    circuit_breaker: Arc<crate::core::error_handling::CircuitBreaker>,
}

#[derive(Debug, Default, Clone)]
pub struct PoolMetrics {
    pub total_requests: u64,
    pub active_connections: u64,
    pub failed_connections: u64,
    pub avg_response_time_ms: f64,
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
}

impl ConnectionPool {
    pub fn new(endpoints: Vec<RpcEndpoint>, max_connections: usize) -> Self {
        let pool_size = max_connections.min(endpoints.len() * 4); // Increase multiplier for better performance
        let circuit_breaker_config = crate::core::error_handling::CircuitBreakerConfig {
            failure_threshold: 10,
            success_threshold: 5,
            timeout_ms: 30000,
            recovery_timeout_ms: 15000,
        };
        
        Self {
            endpoints: Arc::new(RwLock::new(endpoints)),
            clients: Arc::new(Mutex::new(VecDeque::with_capacity(pool_size))),
            semaphore: Arc::new(Semaphore::new(pool_size)),
            max_connections: pool_size,
            health_check_interval: Duration::from_secs(15), // More frequent health checks
            connection_timeout: Duration::from_secs(10),
            metrics: Arc::new(RwLock::new(PoolMetrics::default())),
            circuit_breaker: Arc::new(
                crate::core::error_handling::CircuitBreaker::new(
                    "rpc_pool".to_string(),
                    circuit_breaker_config
                )
            ),
        }
    }

    pub async fn get_client(&self) -> Result<Arc<RpcClientWrapper>> {
        let _permit = self.semaphore.acquire().await
            .map_err(|_| SolanaRecoverError::ConnectionPoolExhausted)?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
        }

        let mut clients = self.clients.lock().await;
        
        if let Some(client) = clients.pop_front() {
            // Quick health check before returning
            if self.is_client_healthy(&client).await {
                Ok(client)
            } else {
                drop(clients);
                self.create_new_client().await
            }
        } else {
            drop(clients);
            self.create_new_client().await
        }
    }

    pub async fn return_client(&self, client: Arc<RpcClientWrapper>) {
        let mut clients = self.clients.lock().await;
        if clients.len() < self.max_connections {
            clients.push_back(client);
        }
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.active_connections = metrics.active_connections.saturating_sub(1);
        }
    }

    async fn create_new_client(&self) -> Result<Arc<RpcClientWrapper>> {
        let endpoints = self.endpoints.read().await;
        let endpoint = self.select_healthy_endpoint(&*endpoints)?;
        drop(endpoints);
        
        let client = Arc::new(RpcClientWrapper::from_url(
            &endpoint.url,
            endpoint.timeout_ms
        )?);
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.active_connections += 1;
        }
        
        Ok(client)
    }

    fn select_healthy_endpoint(&self, endpoints: &[RpcEndpoint]) -> Result<RpcEndpoint> {
        endpoints
            .iter()
            .filter(|e| e.healthy)
            .min_by_key(|e| (e.priority, e.rate_limit_rps)) // Consider rate limits in selection
            .cloned()
            .ok_or_else(|| SolanaRecoverError::ConfigError("No healthy RPC endpoints available".to_string()))
    }

    pub async fn health_check(&self) {
        let endpoints = self.endpoints.clone();
        let pool = self.clone();
        
        tokio::spawn(async move {
            let mut endpoints_guard = endpoints.write().await;
            let mut updated_endpoints = Vec::new();
            
            for endpoint in endpoints_guard.iter() {
                let endpoint_clone = endpoint.clone();
                let is_healthy = Self::check_endpoint_health_static(&endpoint_clone.url).await;
                
                let mut updated_endpoint = endpoint_clone;
                updated_endpoint.healthy = is_healthy;
                updated_endpoints.push(updated_endpoint);
            }
            
            *endpoints_guard = updated_endpoints;
            
            // Update metrics
            {
                let mut metrics = pool.metrics.write().await;
                metrics.last_health_check = Some(chrono::Utc::now());
            }
        });
    }

    async fn check_endpoint_health_static(url: &str) -> bool {
        let client = RpcClient::new_with_timeout(
            url.to_string(),
            Duration::from_millis(3000), // Faster health checks
        );
        
        let start = std::time::Instant::now();
        match client.get_latest_blockhash() {
            Ok(_) => {
                // Consider response time in health assessment
                start.elapsed() < Duration::from_secs(2)
            }
            Err(_) => false,
        }
    }
    
    async fn is_client_healthy(&self, client: &Arc<RpcClientWrapper>) -> bool {
        let start = std::time::Instant::now();
        match client.get_health().await {
            Ok(_) => {
                let response_time = start.elapsed();
                // Update metrics
                {
                    let mut metrics = self.metrics.write().await;
                    let total_requests = metrics.total_requests;
                    if total_requests > 0 {
                        metrics.avg_response_time_ms = 
                            (metrics.avg_response_time_ms * (total_requests - 1) as f64 + response_time.as_millis() as f64) 
                            / total_requests as f64;
                    }
                }
                response_time < self.connection_timeout
            }
            Err(_) => {
                // Update failed connections metric
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.failed_connections += 1;
                }
                false
            }
        }
    }
    
    pub async fn get_metrics(&self) -> PoolMetrics {
        let metrics = self.metrics.read().await;
        PoolMetrics {
            total_requests: metrics.total_requests,
            active_connections: metrics.active_connections,
            failed_connections: metrics.failed_connections,
            avg_response_time_ms: metrics.avg_response_time_ms,
            last_health_check: metrics.last_health_check,
        }
    }

    pub async fn start_health_checks(self: Arc<Self>) {
        let pool = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(pool.health_check_interval);
            loop {
                interval.tick().await;
                pool.health_check().await;
            }
        });
    }
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            endpoints: self.endpoints.clone(),
            clients: self.clients.clone(),
            semaphore: self.semaphore.clone(),
            max_connections: self.max_connections,
            health_check_interval: self.health_check_interval,
            connection_timeout: self.connection_timeout,
            metrics: self.metrics.clone(),
            circuit_breaker: self.circuit_breaker.clone(),
        }
    }
}

#[async_trait]
impl ConnectionPoolTrait for ConnectionPool {
    async fn get_client(&self) -> Result<RpcClientWrapper> {
        let wrapper = self.get_client().await?;
        Ok(Arc::try_unwrap(wrapper).unwrap_or_else(|_| {
            // If unwrap fails, create a new wrapper
            RpcClientWrapper::from_url("config_endpoint", 5000).unwrap()
        }))
    }
}
