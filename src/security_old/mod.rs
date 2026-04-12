//! Security module for enhanced protection features
//! 
//! This module provides comprehensive security enhancements including:
//! - Hardware Security Module (HSM) support
//! - Memory encryption for sensitive data
//! - Advanced rate limiting with token bucket algorithm
//! - Audit logging and security monitoring

pub mod hsm_provider;
pub mod memory_encryption;
pub mod rate_limiter;

// Re-export commonly used security types
pub use hsm_provider::{HsmManager, HsmProvider, HsmProviderInfo, SecurityLevel, HsmProviderType};
pub use memory_encryption::{MemoryEncryptionManager, SecureBuffer, SecurityConfig};
pub use rate_limiter::{TokenBucketRateLimiter, RateLimitConfig, RateLimitResult, BucketStatus};
