use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::vec::Vec;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub address: String,
    pub pubkey: Pubkey,
    pub total_accounts: u64,
    pub empty_accounts: u64,
    pub recoverable_lamports: u64,
    pub recoverable_sol: f64,
    pub empty_account_addresses: Vec<String>,
    pub scan_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyAccount {
    pub address: String,
    pub lamports: u64,
    pub owner: String,
    pub mint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub id: Uuid,
    pub wallet_address: String,
    pub status: ScanStatus,
    pub result: Option<WalletInfo>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScanStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchScanRequest {
    pub id: Uuid,
    pub wallet_addresses: Vec<String>,
    pub user_id: Option<String>,
    pub fee_percentage: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchScanResult {
    pub id: Uuid,
    pub batch_id: Option<String>, // For backward compatibility
    pub total_wallets: usize,
    pub successful_scans: usize,
    pub failed_scans: usize,
    pub completed_wallets: usize, // For backward compatibility
    pub failed_wallets: usize,   // For backward compatibility
    pub total_recoverable_sol: f64,
    pub estimated_fee_sol: f64,
    pub results: Vec<ScanResult>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEndpoint {
    pub url: String,
    pub priority: u8,
    pub rate_limit_rps: u32,
    pub timeout_ms: u64,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStructure {
    pub percentage: f64,
    pub minimum_lamports: u64,
    pub maximum_lamports: Option<u64>,
    pub waive_below_lamports: Option<u64>,
    pub firm_wallet_address: Option<String>,
}

impl Default for FeeStructure {
    fn default() -> Self {
        Self {
            percentage: 0.15, // 15%
            minimum_lamports: 1_000_000, // 0.001 SOL
            maximum_lamports: None,
            waive_below_lamports: Some(10_000_000), // 0.01 SOL
            firm_wallet_address: None, // Must be configured
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub api_key: Option<String>,
    pub fee_structure: Option<FeeStructure>,
    pub rate_limit_rps: Option<u32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_active: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanMetrics {
    pub total_scans: u64,
    pub successful_scans: u64,
    pub failed_scans: u64,
    pub total_recoverable_sol: f64,
    pub average_scan_time_ms: f64,
    pub wallets_processed: u64,
    pub empty_accounts_found: u64,
    pub requests_per_second: f64,
}

// Recovery types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub id: Uuid,
    pub wallet_address: String,
    pub empty_accounts: Vec<String>,
    pub destination_address: String,
    pub wallet_connection_id: Option<String>,
    pub max_fee_lamports: Option<u64>,
    pub priority_fee_lamports: Option<u64>,
    pub user_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryTransaction {
    pub id: Uuid,
    pub recovery_request_id: Uuid,
    pub transaction_signature: String,
    pub transaction_data: Vec<u8>,
    pub accounts_recovered: Vec<String>,
    pub lamports_recovered: u64,
    pub fee_paid: u64,
    pub status: TransactionStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub signed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Signing,
    Signed,
    Submitted,
    Confirmed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub id: Uuid,
    pub recovery_request_id: Uuid,
    pub wallet_address: String,
    pub total_accounts_recovered: usize,
    pub total_lamports_recovered: u64,
    pub total_fees_paid: u64,
    pub net_lamports: u64,
    pub net_sol: f64,
    pub transactions: Vec<RecoveryTransaction>,
    pub status: RecoveryStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryStatus {
    Pending,
    Building,
    Signing,
    Submitting,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub max_accounts_per_transaction: usize,
    pub priority_fee_lamports: u64,
    pub max_fee_lamports: u64,
    pub confirmation_timeout_seconds: u64,
    pub retry_attempts: u32,
    pub min_balance_lamports: u64,
    pub max_concurrent_recoveries: Option<usize>,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_accounts_per_transaction: 12, // Optimized for MTU limits
            priority_fee_lamports: 1_000_000, // 0.001 SOL
            max_fee_lamports: 5_000_000,    // 0.005 SOL
            confirmation_timeout_seconds: 120,
            retry_attempts: 3,
            min_balance_lamports: 5_000,     // Minimum balance to include in recovery
            max_concurrent_recoveries: Some(5),
        }
    }
}
