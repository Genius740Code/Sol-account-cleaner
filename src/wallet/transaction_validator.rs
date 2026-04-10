use crate::core::{Result, SolanaRecoverError};
use solana_sdk::{
    pubkey::Pubkey,
    transaction::Transaction,
    message::Message,
    commitment_config::CommitmentConfig,
};
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct TransactionValidator {
    allowed_destinations: HashSet<Pubkey>,
    max_signers: usize,
    max_instructions: usize,
    max_lamports_transfer: u64,
    require_simulation: bool,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub simulation_result: Option<SimulationResult>,
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub error: Option<String>,
    pub units_consumed: u64,
    pub fee: u64,
    pub account_changes: Vec<AccountChange>,
}

#[derive(Debug, Clone)]
pub struct AccountChange {
    pub pubkey: Pubkey,
    pub lamports_before: u64,
    pub lamports_after: u64,
    pub data_changed: bool,
}

impl TransactionValidator {
    pub fn new() -> Self {
        Self {
            allowed_destinations: HashSet::new(),
            max_signers: 5,
            max_instructions: 20,
            max_lamports_transfer: 1_000_000_000_000, // 1000 SOL
            require_simulation: true,
        }
    }

    pub fn with_allowed_destinations(mut self, destinations: Vec<Pubkey>) -> Self {
        self.allowed_destinations = destinations.into_iter().collect();
        self
    }

    pub fn with_limits(mut self, max_signers: usize, max_instructions: usize, max_lamports: u64) -> Self {
        self.max_signers = max_signers;
        self.max_instructions = max_instructions;
        self.max_lamports_transfer = max_lamports;
        self
    }

    pub fn require_simulation(mut self, require: bool) -> Self {
        self.require_simulation = require;
        self
    }

    pub async fn validate_transaction(&self, transaction: &[u8], rpc_client: &solana_client::rpc_client::RpcClient) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            is_valid: true,
            warnings: Vec::new(),
            errors: Vec::new(),
            simulation_result: None,
        };

        // Parse transaction
        let tx = match bincode::deserialize::<Transaction>(transaction) {
            Ok(tx) => tx,
            Err(e) => {
                result.errors.push(format!("Failed to deserialize transaction: {}", e));
                result.is_valid = false;
                return Ok(result);
            }
        };

        // Basic structure validation
        self.validate_structure(&tx, &mut result);

        // Security validation
        self.validate_security(&tx, &mut result);

        // Account validation
        self.validate_accounts(&tx, &mut result);

        // Simulation validation
        if self.require_simulation {
            match self.simulate_transaction(&tx, rpc_client).await {
                Ok(sim_result) => {
                    result.simulation_result = Some(sim_result.clone());
                    if !sim_result.success {
                        result.errors.push(format!("Transaction simulation failed: {:?}", sim_result.error));
                        result.is_valid = false;
                    }
                    self.validate_simulation_results(&sim_result, &mut result);
                }
                Err(e) => {
                    result.warnings.push(format!("Failed to simulate transaction: {}", e));
                    if self.require_simulation {
                        result.errors.push("Transaction simulation is required but failed".to_string());
                        result.is_valid = false;
                    }
                }
            }
        }

        Ok(result)
    }

    fn validate_structure(&self, tx: &Transaction, result: &mut ValidationResult) {
        // Check number of signers
        if tx.signatures.len() > self.max_signers {
            result.errors.push(format!(
                "Too many signers: {} (max: {})",
                tx.signatures.len(),
                self.max_signers
            ));
            result.is_valid = false;
        }

        // Check number of instructions
        if tx.message.instructions.len() > self.max_instructions {
            result.errors.push(format!(
                "Too many instructions: {} (max: {})",
                tx.message.instructions.len(),
                self.max_instructions
            ));
            result.is_valid = false;
        }

        // Check for empty instructions
        if tx.message.instructions.is_empty() {
            result.errors.push("Transaction has no instructions".to_string());
            result.is_valid = false;
        }

        // Verify recent blockhash
        if tx.message.recent_blockhash == solana_sdk::hash::Hash::default() {
            result.errors.push("Invalid recent blockhash (all zeros)".to_string());
            result.is_valid = false;
        }
    }

    fn validate_security(&self, tx: &Transaction, result: &mut ValidationResult) {
        // Check for suspicious instructions
        for (i, instruction) in tx.message.instructions.iter().enumerate() {
            if let Some(program_id) = tx.message.account_keys.get(instruction.program_id_index as usize) {
                // Check for known suspicious program IDs
                if self.is_suspicious_program(program_id) {
                    result.errors.push(format!(
                        "Suspicious program ID at instruction {}: {}",
                        i, program_id
                    ));
                    result.is_valid = false;
                }

                // Check for large transfers
                if self.is_large_transfer(instruction, &tx.message) {
                    result.warnings.push(format!(
                        "Large transfer detected at instruction {}",
                        i
                    ));
                }
            }
        }

        // Validate destination addresses if whitelist is configured
        if !self.allowed_destinations.is_empty() {
            for account_key in &tx.message.account_keys {
                if !self.allowed_destinations.contains(account_key) && 
                   !self.is_system_program(account_key) && 
                   !self.is_token_program(account_key) {
                    result.errors.push(format!(
                        "Destination address not in whitelist: {}",
                        account_key
                    ));
                    result.is_valid = false;
                }
            }
        }
    }

    fn validate_accounts(&self, tx: &Transaction, result: &mut ValidationResult) {
        // Check for duplicate account keys
        let mut seen_accounts = HashSet::new();
        for (i, account_key) in tx.message.account_keys.iter().enumerate() {
            if seen_accounts.contains(account_key) {
                result.warnings.push(format!(
                    "Duplicate account key at index {}: {}",
                    i, account_key
                ));
            }
            seen_accounts.insert(account_key);
        }

        // Validate account indexes in instructions
        for (i, instruction) in tx.message.instructions.iter().enumerate() {
            if instruction.program_id_index as usize >= tx.message.account_keys.len() {
                result.errors.push(format!(
                    "Invalid program_id_index {} in instruction {}",
                    instruction.program_id_index, i
                ));
                result.is_valid = false;
            }

            for (j, account_index) in instruction.accounts.iter().enumerate() {
                if *account_index as usize >= tx.message.account_keys.len() {
                    result.errors.push(format!(
                        "Invalid account_index {} at position {} in instruction {}",
                        account_index, j, i
                    ));
                    result.is_valid = false;
                }
            }
        }
    }

    async fn simulate_transaction(&self, tx: &Transaction, rpc_client: &solana_client::rpc_client::RpcClient) -> Result<SimulationResult> {
        let simulation = rpc_client
            .simulate_transaction_with_config(
                tx,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: true,
                    commitment: Some(CommitmentConfig::processed()),
                    encoding: None,
                    accounts: None,
                    min_context_slot: None,
                    inner_instructions: Some(false).is_some(),
                },
            )
            .map_err(|e| SolanaRecoverError::RpcClientError(e.to_string()))?;

        let mut account_changes = Vec::new();
        
        let value = simulation;
        
        if let Some(accounts) = value.value.accounts {
            for account in accounts {
                if let Some(account) = account {
                    // For now, we'll skip the pubkey parsing since UiAccount doesn't have pubkey field
                    // In a real implementation, we'd need to get the account key from elsewhere
                    account_changes.push(AccountChange {
                        pubkey: Pubkey::new_unique(), // Placeholder
                        lamports_before: account.lamports,
                        lamports_after: account.lamports,
                        data_changed: false, // Would need more detailed simulation data
                    });
                }
            }
        }

        Ok(SimulationResult {
            success: value.value.err.is_none(),
            error: value.value.err.map(|e| e.to_string()),
            units_consumed: value.value.units_consumed.unwrap_or(0),
            fee: 0, // Fee calculation would need more detailed simulation
            account_changes,
        })
    }

    fn validate_simulation_results(&self, sim_result: &SimulationResult, result: &mut ValidationResult) {
        // Check for excessive compute units
        if sim_result.units_consumed > 1_400_000 {
            result.warnings.push(format!(
                "High compute unit consumption: {} (max recommended: 1.4M)",
                sim_result.units_consumed
            ));
        }

        // Check for excessive fees
        if sim_result.fee > 10_000_000 {
            result.warnings.push(format!(
                "High transaction fee: {} lamports",
                sim_result.fee
            ));
        }

        // Check for unexpected account changes
        for change in &sim_result.account_changes {
            if change.lamports_after > change.lamports_before + self.max_lamports_transfer {
                result.warnings.push(format!(
                    "Large balance increase detected for account {}: {} -> {}",
                    change.pubkey, change.lamports_before, change.lamports_after
                ));
            }
        }
    }

    fn is_suspicious_program(&self, _program_id: &Pubkey) -> bool {
        // Add known suspicious program IDs
        // This is a placeholder - in production, maintain a list of known malicious programs
        false
    }

    fn is_large_transfer(&self, _instruction: &solana_sdk::instruction::CompiledInstruction, _message: &Message) -> bool {
        // Check if this looks like a large transfer instruction
        // This is a simplified check - in production, decode the instruction properly
        false
    }

    fn is_system_program(&self, pubkey: &Pubkey) -> bool {
        pubkey == &solana_sdk::system_program::id()
    }

    fn is_token_program(&self, pubkey: &Pubkey) -> bool {
        pubkey == &solana_sdk::system_program::id() || pubkey == &spl_token::id() || pubkey == &spl_token_2022::id()
    }
}

impl Default for TransactionValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{signature::Keypair, transaction::Transaction, message::Message};

    #[tokio::test]
    async fn test_transaction_validation() {
        let validator = TransactionValidator::new();
        let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");
        
        // Create a simple test transaction
        let keypair = Keypair::new();
        let message = Message::new(&[], Some(&keypair.pubkey()));
        let tx = Transaction::new_unsigned(message);
        
        // Serialize for validation
        let serialized = bincode::serialize(&tx).unwrap();
        
        let result = validator.validate_transaction(&serialized, &rpc_client).await;
        assert!(result.is_ok());
        
        let validation = result.unwrap();
        // Should fail because no recent blockhash is set
        assert!(!validation.is_valid);
        assert!(validation.errors.iter().any(|e| e.contains("Invalid recent blockhash")));
    }
}
