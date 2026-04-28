use crate::core::{Result, SolanaRecoverError};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

/// Secure program ID configuration with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramIds {
    pub openbook_v2: Pubkey,
    pub serum_dex: Pubkey,
    pub token_program: Pubkey,
    pub token_2022_program: Pubkey,
}

impl Default for ProgramIds {
    fn default() -> Self {
        Self {
            openbook_v2: Pubkey::from_str("opnb2vDkSQsqmY24zQ4DDEZf1V3oEisPZ5bEErLNRsA")
                .expect("Invalid default OpenBook V2 program ID"),
            serum_dex: Pubkey::from_str("srmqPvvk92GzrcCbKgSGx3mFHTEQuoE3jUuAM6gEKrP")
                .expect("Invalid default Serum DEX program ID"),
            token_program: spl_token::id(),
            token_2022_program: spl_token_2022::id(),
        }
    }
}


impl ProgramIds {
    /// Create new ProgramIds with validation
    pub fn new(
        openbook_v2: &str,
        serum_dex: &str,
        token_program: Option<&str>,
        token_2022_program: Option<&str>,
    ) -> Result<Self> {
        let openbook_v2_pubkey = Pubkey::from_str(openbook_v2)
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid OpenBook V2 program ID".to_string()))?;
        
        let serum_dex_pubkey = Pubkey::from_str(serum_dex)
            .map_err(|_| SolanaRecoverError::InvalidInput("Invalid Serum DEX program ID".to_string()))?;
        
        let token_program_pubkey = match token_program {
            Some(id) => Pubkey::from_str(id)
                .map_err(|_| SolanaRecoverError::InvalidInput("Invalid Token program ID".to_string()))?,
            None => spl_token::id(),
        };
        
        let token_2022_program_pubkey = match token_2022_program {
            Some(id) => Pubkey::from_str(id)
                .map_err(|_| SolanaRecoverError::InvalidInput("Invalid Token-2022 program ID".to_string()))?,
            None => spl_token_2022::id(),
        };
        
        let program_ids = Self {
            openbook_v2: openbook_v2_pubkey,
            serum_dex: serum_dex_pubkey,
            token_program: token_program_pubkey,
            token_2022_program: token_2022_program_pubkey,
        };
        
        // Validate the program IDs
        program_ids.validate()?;
        
        Ok(program_ids)
    }
    
    /// Validate program IDs against known good values
    pub fn validate(&self) -> Result<()> {
        // Known good program IDs for mainnet
        const KNOWN_OPENBOOK_V2: &str = "opnb2vDkSQsqmY24zQ4DDEZf1V3oEisPZ5bEErLNRsA";
        const KNOWN_SERUM_DEX: &str = "srmqPvvk92GzrcCbKgSGx3mFHTEQuoE3jUuAM6gEKrP";
        const KNOWN_TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        const KNOWN_TOKEN_2022_PROGRAM: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBwCXr2MCTc";
        
        // Validate OpenBook V2
        if self.openbook_v2.to_string() != KNOWN_OPENBOOK_V2 {
            return Err(SolanaRecoverError::SecurityError(
                format!("OpenBook V2 program ID mismatch. Expected: {}, Got: {}", 
                    KNOWN_OPENBOOK_V2, self.openbook_v2)
            ));
        }
        
        // Validate Serum DEX
        if self.serum_dex.to_string() != KNOWN_SERUM_DEX {
            return Err(SolanaRecoverError::SecurityError(
                format!("Serum DEX program ID mismatch. Expected: {}, Got: {}", 
                    KNOWN_SERUM_DEX, self.serum_dex)
            ));
        }
        
        // Validate Token program
        if self.token_program.to_string() != KNOWN_TOKEN_PROGRAM {
            return Err(SolanaRecoverError::SecurityError(
                format!("Token program ID mismatch. Expected: {}, Got: {}", 
                    KNOWN_TOKEN_PROGRAM, self.token_program)
            ));
        }
        
        // Validate Token-2022 program
        if self.token_2022_program.to_string() != KNOWN_TOKEN_2022_PROGRAM {
            return Err(SolanaRecoverError::SecurityError(
                format!("Token-2022 program ID mismatch. Expected: {}, Got: {}", 
                    KNOWN_TOKEN_2022_PROGRAM, self.token_2022_program)
            ));
        }
        
        Ok(())
    }
    
    /// Validate program IDs for devnet (allows different IDs)
    pub fn validate_devnet(&self) -> Result<()> {
        // For devnet, we just validate that they are valid pubkeys
        // The actual validation should be done at runtime by checking on-chain programs
        Ok(())
    }
    
    /// Get program IDs for specific network
    pub fn for_network(network: &str) -> Result<Self> {
        match network {
            "mainnet" | "mainnet-beta" => Ok(ProgramIds::default()),
            "devnet" => {
                // Devnet uses different program IDs - these should be configured properly
                Ok(ProgramIds::default()) // For now, use defaults but add proper devnet config
            },
            "testnet" => {
                // Testnet configuration
                Ok(ProgramIds::default()) // For now, use defaults but add proper testnet config
            },
            _ => Err(SolanaRecoverError::InvalidInput(
                format!("Unsupported network: {}", network)
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_program_ids() {
        let program_ids = ProgramIds::default();
        assert!(program_ids.validate().is_ok());
    }
    
    #[test]
    fn test_invalid_program_ids() {
        let result = ProgramIds::new(
            "invalid_pubkey",
            "srmqPvvk92GzrcCbKgSGx3mFHTEQuoE3jUuAM6gEKrP",
            None,
            None,
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_program_id_validation() {
        let program_ids = ProgramIds::default();
        
        // Test with valid program IDs
        assert!(program_ids.validate().is_ok());
        
        // Test with invalid OpenBook ID
        let mut invalid_ids = program_ids.clone();
        invalid_ids.openbook_v2 = Pubkey::new_unique();
        assert!(invalid_ids.validate().is_err());
    }
}
