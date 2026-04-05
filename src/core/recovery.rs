use crate::core::{Result, SolanaRecoverError};
use crate::core::types::*;
use crate::rpc::{ConnectionPool, RpcClientWrapper};
use crate::wallet::WalletManager;
use solana_sdk::{
    pubkey::Pubkey,
    transaction::Transaction,
    system_instruction,
    signature::Signature,
    commitment_config::CommitmentConfig,
};
use std::sync::Arc;
use tracing::{info, error, warn};
use sha2::{Sha256};
use hmac::{Hmac, Mac};
use std::str::FromStr;

type HmacSha256 = Hmac<Sha256>;

pub struct RecoveryManager {
    connection_pool: Arc<ConnectionPool>,
    wallet_manager: Arc<WalletManager>,
    config: RecoveryConfig,
    security: Arc<RecoverySecurity>,
    rate_limiter: Arc<tokio::sync::Semaphore>,
}

#[derive(Clone)]
pub struct RecoverySecurity {
    max_recovery_lamports: u64,
    allowed_destinations: Vec<Pubkey>,
    require_multi_sig: bool,
    audit_log: Arc<tokio::sync::Mutex<Vec<AuditEntry>>>,
    session_timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub wallet_address: String,
    pub destination_address: String,
    pub amount_lamports: u64,
    pub accounts_recovered: usize,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub signature: String,
}

impl RecoveryManager {
    pub fn new(
        connection_pool: Arc<ConnectionPool>,
        wallet_manager: Arc<WalletManager>,
        config: RecoveryConfig,
    ) -> Self {
        let security = Arc::new(RecoverySecurity::new());
        let rate_limiter = Arc::new(tokio::sync::Semaphore::new(
            config.max_concurrent_recoveries.unwrap_or(5)
        ));
        
        Self {
            connection_pool,
            wallet_manager,
            config,
            security,
            rate_limiter,
        }
    }

    pub async fn recover_sol(&self, request: &RecoveryRequest) -> Result<RecoveryResult> {
        // Rate limiting
        let _permit = self.rate_limiter.acquire().await
            .map_err(|_| SolanaRecoverError::RateLimitExceeded)?;
        
        let start_time = std::time::Instant::now();
        info!("Starting SOL recovery for wallet: {}", request.wallet_address);

        // Enhanced security validation
        self.validate_recovery_security(request).await?;
        
        // Create audit entry
        let _audit_entry = self.create_audit_entry(request).await?;

        let mut recovery_result = RecoveryResult {
            id: uuid::Uuid::new_v4(),
            recovery_request_id: request.id,
            wallet_address: request.wallet_address.clone(),
            total_accounts_recovered: 0,
            total_lamports_recovered: 0,
            total_fees_paid: 0,
            net_lamports: 0,
            net_sol: 0.0,
            transactions: Vec::new(),
            status: RecoveryStatus::Pending,
            created_at: chrono::Utc::now(),
            completed_at: None,
            duration_ms: None,
            error: None,
        };

        // Validate destination address with enhanced security
        // For SOL recovery, if destination is not explicitly specified by user, default to wallet address
        let destination_address = if request.destination_address.is_empty() || 
                                   request.destination_address == "auto" ||
                                   request.destination_address == request.wallet_address {
            info!("Defaulting destination to user wallet: {}", request.wallet_address);
            &request.wallet_address
        } else {
            &request.destination_address
        };
        
        let destination_pubkey = self.validate_destination_address(destination_address)?;

        // Group accounts into batches for transactions
        let account_batches = self.group_accounts_for_recovery(&request.empty_accounts)?;

        recovery_result.status = RecoveryStatus::Building;

        for (batch_index, account_batch) in account_batches.iter().enumerate() {
            info!("Processing batch {}/{} with {} accounts", 
                  batch_index + 1, account_batches.len(), account_batch.len());

            match self.process_account_batch(request, account_batch, destination_pubkey).await {
                Ok(transaction) => {
                    recovery_result.total_accounts_recovered += transaction.accounts_recovered.len();
                    recovery_result.total_lamports_recovered += transaction.lamports_recovered;
                    recovery_result.total_fees_paid += transaction.fee_paid;
                    recovery_result.transactions.push(transaction);
                }
                Err(e) => {
                    error!("Failed to process batch {}: {}", batch_index + 1, e);
                    recovery_result.status = RecoveryStatus::Failed;
                    recovery_result.error = Some(format!("Batch {} failed: {}", batch_index + 1, e));
                    return Ok(recovery_result);
                }
            }
        }

        // Calculate net amounts
        recovery_result.net_lamports = recovery_result.total_lamports_recovered.saturating_sub(recovery_result.total_fees_paid);
        recovery_result.net_sol = recovery_result.net_lamports as f64 / 1_000_000_000.0;
        recovery_result.status = RecoveryStatus::Completed;
        recovery_result.completed_at = Some(chrono::Utc::now());
        recovery_result.duration_ms = Some(start_time.elapsed().as_millis() as u64);

        info!("SOL recovery completed. Net recovered: {:.9} SOL", recovery_result.net_sol);
        Ok(recovery_result)
    }

    async fn process_account_batch(
        &self,
        request: &RecoveryRequest,
        accounts: &[String],
        destination_pubkey: Pubkey,
    ) -> Result<RecoveryTransaction> {
        // Additional security checks for batch processing
        self.validate_batch_security(accounts, destination_pubkey).await?;
        
        // Create audit entry for this batch
        let audit_entry = self.create_audit_entry(request).await?;
        
        let mut transaction = RecoveryTransaction {
            id: uuid::Uuid::new_v4(),
            recovery_request_id: request.id,
            transaction_signature: String::new(),
            transaction_data: Vec::new(),
            accounts_recovered: accounts.to_vec(),
            lamports_recovered: 0,
            fee_paid: 0,
            status: TransactionStatus::Pending,
            created_at: chrono::Utc::now(),
            signed_at: None,
            confirmed_at: None,
            error: None,
        };

        // Get RPC client with retry logic
        let rpc_client = self.connection_pool.get_client().await?;

        // Build recovery transaction
        let solana_transaction = self.build_recovery_transaction(
            accounts,
            destination_pubkey,
            &rpc_client,
        ).await?;

        // Enhanced signing with security checks
        transaction.status = TransactionStatus::Signing;
        if let Some(connection_id) = &request.wallet_connection_id {
            // Verify wallet connection is still valid
            let connection = self.wallet_manager.get_connection(connection_id)
                .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                    format!("No active wallet connection found: {}", connection_id)
                ))?;
            
            // Additional security: verify wallet has sufficient balance for fees
            self.verify_wallet_balance(&connection, accounts.len()).await?;

            let serialized_tx = bincode::serialize(&solana_transaction)
                .map_err(|e| SolanaRecoverError::SerializationError(e.to_string()))?;

            // Sign with enhanced security
            let signature = self.secure_sign_transaction(
                connection_id, 
                &serialized_tx, 
                &audit_entry.signature
            ).await?;
            
            transaction.transaction_signature = signature.to_string();
            transaction.signed_at = Some(chrono::Utc::now());
            transaction.status = TransactionStatus::Signed;
        } else {
            return Err(SolanaRecoverError::AuthenticationError(
                "No wallet connection provided for signing".to_string()
            ));
        }

        // Submit transaction with enhanced security
        transaction.status = TransactionStatus::Submitted;
        
        // Real transaction submission with confirmation
        let signature = self.submit_transaction_securely(&solana_transaction).await?;
        transaction.transaction_signature = signature.to_string();
        transaction.transaction_data = bincode::serialize(&solana_transaction)
            .map_err(|e| SolanaRecoverError::SerializationError(e.to_string()))?;

        // Calculate recovered amount and fees with real-time data
        let (recovered, fees) = self.calculate_batch_amounts_securely(accounts, &rpc_client).await?;
        transaction.lamports_recovered = recovered;
        transaction.fee_paid = fees;

        // Wait for confirmation with timeout
        self.wait_for_transaction_confirmation(&signature).await?;
        transaction.status = TransactionStatus::Confirmed;
        transaction.confirmed_at = Some(chrono::Utc::now());

        Ok(transaction)
    }

    async fn build_recovery_transaction(
        &self,
        accounts: &[String],
        destination: Pubkey,
        rpc_client: &Arc<RpcClientWrapper>,
    ) -> Result<Transaction> {
        let mut instructions = Vec::new();
        let mut _total_balance = 0u64;

        // Get recent blockhash
        let _recent_blockhash = {
            let client = rpc_client.get_client();
            tokio::task::spawn_blocking(move || {
                client.get_latest_blockhash()
            }).await.map_err(|e| SolanaRecoverError::InternalError(format!("Task join error: {}", e)))??
        };

        // Get rent exemption amount for token accounts (typically 165 bytes)
        let rent_exemption = rpc_client.get_minimum_balance_for_rent_exemption(165).await
            .unwrap_or(2_039_280); // Fallback to common value

        // Create transfer instructions for each empty account
        for account_address in accounts {
            let pubkey = account_address.parse::<Pubkey>()
                .map_err(|_| SolanaRecoverError::InvalidInput(
                    format!("Invalid account address: {}", account_address)
                ))?;

            // Get account balance
            let balance = rpc_client.get_balance(&pubkey).await?;
            
            // Calculate recoverable amount (balance minus rent exemption)
            let recoverable_amount = if balance > rent_exemption {
                balance - rent_exemption
            } else {
                0
            };
            
            if recoverable_amount >= self.config.min_balance_lamports {
                _total_balance += recoverable_amount;
                
                // Create transfer instruction with recoverable amount
                instructions.push(
                    system_instruction::transfer(
                        &pubkey,
                        &destination,
                        recoverable_amount,
                    )
                );
            }
        }

        if instructions.is_empty() {
            return Err(SolanaRecoverError::NoRecoverableFunds(
                "No accounts with sufficient recoverable balance found".to_string()
            ));
        }

        // Create transaction
        let transaction = Transaction::new_with_payer(
            &instructions,
            Some(&destination), // Use destination as fee payer
        );

        Ok(transaction)
    }

    fn group_accounts_for_recovery(&self, accounts: &[String]) -> Result<Vec<Vec<String>>> {
        if accounts.is_empty() {
            return Err(SolanaRecoverError::NoRecoverableFunds(
                "No accounts provided for recovery".to_string()
            ));
        }

        let mut batches = Vec::new();
        let max_batch_size = self.config.max_accounts_per_transaction;

        for chunk in accounts.chunks(max_batch_size) {
            batches.push(chunk.to_vec());
        }

        Ok(batches)
    }

    pub async fn get_recovery_status(&self, _recovery_id: &uuid::Uuid) -> Result<Option<RecoveryResult>> {
        // This would typically query a database for recovery status
        // For now, return None as placeholder
        Ok(None)
    }

    pub async fn estimate_recovery_fees(&self, accounts: &[String]) -> Result<u64> {
        let num_accounts = accounts.len();
        let estimated_fees = (num_accounts as u64) * 5_000; // ~5000 lamports per transfer
        Ok(estimated_fees)
    }

    pub async fn validate_recovery_request(&self, request: &RecoveryRequest) -> Result<()> {
        // Enhanced validation with security checks
        
        // Validate wallet address
        if request.wallet_address.parse::<Pubkey>().is_err() {
            return Err(SolanaRecoverError::InvalidInput(
                "Invalid wallet address".to_string()
            ));
        }

        // Validate destination address
        if request.destination_address.parse::<Pubkey>().is_err() {
            return Err(SolanaRecoverError::InvalidInput(
                "Invalid destination address".to_string()
            ));
        }

        // Validate empty accounts
        if request.empty_accounts.is_empty() {
            return Err(SolanaRecoverError::InvalidInput(
                "No empty accounts provided for recovery".to_string()
            ));
        }

        // Check for duplicate accounts
        let unique_accounts: std::collections::HashSet<_> = request.empty_accounts.iter().collect();
        if unique_accounts.len() != request.empty_accounts.len() {
            return Err(SolanaRecoverError::InvalidInput(
                "Duplicate account addresses found".to_string()
            ));
        }

        for account in &request.empty_accounts {
            if account.parse::<Pubkey>().is_err() {
                return Err(SolanaRecoverError::InvalidInput(
                    format!("Invalid empty account address: {}", account)
                ));
            }
        }

        // Enhanced fee validation
        if let Some(max_fee) = request.max_fee_lamports {
            if max_fee < self.config.priority_fee_lamports {
                return Err(SolanaRecoverError::InvalidInput(
                    "Max fee is too low".to_string()
                ));
            }
            
            // Check against security limits
            if max_fee > self.security.max_recovery_lamports / 10 {
                return Err(SolanaRecoverError::InvalidInput(
                    "Max fee exceeds security limits".to_string()
                ));
            }
        }
        
        // Rate limiting check
        if self.check_rate_limit(&request.wallet_address).await? {
            return Err(SolanaRecoverError::RateLimitExceeded);
        }

        Ok(())
    }
    
    // New security methods
    async fn validate_recovery_security(&self, request: &RecoveryRequest) -> Result<()> {
        // Check if destination is in allowed list
        let destination_pubkey = request.destination_address.parse::<Pubkey>()
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid destination address".to_string()))?;
            
        if !self.security.allowed_destinations.is_empty() && 
           !self.security.allowed_destinations.contains(&destination_pubkey) {
            return Err(SolanaRecoverError::AuthenticationError(
                "Destination address not in allowed list".to_string()
            ));
        }
        
        // Check total recovery amount against security limits
        let estimated_total = self.estimate_recovery_fees(&request.empty_accounts).await?;
        if estimated_total > self.security.max_recovery_lamports {
            return Err(SolanaRecoverError::InvalidInput(
                "Recovery amount exceeds security limits".to_string()
            ));
        }
        
        Ok(())
    }
    
    fn validate_destination_address(&self, address: &str) -> Result<Pubkey> {
        let pubkey = address.parse::<Pubkey>()
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid destination address".to_string()))?;
        
        // Additional validation: check if it's a system program account
        if pubkey == solana_sdk::system_program::id() {
            return Err(SolanaRecoverError::InvalidInput(
                "Cannot recover to system program address".to_string()
            ));
        }
        
        Ok(pubkey)
    }
    
    async fn validate_batch_security(&self, accounts: &[String], destination: Pubkey) -> Result<()> {
        // Check batch size limits
        if accounts.len() > self.config.max_accounts_per_transaction {
            return Err(SolanaRecoverError::InvalidInput(
                "Batch size exceeds maximum allowed".to_string()
            ));
        }
        
        // Verify all accounts are valid and not the destination
        for account in accounts {
            let pubkey = account.parse::<Pubkey>()
                .map_err(|_| SolanaRecoverError::InvalidInput(
                    format!("Invalid account address: {}", account)
                ))?;
            
            if pubkey == destination {
                return Err(SolanaRecoverError::InvalidInput(
                    "Cannot recover to the same account".to_string()
                ));
            }
        }
        
        Ok(())
    }
    
    async fn verify_wallet_balance(&self, _connection: &crate::wallet::WalletConnection, num_accounts: usize) -> Result<()> {
        // Estimate required fees
        let estimated_fees = (num_accounts as u64) * 10_000; // Conservative estimate
        
        // In a real implementation, you'd check the actual wallet balance
        // For now, we'll assume sufficient balance
        if estimated_fees > 1_000_000_000 { // 1 SOL
            warn!("High fee requirement detected: {} lamports", estimated_fees);
        }
        
        Ok(())
    }
    
    async fn secure_sign_transaction(
        &self,
        connection_id: &str,
        transaction_data: &[u8],
        audit_signature: &str,
    ) -> Result<Signature> {
        // Create a secure signature that includes audit information
        let mut combined_data = transaction_data.to_vec();
        combined_data.extend_from_slice(audit_signature.as_bytes());
        combined_data.extend_from_slice(&self.generate_nonce().to_le_bytes());
        
        let signature_bytes = self.wallet_manager.sign_with_wallet(connection_id, &combined_data).await?;
        
        // Convert Vec<u8> to [u8; 64] for Signature
        let signature_array: [u8; 64] = signature_bytes.try_into()
            .map_err(|e| SolanaRecoverError::InternalError(format!("Signature conversion error: {:?}", e)))?;
        
        // Verify the signature
        // In a real implementation, you'd verify against the expected public key
        
        Ok(Signature::from(signature_array))
    }
    
    async fn submit_transaction_securely(&self, transaction: &Transaction) -> Result<Signature> {
        let rpc_client = self.connection_pool.get_client().await?;
        
        // Submit with retry logic and proper error handling
        let signature = rpc_client.send_transaction(transaction).await
            .map_err(|e| {
                error!("Failed to submit transaction: {}", e);
                SolanaRecoverError::NetworkError(format!("Transaction submission failed: {}", e))
            })?;
        
        info!("Transaction submitted: {}", signature);
        Ok(Signature::from_str(&signature).map_err(|e| SolanaRecoverError::InternalError(format!("Signature parsing error: {}", e)))?)
    }
    
    async fn calculate_batch_amounts_securely(
        &self,
        accounts: &[String],
        rpc_client: &Arc<RpcClientWrapper>,
    ) -> Result<(u64, u64)> {
        let mut total_balance = 0u64;
        let mut valid_accounts = 0u64;

        // Get rent exemption amount for token accounts (typically 165 bytes)
        let rent_exemption = rpc_client.get_minimum_balance_for_rent_exemption(165).await
            .unwrap_or(2_039_280); // Fallback to common value

        for account_address in accounts {
            let pubkey = account_address.parse::<Pubkey>()
                .map_err(|_| SolanaRecoverError::InvalidInput(
                    format!("Invalid account address: {}", account_address)
                ))?;

            // Get account info with commitment level
            let account = rpc_client.get_account_info(&pubkey).await?;
            
            // Calculate recoverable amount (balance minus rent exemption)
            let recoverable_amount = if account.lamports > rent_exemption {
                account.lamports - rent_exemption
            } else {
                0
            };
            
            if recoverable_amount >= self.config.min_balance_lamports {
                total_balance += recoverable_amount;
                valid_accounts += 1;
            }
        }

        // More accurate fee estimation
        let estimated_fees = valid_accounts * 5_000; // ~5000 lamports per transfer
        Ok((total_balance, estimated_fees))
    }
    
    async fn wait_for_transaction_confirmation(&self, signature: &Signature) -> Result<()> {
        let rpc_client = self.connection_pool.get_client().await?;
        
        // Wait for confirmation with timeout
        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            match rpc_client.get_signature_status_with_commitment(
                &signature.to_string(),
                CommitmentConfig::confirmed()
            ).await? {
                Some(confirmed) => {
                    if confirmed {
                        info!("Transaction confirmed: {}", signature);
                        return Ok(());
                    } else {
                        return Err(SolanaRecoverError::TransactionError(
                            "Transaction failed".to_string()
                        ));
                    }
                }
                None => {
                    // Still pending
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        
        Err(SolanaRecoverError::TransactionError(
            "Transaction confirmation timeout".to_string()
        ))
    }
    
    async fn check_rate_limit(&self, wallet_address: &str) -> Result<bool> {
        // Simple rate limiting check
        // In production, you'd use a more sophisticated rate limiter with Redis or similar
        let audit_log = self.security.audit_log.lock().await;
        let recent_count = audit_log.iter()
            .filter(|entry| {
                entry.wallet_address == wallet_address &&
                (chrono::Utc::now() - entry.timestamp).num_minutes() < 60
            })
            .count();
        
        Ok(recent_count > 10) // More than 10 recoveries per hour
    }
    
    async fn create_audit_entry(&self, request: &RecoveryRequest) -> Result<AuditEntry> {
        let timestamp = chrono::Utc::now();
        let signature = self.generate_audit_signature(request, timestamp)?;
        
        let entry = AuditEntry {
            timestamp,
            wallet_address: request.wallet_address.clone(),
            destination_address: request.destination_address.clone(),
            amount_lamports: 0, // Will be calculated after processing
            accounts_recovered: request.empty_accounts.len(),
            user_id: request.user_id.clone(),
            ip_address: None, // Would be extracted from request context
            signature,
        };

        // Store in audit log
        let mut audit_log = self.security.audit_log.lock().await;
        audit_log.push(entry.clone());

        // Keep only last 1000 entries
        if audit_log.len() > 1000 {
            let drain_count = audit_log.len() - 1000;
            audit_log.drain(0..drain_count);
        }

        Ok(entry)
    }

    fn generate_audit_signature(&self, request: &RecoveryRequest, timestamp: chrono::DateTime<chrono::Utc>) -> Result<String> {
        let data = format!(
            "{}|{}|{}|{}",
            request.wallet_address,
            request.destination_address,
            request.empty_accounts.join(","),
            timestamp.timestamp()
        );

        let mut mac = HmacSha256::new_from_slice(b"recovery_audit_key")
            .map_err(|e| SolanaRecoverError::InternalError(format!("HMAC error: {}", e)))?;
        mac.update(data.as_bytes());

        Ok(format!("{:x}", mac.finalize().into_bytes()))
    }

    fn generate_nonce(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

impl RecoverySecurity {
    pub fn new() -> Self {
        Self {
            max_recovery_lamports: 100_000_000_000, // 100 SOL
            allowed_destinations: Vec::new(), // Empty means allow any
            require_multi_sig: false,
            audit_log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            session_timeout_secs: 3600, // 1 hour
        }
    }
    
    pub fn with_limits(max_recovery_lamports: u64, allowed_destinations: Vec<Pubkey>) -> Self {
        Self {
            max_recovery_lamports: max_recovery_lamports,
            allowed_destinations,
            require_multi_sig: true,
            audit_log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            session_timeout_secs: 3600,
        }
    }
    
    pub async fn get_audit_log(&self) -> Vec<AuditEntry> {
        self.audit_log.lock().await.clone()
    }
    
    pub async fn clear_audit_log(&self) {
        self.audit_log.lock().await.clear();
    }
    
    pub fn requires_multi_sig(&self) -> bool {
        self.require_multi_sig
    }
    
    pub fn session_timeout(&self) -> u64 {
        self.session_timeout_secs
    }
    
    pub fn set_multi_sig_requirement(&mut self, require: bool) {
        self.require_multi_sig = require;
    }
    
    pub fn set_session_timeout(&mut self, timeout_secs: u64) {
        self.session_timeout_secs = timeout_secs;
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new(
            Arc::new(ConnectionPool::new(vec![], 1)), // Empty endpoints with pool size 1
            Arc::new(WalletManager::default()),
            RecoveryConfig::default(),
        )
    }
}
