use crate::core::{Result, SolanaRecoverError};
use crate::rpc::ConnectionPoolTrait;
use std::sync::Arc;
use async_trait::async_trait;
use std::time::Duration;

/// Mock connection pool for testing
pub struct MockConnectionPool {
    pub call_count: std::sync::atomic::AtomicU32,
    pub delay_ms: u64,
}

impl MockConnectionPool {
    pub fn new(_endpoints: Vec<String>, _max_connections: usize) -> crate::core::Result<Self> {
        Ok(Self {
            call_count: std::sync::atomic::AtomicU32::new(0),
            delay_ms: 10,
        })
    }
    
    pub async fn get_client_internal(&self) -> crate::core::Result<Arc<crate::rpc::RpcClientWrapper>> {
        self.call_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // Simulate network delay
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        
        // Create a mock RpcClientWrapper (using a mock URL)
        crate::rpc::RpcClientWrapper::new_with_url("https://api.mainnet-beta.solana.com", 30000)
            .map_err(|e| crate::core::SolanaRecoverError::InternalError(format!("Failed to create mock client: {}", e)))
            .map(Arc::new)
    }
}

impl MockConnectionPool {
    pub fn new_simple() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicU32::new(0),
            delay_ms: 10,
        }
    }
    
    pub fn with_delay(delay_ms: u64) -> Self {
        Self {
            call_count: std::sync::atomic::AtomicU32::new(0),
            delay_ms,
        }
    }
    
    pub fn get_call_count(&self) -> u32 {
        self.call_count.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    pub fn reset_call_count(&self) {
        self.call_count.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for MockConnectionPool {
    fn default() -> Self {
        Self::new_simple()
    }
}

#[async_trait]
impl ConnectionPoolTrait for MockConnectionPool {
    async fn get_client(&self) -> Result<Arc<crate::rpc::RpcClientWrapper>> {
        self.call_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // Simulate network delay
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        
        // Create a mock RpcClientWrapper (using a mock URL)
        let mock_client = crate::rpc::RpcClientWrapper::new_with_url("https://api.mainnet-beta.solana.com", 30000)
            .map_err(|e| SolanaRecoverError::InternalError(format!("Failed to create mock client: {}", e)))?;
        Ok(Arc::new(mock_client))
    }
}

