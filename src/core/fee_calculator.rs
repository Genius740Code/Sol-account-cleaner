use crate::core::{FeeStructure, WalletInfo, SolanaRecoverError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeCalculation {
    pub total_recoverable_lamports: u64,
    pub fee_lamports: u64,
    pub net_recoverable_lamports: u64,
    pub fee_percentage: f64,
    pub fee_waived: bool,
    pub fee_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFeeCalculation {
    pub total_recoverable_lamports: u64,
    pub total_fee_lamports: u64,
    pub total_net_recoverable_lamports: u64,
    pub wallet_calculations: Vec<FeeCalculation>,
    pub effective_fee_percentage: f64,
}

pub struct FeeCalculator;

impl FeeCalculator {
    pub fn calculate_fee(
        recoverable_lamports: u64,
        fee_structure: &FeeStructure,
    ) -> FeeCalculation {
        // Check if fee should be waived
        if let Some(waive_threshold) = fee_structure.waive_below_lamports {
            if recoverable_lamports <= waive_threshold {
                return FeeCalculation {
                    total_recoverable_lamports: recoverable_lamports,
                    fee_lamports: 0,
                    net_recoverable_lamports: recoverable_lamports,
                    fee_percentage: 0.0,
                    fee_waived: true,
                    fee_reason: Some("Below waiver threshold".to_string()),
                };
            }
        }

        // Calculate fee based on percentage
        let fee_lamports = ((recoverable_lamports as f64 * fee_structure.percentage) as u64)
            .max(fee_structure.minimum_lamports);

        // Apply maximum fee cap if specified
        let final_fee_lamports = if let Some(max_fee) = fee_structure.maximum_lamports {
            fee_lamports.min(max_fee)
        } else {
            fee_lamports
        };

        // Ensure fee doesn't exceed recoverable amount
        let final_fee_lamports = final_fee_lamports.min(recoverable_lamports);

        let net_recoverable = recoverable_lamports.saturating_sub(final_fee_lamports);
        let effective_percentage = if recoverable_lamports > 0 {
            final_fee_lamports as f64 / recoverable_lamports as f64
        } else {
            0.0
        };

        FeeCalculation {
            total_recoverable_lamports: recoverable_lamports,
            fee_lamports: final_fee_lamports,
            net_recoverable_lamports: net_recoverable,
            fee_percentage: effective_percentage,
            fee_waived: false,
            fee_reason: None,
        }
    }

    pub fn calculate_wallet_fee(
        wallet_info: &WalletInfo,
        fee_structure: &FeeStructure,
    ) -> FeeCalculation {
        Self::calculate_fee(wallet_info.recoverable_lamports, fee_structure)
    }

    pub fn calculate_batch_fee(
        wallet_infos: &[WalletInfo],
        fee_structure: &FeeStructure,
    ) -> BatchFeeCalculation {
        let wallet_calculations: Vec<FeeCalculation> = wallet_infos
            .iter()
            .map(|wallet| Self::calculate_wallet_fee(wallet, fee_structure))
            .collect();

        let total_recoverable_lamports: u64 = wallet_calculations
            .iter()
            .map(|calc| calc.total_recoverable_lamports)
            .sum();

        let total_fee_lamports: u64 = wallet_calculations
            .iter()
            .map(|calc| calc.fee_lamports)
            .sum();

        let total_net_recoverable_lamports = total_recoverable_lamports.saturating_sub(total_fee_lamports);

        let effective_fee_percentage = if total_recoverable_lamports > 0 {
            total_fee_lamports as f64 / total_recoverable_lamports as f64
        } else {
            0.0
        };

        BatchFeeCalculation {
            total_recoverable_lamports,
            total_fee_lamports,
            total_net_recoverable_lamports,
            wallet_calculations,
            effective_fee_percentage,
        }
    }

    pub fn validate_fee_structure(fee_structure: &FeeStructure) -> Result<(), SolanaRecoverError> {
        if fee_structure.percentage < 0.0 || fee_structure.percentage > 1.0 {
            return Err(SolanaRecoverError::ValidationError(
                "Fee percentage must be between 0.0 and 1.0".to_string()
            ));
        }

        if fee_structure.minimum_lamports == 0 {
            return Err(SolanaRecoverError::ValidationError(
                "Minimum fee must be greater than 0".to_string()
            ));
        }

        if let Some(max_fee) = fee_structure.maximum_lamports {
            if max_fee < fee_structure.minimum_lamports {
                return Err(SolanaRecoverError::ValidationError(
                    "Maximum fee cannot be less than minimum fee".to_string()
                ));
            }
        }

        if let Some(waive_threshold) = fee_structure.waive_below_lamports {
            if waive_threshold == 0 {
                return Err(SolanaRecoverError::ValidationError(
                    "Waive threshold must be greater than 0".to_string()
                ));
            }
        }

        Ok(())
    }

    pub fn estimate_fee_for_amount(
        amount_lamports: u64,
        fee_percentage: f64,
    ) -> u64 {
        ((amount_lamports as f64 * fee_percentage) as u64).max(1_000_000) // Default minimum
    }

    pub fn format_fee_calculation(calculation: &FeeCalculation) -> String {
        format!(
            "Recoverable: {:.9} SOL, Fee: {:.9} SOL ({:.1}%), Net: {:.9} SOL{}",
            calculation.total_recoverable_lamports as f64 / 1_000_000_000.0,
            calculation.fee_lamports as f64 / 1_000_000_000.0,
            calculation.fee_percentage * 100.0,
            calculation.net_recoverable_lamports as f64 / 1_000_000_000.0,
            if calculation.fee_waived { " (WAIVED)" } else { "" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_calculation_standard() {
        let fee_structure = FeeStructure::default();
        let recoverable = 100_000_000; // 0.1 SOL

        let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

        assert_eq!(calculation.total_recoverable_lamports, recoverable);
        assert_eq!(calculation.fee_lamports, 15_000_000); // 15% of 0.1 SOL
        assert_eq!(calculation.net_recoverable_lamports, 85_000_000);
        assert!((calculation.fee_percentage - 0.15).abs() < f64::EPSILON);
        assert!(!calculation.fee_waived);
    }

    #[test]
    fn test_fee_calculation_waived() {
        let mut fee_structure = FeeStructure::default();
        fee_structure.waive_below_lamports = Some(50_000_000); // 0.05 SOL
        let recoverable = 30_000_000; // 0.03 SOL

        let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

        assert_eq!(calculation.total_recoverable_lamports, recoverable);
        assert_eq!(calculation.fee_lamports, 0);
        assert_eq!(calculation.net_recoverable_lamports, recoverable);
        assert_eq!(calculation.fee_percentage, 0.0);
        assert!(calculation.fee_waived);
        assert!(calculation.fee_reason.is_some());
    }

    #[test]
    fn test_fee_calculation_minimum() {
        let fee_structure = FeeStructure::default();
        let recoverable = 1_000_000; // 0.001 SOL (very small)

        let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

        assert_eq!(calculation.fee_lamports, fee_structure.minimum_lamports);
    }

    #[test]
    fn test_fee_calculation_maximum() {
        let mut fee_structure = FeeStructure::default();
        fee_structure.maximum_lamports = Some(5_000_000); // 0.005 SOL max
        let recoverable = 100_000_000; // 0.1 SOL

        let calculation = FeeCalculator::calculate_fee(recoverable, &fee_structure);

        assert_eq!(calculation.fee_lamports, 5_000_000); // Capped at maximum
    }

    #[test]
    fn test_batch_fee_calculation() {
        let fee_structure = FeeStructure::default();
        let wallet_infos = vec![
            WalletInfo {
                address: "wallet1".to_string(),
                pubkey: solana_sdk::pubkey::Pubkey::default(),
                total_accounts: 10,
                empty_accounts: 5,
                recoverable_lamports: 100_000_000,
                recoverable_sol: 0.1,
                empty_account_addresses: vec![],
                scan_time_ms: 1000,
            },
            WalletInfo {
                address: "wallet2".to_string(),
                pubkey: solana_sdk::pubkey::Pubkey::default(),
                total_accounts: 8,
                empty_accounts: 3,
                recoverable_lamports: 50_000_000,
                recoverable_sol: 0.05,
                empty_account_addresses: vec![],
                scan_time_ms: 800,
            },
        ];

        let batch_calc = FeeCalculator::calculate_batch_fee(&wallet_infos, &fee_structure);

        assert_eq!(batch_calc.total_recoverable_lamports, 150_000_000);
        assert_eq!(batch_calc.total_fee_lamports, 22_500_000); // 15% of total
        assert_eq!(batch_calc.total_net_recoverable_lamports, 127_500_000);
        assert_eq!(batch_calc.wallet_calculations.len(), 2);
    }

    #[test]
    fn test_fee_structure_validation() {
        let mut fee_structure = FeeStructure::default();

        // Valid structure
        assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_ok());

        // Invalid percentage
        fee_structure.percentage = 1.5;
        assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_err());

        // Reset and test minimum
        fee_structure.percentage = 0.15;
        fee_structure.minimum_lamports = 0;
        assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_err());

        // Reset and test maximum < minimum
        fee_structure.minimum_lamports = 1_000_000;
        fee_structure.maximum_lamports = Some(500_000);
        assert!(FeeCalculator::validate_fee_structure(&fee_structure).is_err());
    }
}
