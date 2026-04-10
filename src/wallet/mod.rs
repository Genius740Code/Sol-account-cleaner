pub mod manager;
pub mod private_key;
pub mod phantom;
pub mod solflare;
pub mod turnkey;
pub mod tests;
pub mod transaction_validator;
pub mod nonce_manager;
pub mod audit_logger;

pub use manager::{WalletManager, WalletManagerConfig, WalletCredentials, WalletInfo, WalletConnection, WalletProvider, ConnectionData};
pub use private_key::PrivateKeyProvider;
pub use phantom::PhantomProvider;
pub use solflare::{SolflareProvider, SolflareConfig};
pub use turnkey::TurnkeyProvider;
pub use transaction_validator::{TransactionValidator, ValidationResult, SimulationResult};
pub use nonce_manager::{NonceManager, NonceInfo, ReplayProtectionConfig, NonceMetrics};
pub use audit_logger::{AuditLogger, AuditEvent, SecurityMetrics, RiskLevel, SecurityContext};


#[cfg(test)]
mod tests;
