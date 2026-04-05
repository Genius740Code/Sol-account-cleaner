use async_trait::async_trait;

#[async_trait]
pub trait ConnectionPoolTrait: Send + Sync {
    async fn get_client(&self) -> crate::core::Result<RpcClientWrapper>;
}

pub mod pool;
pub mod client;

#[cfg(test)]
mod tests;

pub use pool::*;
pub use client::*;
