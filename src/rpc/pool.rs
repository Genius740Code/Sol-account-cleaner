use crate::core::{RpcEndpoint, Result, SolanaRecoverError};
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use std::collections::VecDeque;
use std::time::Duration;

pub struct ConnectionPool {
    endpoints: Vec<RpcEndpoint>,
    clients: Arc<Mutex<VecDeque<Arc<RpcClient>>>>,
    semaphore: Arc<Semaphore>,
    max_connections: usize,
    health_check_interval: Duration,
}

impl ConnectionPool {
    pub fn new(endpoints: Vec<RpcEndpoint>, max_connections: usize) -> Self {
        let pool_size = max_connections.min(endpoints.len() * 2);
        
        Self {
            endpoints,
            clients: Arc::new(Mutex::new(VecDeque::with_capacity(pool_size))),
            semaphore: Arc::new(Semaphore::new(pool_size)),
            max_connections: pool_size,
            health_check_interval: Duration::from_secs(30),
        }
    }

    pub async fn get_client(&self) -> Result<Arc<RpcClient>> {
        let _permit = self.semaphore.acquire().await
            .map_err(|_| SolanaRecoverError::ConnectionPoolExhausted)?;

        let mut clients = self.clients.lock().await;
        
        if let Some(client) = clients.pop_front() {
            Ok(client)
        } else {
            drop(clients);
            self.create_new_client().await
        }
    }

    pub async fn return_client(&self, client: Arc<RpcClient>) {
        let mut clients = self.clients.lock().await;
        if clients.len() < self.max_connections {
            clients.push_back(client);
        }
    }

    async fn create_new_client(&self) -> Result<Arc<RpcClient>> {
        let endpoint = self.select_healthy_endpoint()?;
        let client = Arc::new(RpcClient::new_with_timeout(
            endpoint.url.clone(),
            Duration::from_millis(endpoint.timeout_ms),
        ));
        
        Ok(client)
    }

    fn select_healthy_endpoint(&self) -> Result<RpcEndpoint> {
        self.endpoints
            .iter()
            .filter(|e| e.healthy)
            .min_by_key(|e| e.priority)
            .cloned()
            .ok_or_else(|| SolanaRecoverError::ConfigError("No healthy RPC endpoints available".to_string()))
    }

    pub async fn health_check(&self) {
        for endpoint in &self.endpoints {
            let endpoint_clone = endpoint.clone();
            let url = endpoint_clone.url.clone();
            
            tokio::spawn(async move {
                let _is_healthy = Self::check_endpoint_health_static(&url).await;
                // Note: In a real implementation, you'd want to update endpoint health
                // This requires more complex state management
            });
        }
    }

    async fn check_endpoint_health_static(url: &str) -> bool {
        let client = RpcClient::new_with_timeout(
            url.to_string(),
            Duration::from_millis(5000),
        );
        
        match client.get_latest_blockhash() {
            Ok(_) => true,
            Err(_) => false,
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
