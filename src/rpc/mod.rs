use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait ConnectionPoolTrait: Send + Sync {
    async fn get_client(&self) -> crate::core::Result<Arc<RpcClientWrapper>>;
}

pub mod client;
pub mod enhanced_pool;
pub mod mock;
pub mod pool;

#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod tests;

pub use pool::*;
pub use client::*;
pub use enhanced_pool::*;
pub use mock::*;
