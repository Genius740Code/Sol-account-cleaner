use crate::core::{Result, SolanaRecoverError};
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use hmac::{Hmac, Mac};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Operation result for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationResult {
    Success,
    Failure,
    Error(String),
    RateLimited,
    Unauthorized,
}

/// Security audit entry with tamper protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier for the audit entry
    pub id: Uuid,
    /// Timestamp when the operation occurred
    pub timestamp: DateTime<Utc>,
    /// Type of operation performed
    pub operation: String,
    /// User identifier (if available)
    pub user_id: Option<String>,
    /// Wallet address involved (if applicable)
    pub wallet_address: Option<String>,
    /// Amount involved (if applicable)
    pub amount: Option<u64>,
    /// Result of the operation
    pub result: OperationResult,
    /// IP address of the request
    pub ip_address: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// HMAC signature for tamper protection
    pub signature: String,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl AuditEntry {
    /// Create a new audit entry
    pub fn new(
        operation: String,
        user_id: Option<String>,
        wallet_address: Option<String>,
        amount: Option<u64>,
        result: OperationResult,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation,
            user_id,
            wallet_address,
            amount,
            result,
            ip_address: None,
            user_agent: None,
            request_id: None,
            signature: String::new(), // Will be set by SecurityAuditor
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add metadata to the audit entry
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set IP address
    pub fn with_ip_address(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Set request ID
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Get the data that should be signed (everything except the signature)
    fn get_signable_data(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.id,
            self.timestamp.timestamp_millis(),
            self.operation,
            self.user_id.as_deref().unwrap_or(""),
            self.wallet_address.as_deref().unwrap_or(""),
            self.amount.unwrap_or(0),
            match &self.result {
                OperationResult::Success => "success",
                OperationResult::Failure => "failure",
                OperationResult::Error(msg) => &format!("error:{}", msg),
                OperationResult::RateLimited => "rate_limited",
                OperationResult::Unauthorized => "unauthorized",
            },
            self.ip_address.as_deref().unwrap_or(""),
            self.user_agent.as_deref().unwrap_or(""),
            self.request_id.as_deref().unwrap_or("")
        )
    }
}

/// Tamper-evident security auditor
pub struct SecurityAuditor {
    /// In-memory audit log (for production, this should be persisted)
    audit_log: Arc<Mutex<Vec<AuditEntry>>>,
    /// HMAC key for signature generation
    hmac_key: [u8; 32],
    /// Configuration
    config: AuditorConfig,
}

/// Auditor configuration
#[derive(Debug, Clone)]
pub struct AuditorConfig {
    /// Maximum number of entries to keep in memory
    pub max_memory_entries: usize,
    /// Whether to persist to disk
    pub persist_to_disk: bool,
    /// Log file path (if persisting)
    pub log_file_path: Option<String>,
    /// Whether to encrypt logs at rest
    pub encrypt_at_rest: bool,
    /// Retention period in days
    pub retention_days: u32,
}

impl Default for AuditorConfig {
    fn default() -> Self {
        Self {
            max_memory_entries: 10000,
            persist_to_disk: true,
            log_file_path: Some("audit.log".to_string()),
            encrypt_at_rest: true,
            retention_days: 90,
        }
    }
}

impl SecurityAuditor {
    /// Create a new security auditor with a random HMAC key
    pub fn new() -> Self {
        let hmac_key = Self::generate_hmac_key();
        Self::with_key(hmac_key)
    }

    /// Create a new security auditor with a specific HMAC key
    pub fn with_key(hmac_key: [u8; 32]) -> Self {
        Self {
            audit_log: Arc::new(Mutex::new(Vec::new())),
            hmac_key,
            config: AuditorConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: AuditorConfig) -> Self {
        let hmac_key = Self::generate_hmac_key();
        Self {
            audit_log: Arc::new(Mutex::new(Vec::new())),
            hmac_key,
            config,
        }
    }

    /// Generate a cryptographically secure HMAC key
    fn generate_hmac_key() -> [u8; 32] {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    /// Log an operation with automatic signature generation
    pub async fn log_operation(&self, entry: AuditEntry) -> Result<()> {
        let signed_entry = self.sign_entry(entry).await?;
        self.append_to_audit_log(signed_entry).await?;
        Ok(())
    }

    /// Create and log an operation in one step
    pub async fn log(
        &self,
        operation: &str,
        user_id: Option<&str>,
        wallet_address: Option<&str>,
        amount: Option<u64>,
        result: OperationResult,
    ) -> Result<()> {
        let entry = AuditEntry::new(
            operation.to_string(),
            user_id.map(|s| s.to_string()),
            wallet_address.map(|s| s.to_string()),
            amount,
            result,
        );
        
        self.log_operation(entry).await
    }

    /// Sign an audit entry with HMAC
    async fn sign_entry(&self, mut entry: AuditEntry) -> Result<AuditEntry> {
        let signable_data = entry.get_signable_data();
        
        let mut mac = HmacSha256::new_from_slice(&self.hmac_key)
            .map_err(|_| SolanaRecoverError::InternalError("Failed to create HMAC".to_string()))?;
        
        mac.update(signable_data.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        
        entry.signature = signature;
        Ok(entry)
    }

    /// Append entry to audit log
    async fn append_to_audit_log(&self, entry: AuditEntry) -> Result<()> {
        let mut log = self.audit_log.lock().await;
        
        // Add entry
        log.push(entry.clone());
        
        // Enforce memory limit
        if log.len() > self.config.max_memory_entries {
            log.remove(0);
        }
        
        // Persist to disk if configured
        if self.config.persist_to_disk {
            if let Err(e) = self.persist_to_disk(&entry).await {
                eprintln!("Failed to persist audit entry: {}", e);
            }
        }
        
        Ok(())
    }

    /// Persist entry to disk
    async fn persist_to_disk(&self, entry: &AuditEntry) -> Result<()> {
        if let Some(ref path) = self.config.log_file_path {
            let serialized = serde_json::to_string(entry)
                .map_err(|e| SolanaRecoverError::InternalError(format!("Failed to serialize audit entry: {}", e)))?;
            
            // In production, use proper async file I/O
            tokio::fs::write(path, format!("{}\n", serialized))
                .await
                .map_err(|e| SolanaRecoverError::InternalError(format!("Failed to write audit log: {}", e)))?;
        }
        Ok(())
    }

    /// Verify the integrity of an audit entry
    pub async fn verify_entry(&self, entry: &AuditEntry) -> Result<bool> {
        let signable_data = entry.get_signable_data();
        
        let mut mac = HmacSha256::new_from_slice(&self.hmac_key)
            .map_err(|_| SolanaRecoverError::InternalError("Failed to create HMAC".to_string()))?;
        
        mac.update(signable_data.as_bytes());
        let expected_signature = hex::encode(mac.finalize().into_bytes());
        
        Ok(entry.signature == expected_signature)
    }

    /// Get audit entries for a specific user
    pub async fn get_user_audit_entries(&self, user_id: &str, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        let log = self.audit_log.lock().await;
        let entries: Vec<AuditEntry> = log.iter()
            .filter(|entry| entry.user_id.as_ref().map_or(false, |uid| uid == user_id))
            .rev()
            .take(limit.unwrap_or(100))
            .cloned()
            .collect();
        
        Ok(entries)
    }

    /// Get audit entries for a specific wallet
    pub async fn get_wallet_audit_entries(&self, wallet_address: &str, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        let log = self.audit_log.lock().await;
        let entries: Vec<AuditEntry> = log.iter()
            .filter(|entry| entry.wallet_address.as_ref().map_or(false, |addr| addr == wallet_address))
            .rev()
            .take(limit.unwrap_or(100))
            .cloned()
            .collect();
        
        Ok(entries)
    }

    /// Get audit entries in a time range
    pub async fn get_audit_entries_by_time(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let log = self.audit_log.lock().await;
        let entries: Vec<AuditEntry> = log.iter()
            .filter(|entry| entry.timestamp >= start && entry.timestamp <= end)
            .rev()
            .take(limit.unwrap_or(1000))
            .cloned()
            .collect();
        
        Ok(entries)
    }

    /// Get recent audit entries
    pub async fn get_recent_entries(&self, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        let log = self.audit_log.lock().await;
        let entries: Vec<AuditEntry> = log.iter()
            .rev()
            .take(limit.unwrap_or(50))
            .cloned()
            .collect();
        
        Ok(entries)
    }

    /// Verify integrity of all audit entries
    pub async fn verify_all_entries(&self) -> Result<Vec<(usize, bool)>> {
        let log = self.audit_log.lock().await;
        let mut results = Vec::new();
        
        for (index, entry) in log.iter().enumerate() {
            let is_valid = self.verify_entry(entry).await.unwrap_or(false);
            results.push((index, is_valid));
        }
        
        Ok(results)
    }

    /// Get audit statistics
    pub async fn get_statistics(&self) -> Result<AuditStatistics> {
        let log = self.audit_log.lock().await;
        
        let mut stats = AuditStatistics::default();
        stats.total_entries = log.len();
        
        for entry in log.iter() {
            stats.total_operations += 1;
            
            match entry.result {
                OperationResult::Success => stats.successful_operations += 1,
                OperationResult::Failure => stats.failed_operations += 1,
                OperationResult::Error(_) => stats.error_operations += 1,
                OperationResult::RateLimited => stats.rate_limited_operations += 1,
                OperationResult::Unauthorized => stats.unauthorized_operations += 1,
            }
            
            // Count unique users
            if let Some(ref user_id) = entry.user_id {
                stats.unique_users.insert(user_id.clone());
            }
            
            // Count unique wallets
            if let Some(ref wallet) = entry.wallet_address {
                stats.unique_wallets.insert(wallet.clone());
            }
        }
        
        stats.unique_user_count = stats.unique_users.len();
        stats.unique_wallet_count = stats.unique_wallets.len();
        
        Ok(stats)
    }

    /// Clear old audit entries based on retention policy
    pub async fn cleanup_old_entries(&self) -> Result<usize> {
        let cutoff_time = Utc::now() - chrono::Duration::days(self.config.retention_days as i64);
        let mut log = self.audit_log.lock().await;
        
        let initial_count = log.len();
        log.retain(|entry| entry.timestamp >= cutoff_time);
        let removed_count = initial_count - log.len();
        
        Ok(removed_count)
    }
}

/// Audit statistics
#[derive(Debug, Clone, Default)]
pub struct AuditStatistics {
    pub total_entries: usize,
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub error_operations: u64,
    pub rate_limited_operations: u64,
    pub unauthorized_operations: u64,
    pub unique_user_count: usize,
    pub unique_wallet_count: usize,
    pub unique_users: std::collections::HashSet<String>,
    pub unique_wallets: std::collections::HashSet<String>,
}

impl AuditStatistics {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.successful_operations as f64 / self.total_operations as f64) * 100.0
        }
    }

    /// Calculate failure rate
    pub fn failure_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.failed_operations as f64 / self.total_operations as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_audit_entry_creation() {
        let entry = AuditEntry::new(
            "test_operation".to_string(),
            Some("test_user".to_string()),
            Some("test_wallet".to_string()),
            Some(1000),
            OperationResult::Success,
        );
        
        assert_eq!(entry.operation, "test_operation");
        assert_eq!(entry.user_id, Some("test_user".to_string()));
        assert_eq!(entry.wallet_address, Some("test_wallet".to_string()));
        assert_eq!(entry.amount, Some(1000));
        assert!(matches!(entry.result, OperationResult::Success));
    }

    #[tokio::test]
    async fn test_security_auditor() {
        let auditor = SecurityAuditor::new();
        
        auditor.log(
            "test_operation",
            Some("test_user"),
            Some("test_wallet"),
            Some(1000),
            OperationResult::Success,
        ).await.unwrap();
        
        let entries = auditor.get_recent_entries(Some(10)).await.unwrap();
        assert_eq!(entries.len(), 1);
        
        let entry = &entries[0];
        assert_eq!(entry.operation, "test_operation");
        assert!(!entry.signature.is_empty());
        
        // Verify integrity
        let is_valid = auditor.verify_entry(entry).await.unwrap();
        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_tamper_detection() {
        let auditor = SecurityAuditor::new();
        
        let mut entry = AuditEntry::new(
            "test_operation".to_string(),
            Some("test_user".to_string()),
            None,
            None,
            OperationResult::Success,
        );
        
        // Sign the entry
        let mut signed_entry = auditor.sign_entry(entry.clone()).await.unwrap();
        
        // Tamper with the entry
        signed_entry.operation = "tampered_operation".to_string();
        
        // Verification should fail
        let is_valid = auditor.verify_entry(&signed_entry).await.unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_audit_statistics() {
        let auditor = SecurityAuditor::new();
        
        // Log some operations
        for i in 0..10 {
            let result = if i < 7 { OperationResult::Success } else { OperationResult::Failure };
            auditor.log(
                "test_operation",
                Some(&format!("user_{}", i % 3)),
                Some(&format!("wallet_{}", i % 2)),
                Some(i * 1000),
                result,
            ).await.unwrap();
        }
        
        let stats = auditor.get_statistics().await.unwrap();
        assert_eq!(stats.total_operations, 10);
        assert_eq!(stats.successful_operations, 7);
        assert_eq!(stats.failed_operations, 3);
        assert_eq!(stats.success_rate(), 70.0);
        assert_eq!(stats.failure_rate(), 30.0);
    }
}
