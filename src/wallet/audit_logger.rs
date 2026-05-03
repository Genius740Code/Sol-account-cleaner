use crate::core::{Result, SolanaRecoverError};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
    };
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, Duration};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub user_id: Option<String>,
    pub wallet_type: String,
    pub wallet_address: Option<Pubkey>,
    pub transaction_signature: Option<Signature>,
    pub transaction_hash: Option<String>,
    pub nonce: Option<String>,
    pub details: AuditEventDetails,
    pub security_context: SecurityContext,
    pub cryptographic_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    WalletConnected,
    WalletDisconnected,
    TransactionSigningRequested,
    TransactionSigned,
    TransactionSimulationRequested,
    TransactionSimulated,
    SecurityViolation,
    ReplayAttackDetected,
    InvalidTransaction,
    RateLimitExceeded,
    AuthenticationFailure,
    NonceRegistered,
    NonceRevoked,
    ConfigurationChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventDetails {
    pub description: String,
    pub metadata: serde_json::Value,
    pub risk_level: RiskLevel,
    pub compliance_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
    pub correlation_id: String,
    pub request_id: String,
    pub geo_location: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuditLoggerConfig {
    pub enable_file_logging: bool,
    pub enable_database_logging: bool,
    pub log_retention_days: u32,
    pub enable_encryption: bool,
    pub enable_compression: bool,
    pub batch_size: usize,
    pub flush_interval_seconds: u64,
    pub min_risk_level_for_alert: RiskLevel,
}

impl Default for AuditLoggerConfig {
    fn default() -> Self {
        Self {
            enable_file_logging: true,
            enable_database_logging: false,
            log_retention_days: 90,
            enable_encryption: true,
            enable_compression: true,
            batch_size: 100,
            flush_interval_seconds: 30,
            min_risk_level_for_alert: RiskLevel::High,
        }
    }
}

pub struct AuditLogger {
    config: AuditLoggerConfig,
    pending_events: tokio::sync::Mutex<Vec<AuditEvent>>,
    last_flush: tokio::sync::Mutex<SystemTime>,
}

impl AuditLogger {
    pub fn new(config: AuditLoggerConfig) -> Self {
        Self {
            config,
            pending_events: tokio::sync::Mutex::new(Vec::new()),
            last_flush: tokio::sync::Mutex::new(SystemTime::now()),
        }
    }

    pub async fn log_wallet_connection(
        &self,
        user_id: Option<String>,
        wallet_type: String,
        wallet_address: Pubkey,
        security_context: SecurityContext,
    ) -> Result<Uuid> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::WalletConnected,
            user_id,
            wallet_type,
            wallet_address: Some(wallet_address),
            transaction_signature: None,
            transaction_hash: None,
            nonce: None,
            details: AuditEventDetails {
                description: "Wallet connection established".to_string(),
                metadata: serde_json::json!({
                    "wallet_address": wallet_address.to_string(),
                    "connection_time": Utc::now().to_rfc3339(),
                }),
                risk_level: RiskLevel::Low,
                compliance_flags: vec!["WALLET_CONNECTION".to_string()],
            },
            security_context,
            cryptographic_hash: String::new(),
        };

        self.log_event(event).await
    }

    pub async fn log_transaction_signing(
        &self,
        user_id: Option<String>,
        wallet_type: String,
        wallet_address: Option<Pubkey>,
        transaction: &Transaction,
        signature: Signature,
        security_context: SecurityContext,
        risk_level: RiskLevel,
    ) -> Result<Uuid> {
        let transaction_hash = self.calculate_transaction_hash(transaction)?;

        let event = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::TransactionSigned,
            user_id,
            wallet_type,
            wallet_address,
            transaction_signature: Some(signature),
            transaction_hash: Some(transaction_hash.clone()),
            nonce: Some(transaction.message.recent_blockhash.to_string()),
            details: AuditEventDetails {
                description: "Transaction signed successfully".to_string(),
                metadata: serde_json::json!({
                    "transaction_size": bincode::serialize(transaction).unwrap_or_default().len(),
                    "instruction_count": transaction.message.instructions.len(),
                    "account_count": transaction.message.account_keys.len(),
                    "recent_blockhash": transaction.message.recent_blockhash.to_string(),
                }),
                risk_level,
                compliance_flags: vec![
                    "TRANSACTION_SIGNED".to_string(),
                    "CRYPTOGRAPHIC_OPERATION".to_string(),
                ],
            },
            security_context,
            cryptographic_hash: transaction_hash,
        };

        self.log_event(event).await
    }

    pub async fn log_security_violation(
        &self,
        user_id: Option<String>,
        wallet_type: String,
        violation_type: String,
        details: serde_json::Value,
        security_context: SecurityContext,
    ) -> Result<Uuid> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::SecurityViolation,
            user_id,
            wallet_type,
            wallet_address: None,
            transaction_signature: None,
            transaction_hash: None,
            nonce: None,
            details: AuditEventDetails {
                description: format!("Security violation: {}", violation_type),
                metadata: details,
                risk_level: RiskLevel::Critical,
                compliance_flags: vec![
                    "SECURITY_VIOLATION".to_string(),
                    "IMMEDIATE_REVIEW_REQUIRED".to_string(),
                ],
            },
            security_context,
            cryptographic_hash: String::new(),
        };

        self.log_event(event).await
    }

    pub async fn log_replay_attack(
        &self,
        user_id: Option<String>,
        wallet_type: String,
        signature: Signature,
        security_context: SecurityContext,
    ) -> Result<Uuid> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::ReplayAttackDetected,
            user_id,
            wallet_type,
            wallet_address: None,
            transaction_signature: Some(signature),
            transaction_hash: None,
            nonce: None,
            details: AuditEventDetails {
                description: "Replay attack detected".to_string(),
                metadata: serde_json::json!({
                    "signature": signature.to_string(),
                    "detection_time": Utc::now().to_rfc3339(),
                }),
                risk_level: RiskLevel::Critical,
                compliance_flags: vec![
                    "REPLAY_ATTACK".to_string(),
                    "SECURITY_INCIDENT".to_string(),
                    "IMMEDIATE_ACTION_REQUIRED".to_string(),
                ],
            },
            security_context,
            cryptographic_hash: String::new(),
        };

        self.log_event(event).await
    }

    pub async fn log_wallet_disconnection(
        &self,
        user_id: Option<String>,
        wallet_type: String,
        connection: &crate::wallet::WalletConnection,
        security_context: SecurityContext,
    ) -> Result<Uuid> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::WalletDisconnected,
            user_id,
            wallet_type,
            wallet_address: None, // Would need to extract from connection
            transaction_signature: None,
            transaction_hash: None,
            nonce: None,
            details: AuditEventDetails {
                description: "Wallet disconnected".to_string(),
                metadata: serde_json::json!({
                    "connection_id": connection.id,
                    "wallet_type": format!("{:?}", connection.wallet_type),
                    "disconnection_time": Utc::now().to_rfc3339(),
                }),
                risk_level: RiskLevel::Low,
                compliance_flags: vec!["WALLET_DISCONNECTION".to_string()],
            },
            security_context,
            cryptographic_hash: String::new(),
        };

        self.log_event(event).await
    }

    pub async fn log_transaction_simulation(
        &self,
        user_id: Option<String>,
        wallet_type: String,
        transaction: &Transaction,
        simulation_result: &crate::wallet::transaction_validator::SimulationResult,
        security_context: SecurityContext,
    ) -> Result<Uuid> {
        let transaction_hash = self.calculate_transaction_hash(transaction)?;

        let event = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::TransactionSimulated,
            user_id,
            wallet_type,
            wallet_address: None,
            transaction_signature: None,
            transaction_hash: Some(transaction_hash.clone()),
            nonce: Some(transaction.message.recent_blockhash.to_string()),
            details: AuditEventDetails {
                description: "Transaction simulation completed".to_string(),
                metadata: serde_json::json!({
                    "simulation_success": simulation_result.success,
                    "units_consumed": simulation_result.units_consumed,
                    "fee": simulation_result.fee,
                    "account_changes": simulation_result.account_changes.len(),
                    "error": simulation_result.error,
                }),
                risk_level: if simulation_result.success { RiskLevel::Low } else { RiskLevel::Medium },
                compliance_flags: vec!["TRANSACTION_SIMULATION".to_string()],
            },
            security_context,
            cryptographic_hash: transaction_hash,
        };

        self.log_event(event).await
    }

    async fn log_event(&self, mut event: AuditEvent) -> Result<Uuid> {
        // Calculate cryptographic hash
        event.cryptographic_hash = self.calculate_event_hash(&event);

        // Add to pending events
        {
            let mut pending = self.pending_events.lock().await;
            pending.push(event.clone());
        }

        // Check if we need to flush
        let _ = self.check_and_flush().await;

        // Send alerts for high-risk events
        if event.details.risk_level >= self.config.min_risk_level_for_alert {
            self.send_security_alert(&event).await?;
        }

        Ok(event.id)
    }

    async fn check_and_flush(&self) -> Result<()> {
        let now = SystemTime::now();
        let mut last_flush = self.last_flush.lock().await;

        let should_flush = {
            let pending = self.pending_events.lock().await;
            pending.len() >= self.config.batch_size ||
            now.duration_since(*last_flush).unwrap_or(Duration::ZERO).as_secs() >= self.config.flush_interval_seconds
        };

        if should_flush {
            self.flush_events().await?;
            *last_flush = now;
        }

        Ok(())
    }

    async fn flush_events(&self) -> Result<()> {
        let mut pending = self.pending_events.lock().await;
        let events = pending.drain(..).collect::<Vec<_>>();
        drop(pending);

        if events.is_empty() {
            return Ok(());
        }

        // Write to file if enabled
        if self.config.enable_file_logging {
            self.write_to_file(&events).await?;
        }

        // Write to database if enabled
        if self.config.enable_database_logging {
            self.write_to_database(&events).await?;
        }

        Ok(())
    }

    async fn write_to_file(&self, events: &[AuditEvent]) -> Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let log_path = "logs/audit.log";
        std::fs::create_dir_all("logs").ok();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| SolanaRecoverError::IoError(e.to_string()))?;

        for event in events {
            let line = serde_json::to_string(event)
                .map_err(|e| SolanaRecoverError::SerializationError(e.to_string()))?;
            
            writeln!(file, "{}", line)
                .map_err(|e| SolanaRecoverError::IoError(e.to_string()))?;
        }

        Ok(())
    }

    async fn write_to_database(&self, _events: &[AuditEvent]) -> Result<()> {
        // Placeholder for database logging
        // In production, implement proper database storage
        Ok(())
    }

    async fn send_security_alert(&self, event: &AuditEvent) -> Result<()> {
        // Placeholder for alert system
        // In production, integrate with alerting systems like PagerDuty, Slack, etc.
        eprintln!("SECURITY ALERT: {:?} - {}", event.event_type, event.details.description);
        Ok(())
    }

    fn calculate_transaction_hash(&self, transaction: &Transaction) -> Result<String> {
        let serialized = bincode::serialize(transaction)
            .map_err(|e| SolanaRecoverError::SerializationError(e.to_string()))?;
        
        let mut hasher = Sha256::new();
        hasher.update(serialized);
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn calculate_event_hash(&self, event: &AuditEvent) -> String {
        let serialized = serde_json::to_string(event).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub async fn get_audit_trail(
        &self,
        _user_id: Option<String>,
        _start_time: DateTime<Utc>,
        _end_time: DateTime<Utc>,
        _event_types: Option<Vec<AuditEventType>>,
    ) -> Result<Vec<AuditEvent>> {
        // Placeholder for audit trail retrieval
        // In production, implement proper database queries
        Ok(Vec::new())
    }

    pub async fn get_security_metrics(&self) -> Result<SecurityMetrics> {
        // Placeholder for security metrics
        // In production, calculate from stored audit events
        Ok(SecurityMetrics {
            total_events: 0,
            security_violations: 0,
            replay_attacks: 0,
            high_risk_transactions: 0,
            average_risk_score: 0.0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetrics {
    pub total_events: u64,
    pub security_violations: u64,
    pub replay_attacks: u64,
    pub high_risk_transactions: u64,
    pub average_risk_score: f64,
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(AuditLoggerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        signature::Keypair,
        signature::Signer,
        transaction::Transaction,
        message::Message,
        hash::Hash,
    };

    #[tokio::test]
    async fn test_audit_logging() {
        let logger = AuditLogger::new(AuditLoggerConfig::default());
        
        let user_id = Some("test_user".to_string());
        let wallet_type = "PrivateKey".to_string();
        let wallet_address = Keypair::new().pubkey();
        
        let security_context = SecurityContext {
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("test-client".to_string()),
            session_id: Some("test-session".to_string()),
            correlation_id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            geo_location: None,
        };

        // Test wallet connection logging
        let event_id = logger.log_wallet_connection(
            user_id.clone(),
            wallet_type.clone(),
            wallet_address,
            security_context.clone(),
        ).await.unwrap();

        assert_ne!(event_id, Uuid::default());

        // Test transaction signing logging
        let keypair = Keypair::new();
        let message = Message::new(&[], Some(&keypair.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[&keypair], Hash::new_unique());

        let event_id = logger.log_transaction_signing(
            user_id,
            wallet_type,
            Some(keypair.pubkey()),
            &tx,
            *tx.signatures.first().unwrap(),
            security_context,
            RiskLevel::Medium,
        ).await.unwrap();

        assert_ne!(event_id, Uuid::default());
    }
}
