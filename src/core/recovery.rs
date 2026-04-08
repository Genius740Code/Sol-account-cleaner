use crate::core::{Result, SolanaRecoverError};
use crate::core::types::*;
use crate::rpc::{ConnectionPool, RpcClientWrapper, ConnectionPoolTrait};
use crate::wallet::WalletManager;
use solana_sdk::{
    pubkey::Pubkey,
    transaction::Transaction,
    system_instruction,
    signature::Signature,
    commitment_config::CommitmentConfig,
    signature::Keypair,
    signer::Signer,
};
use spl_token::instruction::close_account;
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
    fee_structure: FeeStructure,
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
    audit_key: String,
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
        fee_structure: FeeStructure,
    ) -> Self {
        let security = Arc::new(RecoverySecurity::new());
        let rate_limiter = Arc::new(tokio::sync::Semaphore::new(
            config.max_concurrent_recoveries.unwrap_or(5)
        ));
        
        Self {
            connection_pool,
            wallet_manager,
            config,
            fee_structure,
            security,
            rate_limiter,
        }
    }

    pub async fn recover_sol(&self, request: &RecoveryRequest) -> Result<RecoveryResult> {
        // Rate limiting
        let _permit = self.rate_limiter.acquire().await
            .map_err(|_| SolanaRecoverError::RateLimitExceeded("Rate limit exceeded".to_string()))?;
        
        let start_time = std::time::Instant::now();
        info!("Starting SOL recovery for wallet: {}", request.wallet_address);

        // Require wallet connection for security
        let _wallet_connection_id = request.wallet_connection_id.as_ref()
            .ok_or_else(|| SolanaRecoverError::AuthenticationError("Wallet connection required for SOL recovery. Please provide a private key to prove ownership.".to_string()))?;

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
            info!("Auto-defaulting destination to source wallet: {}", request.wallet_address);
            &request.wallet_address
        } else {
            info!("Using explicit destination: {}", request.destination_address);
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

        // PRE-CALCULATION: Calculate recoverable amounts BEFORE closing accounts
        let (pre_calculated_recovered, estimated_fees) = self.calculate_batch_amounts_before_close(accounts, &rpc_client).await?;
        
        if pre_calculated_recovered == 0 {
            return Err(SolanaRecoverError::NoRecoverableFunds(
                "No recoverable SOL found in accounts".to_string()
            ));
        }

        // Build recovery transaction
        let (solana_transaction, _fee_payer) = self.build_recovery_transaction(
            accounts,
            destination_pubkey,
            &rpc_client,
        ).await?;

        // Enhanced signing with security checks
        transaction.status = TransactionStatus::Signing;
        let signed_transaction_bytes: Vec<u8>;
        
        if let Some(connection_id) = &request.wallet_connection_id {
            // Verify wallet connection is still valid
            let connection = self.wallet_manager.get_connection(connection_id)
                .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                    format!("No active wallet connection found: {}", connection_id)
                ))?;
            
            // Additional security: verify wallet has sufficient balance for fees
            self.verify_wallet_balance(&connection, accounts.len()).await?;

            let _serialized_tx = bincode::serialize(&solana_transaction)
                .map_err(|e| SolanaRecoverError::SerializationError(e.to_string()))?;

            // Get wallet public key from connection for signing
            let _wallet_pubkey = match &connection.connection_data {
                crate::wallet::ConnectionData::PrivateKey { private_key } => {
                    // Parse private key to get public key
                    crate::wallet::private_key::PrivateKeyProvider::new()
                        .parse_private_key(private_key)
                        .map_err(|_| SolanaRecoverError::AuthenticationError("Failed to parse private key".to_string()))
                        .map(|kp| kp.pubkey())?
                }
                _ => {
                    return Err(SolanaRecoverError::AuthenticationError("Invalid wallet connection type for private key recovery".to_string()));
                }
            };

            // Create keypair for signing
            let keypair = {
                let connection = self.wallet_manager.get_connection(connection_id)
                    .ok_or_else(|| SolanaRecoverError::AuthenticationError(
                        format!("No active wallet connection found: {}", connection_id)
                    ))?;
                
                match &connection.connection_data {
                    crate::wallet::ConnectionData::PrivateKey { private_key } => {
                        // Parse private key to get keypair
                        crate::wallet::private_key::PrivateKeyProvider::new()
                            .parse_private_key(private_key)
                            .map_err(|_| SolanaRecoverError::AuthenticationError("Failed to parse private key".to_string()))?
                    }
                    _ => {
                        return Err(SolanaRecoverError::AuthenticationError("Invalid wallet connection type for private key recovery".to_string()));
                    }
                }
            };

            // Sign with enhanced security - now returns full signed transaction
            signed_transaction_bytes = self.secure_sign_transaction_with_keypair(
                &solana_transaction,
                &keypair,
                &audit_entry.signature
            ).await?;
            
            // Deserialize to signed transaction to get the signature
            let signed_tx: Transaction = bincode::deserialize(&signed_transaction_bytes)
                .map_err(|e| SolanaRecoverError::SerializationError(format!("Failed to deserialize signed transaction: {}", e)))?;
            
            // Extract the signature from the signed transaction
            if let Some(signature) = signed_tx.signatures.get(0) {
                transaction.transaction_signature = signature.to_string();
                transaction.signed_at = Some(chrono::Utc::now());
                transaction.status = TransactionStatus::Signed;
            } else {
                return Err(SolanaRecoverError::InternalError("No signature found in signed transaction".to_string()));
            }
        } else {
            return Err(SolanaRecoverError::AuthenticationError(
                "No wallet connection provided for signing".to_string()
            ));
        }

        // Submit transaction with enhanced security
        transaction.status = TransactionStatus::Submitted;
        
        // Deserialize the signed transaction for submission
        let signed_tx: Transaction = bincode::deserialize(&signed_transaction_bytes)
            .map_err(|e| SolanaRecoverError::SerializationError(format!("Failed to deserialize signed transaction for submission: {}", e)))?;
        
        // Real transaction submission with confirmation
        info!("Submitting transaction with {} instructions", signed_tx.message.instructions.len());
        match self.submit_transaction_securely(&signed_tx).await {
            Ok(signature) => {
                transaction.transaction_signature = signature.to_string();
                transaction.transaction_data = signed_transaction_bytes;
                info!("Transaction submitted successfully: {}", signature);

                // Wait for confirmation with timeout
                match self.wait_for_transaction_confirmation(&signature).await {
                    Ok(()) => {
                        transaction.status = TransactionStatus::Confirmed;
                        transaction.confirmed_at = Some(chrono::Utc::now());
                        info!("Transaction confirmed successfully");
                    }
                    Err(e) => {
                        error!("Transaction confirmation failed: {}", e);
                        transaction.status = TransactionStatus::Failed;
                        transaction.error = Some(format!("Confirmation failed: {}", e));
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                error!("Transaction submission failed: {}", e);
                transaction.status = TransactionStatus::Failed;
                transaction.error = Some(format!("Submission failed: {}", e));
                return Err(e);
            }
        }

        // Use PRE-CALCULATED amounts (calculated before accounts were closed)
        transaction.lamports_recovered = pre_calculated_recovered;
        transaction.fee_paid = estimated_fees;

        Ok(transaction)
    }

    async fn build_recovery_transaction(
        &self,
        accounts: &[String],
        destination: Pubkey,
        rpc_client: &Arc<RpcClientWrapper>,
    ) -> Result<(Transaction, Pubkey)> {
        let mut instructions = Vec::new();
        let mut total_balance = 0u64;

        // Get recent blockhash
        let recent_blockhash = {
            let client = rpc_client.get_client();
            tokio::task::spawn_blocking(move || {
                client.get_latest_blockhash()
            }).await.map_err(|e| SolanaRecoverError::InternalError(format!("Task join error: {}", e)))??
        };

        // Get rent exemption amount for token accounts (typically 165 bytes)
        let rent_exemption = rpc_client.get_minimum_balance_for_rent_exemption(165).await
            .unwrap_or(2_039_280); // Fallback to common value

        // OPTIMIZATION: Batch fetch all account info at once
        let account_pubkeys: Result<Vec<Pubkey>> = accounts.iter()
            .map(|addr| addr.parse::<Pubkey>()
                .map_err(|_| SolanaRecoverError::InvalidInput(
                    format!("Invalid account address: {}", addr)
                )))
            .collect();
        
        let account_pubkeys = account_pubkeys?;
        let batch_size = 100; // Process in batches to avoid RPC limits
        
        // For private key wallets, the destination should be the wallet address
        // Use destination as wallet public key for simplicity
        let wallet_pubkey = destination;
        
        for chunk in account_pubkeys.chunks(batch_size) {
            let account_infos = rpc_client.get_multiple_accounts(chunk).await?;
            
            for (account_pubkey, account_info_opt) in chunk.iter().zip(account_infos) {
                if let Some(account_info) = account_info_opt {
                    // Parse owner as Pubkey
                    let owner_pubkey = account_info.owner.parse::<Pubkey>()
                        .map_err(|_| SolanaRecoverError::InvalidInput(
                            format!("Invalid owner pubkey: {}", account_info.owner)
                        ))?;
                    
                    // Check if this is a token account (owned by SPL Token or Token-2022 program)
                    if owner_pubkey == spl_token::id() || owner_pubkey == spl_token_2022::id() {
                        // This is a token account - use close_account instruction
                        // This will transfer the entire balance (including rent) to the destination
                        let close_instruction = close_account(
                            &spl_token::id(), // token program id
                            account_pubkey,   // account to close
                            &destination,     // destination for reclaimed lamports
                            &owner_pubkey,    // actual account owner (must be token account owner)
                            &[],              // additional signers
                        )
                        .map_err(|e| SolanaRecoverError::InternalError(format!("Failed to create close_account instruction: {}", e)))?;
                        
                        instructions.push(close_instruction);
                        total_balance += account_info.lamports;
                    } else {
                        // This is a system account - use transfer instruction for recoverable amount only
                        let recoverable_amount = if account_info.lamports > rent_exemption {
                            account_info.lamports - rent_exemption
                        } else {
                            0
                        };
                        
                        if recoverable_amount >= self.config.min_balance_lamports {
                            total_balance += recoverable_amount;
                            
                            // Create transfer instruction with recoverable amount
                            instructions.push(
                                system_instruction::transfer(
                                    account_pubkey,
                                    &destination,
                                    recoverable_amount,
                                )
                            );
                        }
                    }
                }
            }
        }

        if instructions.is_empty() {
            return Err(SolanaRecoverError::NoRecoverableFunds(
                "No accounts with sufficient recoverable balance found".to_string()
            ));
        }

        // Create transaction with fee payer and required signers
        let mut required_signers = vec![wallet_pubkey]; // Always include wallet as signer
        
        // Add unique account owners as additional signers for token accounts
        for chunk in account_pubkeys.chunks(batch_size) {
            let account_infos = rpc_client.get_multiple_accounts(chunk).await?;
            
            for (_account_pubkey, account_info_opt) in chunk.iter().zip(account_infos) {
                if let Some(account_info) = account_info_opt {
                    // Parse owner as Pubkey
                    let owner_pubkey = account_info.owner.parse::<Pubkey>()
                        .map_err(|_| SolanaRecoverError::InvalidInput(
                            format!("Invalid owner pubkey: {}", account_info.owner)
                        ))?;
                    
                    // For token accounts, add the owner as required signer if different from wallet
                    if (owner_pubkey == spl_token::id() || owner_pubkey == spl_token_2022::id()) 
                        && owner_pubkey != wallet_pubkey {
                        required_signers.push(owner_pubkey);
                    }
                }
            }
        }
        
        // Remove duplicates and create transaction
        required_signers.sort();
        required_signers.dedup();
        
        let transaction = Transaction::new_with_payer(
            &instructions,
            Some(&wallet_pubkey), // Use wallet public key as fee payer
        );
        
        // Set the recent blockhash
        let mut transaction_with_blockhash = transaction;
        transaction_with_blockhash.message.recent_blockhash = recent_blockhash;

        // Check if fee payer has sufficient balance for transaction fees
        let fee_payer_balance = rpc_client.get_balance(&wallet_pubkey).await.unwrap_or(0);
        let min_required_balance = 1_000_000; // 0.001 SOL minimum for fees
        
        if fee_payer_balance < min_required_balance {
            return Err(SolanaRecoverError::InsufficientBalance { 
                required: min_required_balance, 
                available: fee_payer_balance 
            });
        }

        // Add fee injection if configured and there are recoverable funds
        if let Some(firm_wallet_address) = &self.fee_structure.firm_wallet_address {
            if total_balance > 0 {
                let firm_pubkey = firm_wallet_address.parse::<Pubkey>()
                    .map_err(|_| SolanaRecoverError::InvalidInput(
                        format!("Invalid firm wallet address: {}", firm_wallet_address)
                    ))?;
                
                // Calculate fee using FeeCalculator
                let fee_calculation = crate::core::fee_calculator::FeeCalculator::calculate_fee(
                    total_balance,
                    &self.fee_structure
                );
                
                if fee_calculation.fee_lamports > 0 && !fee_calculation.fee_waived {
                    // Add fee transfer instruction
                    let fee_instruction = system_instruction::transfer(
                        &wallet_pubkey, // From wallet (fee payer)
                        &firm_pubkey, // To firm wallet
                        fee_calculation.fee_lamports,
                    );
                    
                    // Convert Instruction to CompiledInstruction manually
                    let compiled_instruction = solana_sdk::instruction::CompiledInstruction {
                        program_id_index: 0, // System program is usually at index 0 after fee payer
                        accounts: vec![0, 1], // destination and firm accounts
                        data: fee_instruction.data.clone(),
                    };
                    
                    transaction_with_blockhash.message.instructions.push(compiled_instruction);
                    
                    info!("Fee injection added: {} lamports to firm wallet {}", 
                          fee_calculation.fee_lamports, firm_wallet_address);
                }
            }
        }

        Ok((transaction_with_blockhash, wallet_pubkey))
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
            return Err(SolanaRecoverError::RateLimitExceeded("Rate limit exceeded".to_string()));
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
    
    async fn secure_sign_transaction_with_keypair(
        &self,
        transaction: &Transaction,
        keypair: &Keypair,
        _audit_signature: &str, // Audit data stored separately, not mixed into signature
    ) -> Result<Vec<u8>> {
        // Debug: log the message account keys to understand what signers are expected
        info!("Transaction message account keys: {:?}", transaction.message.account_keys);
        info!("Transaction instructions: {:?}", transaction.message.instructions);
        info!("Provided keypair pubkey: {:?}", keypair.pubkey());
        
        // CRITICAL FIX: Verify the keypair is actually a required signer for this transaction
        let keypair_pubkey = keypair.pubkey();
        if !transaction.message.account_keys.contains(&keypair_pubkey) {
            return Err(SolanaRecoverError::AuthenticationError(
                format!("Provided keypair {} is not a signer for this transaction", keypair_pubkey)
            ));
        }
        
        // Create a mutable copy of the transaction for signing
        let mut tx = transaction.clone();
        
        // CRITICAL FIX: Use correct signer array format
        tx.sign(&keypair, tx.message.recent_blockhash);
        
        // Return the full serialized signed transaction
        bincode::serialize(&tx)
            .map_err(|e| SolanaRecoverError::SerializationError(format!("Failed to serialize signed transaction: {}", e)))
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
    
    async fn calculate_batch_amounts_before_close(
        &self,
        accounts: &[String],
        rpc_client: &Arc<RpcClientWrapper>,
    ) -> Result<(u64, u64)> {
        let mut total_balance = 0u64;
        let mut valid_accounts = 0u64;

        for account_address in accounts {
            let pubkey = account_address.parse::<Pubkey>()
                .map_err(|_| SolanaRecoverError::InvalidInput(
                    format!("Invalid account address: {}", account_address)
                ))?;

            // Get account info with commitment level
            let account = rpc_client.get_account_info(&pubkey).await?;
            
            // Parse owner as Pubkey
            let owner_pubkey = account.owner.parse::<Pubkey>()
                .map_err(|_| SolanaRecoverError::InvalidInput(
                    format!("Invalid owner pubkey: {}", account.owner)
                ))?;
            
            // Calculate recoverable amount based on account type
            let recoverable_amount = if owner_pubkey == spl_token::id() || owner_pubkey == spl_token_2022::id() {
                // For token accounts: recover FULL balance (including rent exemption)
                // This fixes the "Zero Reporting Bug" - we count everything for token accounts
                account.lamports
            } else {
                // For system accounts: only recover amount above rent exemption
                let rent_exemption = rpc_client.get_minimum_balance_for_rent_exemption(165).await
                    .unwrap_or(2_039_280); // Fallback to common value
                
                if account.lamports > rent_exemption {
                    account.lamports - rent_exemption
                } else {
                    0
                }
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

        let mut mac = HmacSha256::new_from_slice(self.security.audit_key.as_bytes())
            .map_err(|e| SolanaRecoverError::InternalError(format!("HMAC error: {}", e)))?;
        mac.update(data.as_bytes());

        Ok(format!("{:x}", mac.finalize().into_bytes()))
    }

    #[allow(dead_code)]
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
        let audit_key = std::env::var("RECOVERY_AUDIT_KEY")
            .unwrap_or_else(|_| {
                // Generate a random key if not configured
                format!("{:x}", rand::random::<u64>())
            });
        
        Self {
            max_recovery_lamports: 100_000_000_000, // 100 SOL
            allowed_destinations: Vec::new(), // Empty means allow any
            require_multi_sig: false,
            audit_log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            session_timeout_secs: 3600, // 1 hour
            audit_key,
        }
    }
    
    pub fn with_audit_key(audit_key: String) -> Self {
        Self {
            max_recovery_lamports: 100_000_000_000, // 100 SOL
            allowed_destinations: Vec::new(), // Empty means allow any
            require_multi_sig: false,
            audit_log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            session_timeout_secs: 3600, // 1 hour
            audit_key,
        }
    }
    
    pub fn with_limits(max_recovery_lamports: u64, allowed_destinations: Vec<Pubkey>) -> Self {
        let audit_key = std::env::var("RECOVERY_AUDIT_KEY")
            .unwrap_or_else(|_| {
                // Generate a random key if not configured
                format!("{:x}", rand::random::<u64>())
            });
        
        Self {
            max_recovery_lamports: max_recovery_lamports,
            allowed_destinations,
            require_multi_sig: true,
            audit_log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            session_timeout_secs: 3600,
            audit_key,
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
            FeeStructure::default(),
        )
    }
}
