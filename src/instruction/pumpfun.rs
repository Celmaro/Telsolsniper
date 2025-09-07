use anyhow::{anyhow, Result};
use solana_sdk::{instruction::Instruction, signer::Signer};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::instruction::close_account;

use crate::{
    instruction::utils::pumpfun::{
        accounts, get_bonding_curve_pda, get_creator_vault_pda, get_fee_config_pda,
        get_global_volume_accumulator_pda, get_user_volume_accumulator_pda,
        global_constants::{self, FEE_RECIPIENT},
    },
    utils::calc::{
        common::{calculate_with_slippage_buy, calculate_with_slippage_sell},
        pumpfun::{get_buy_token_amount_from_sol_amount, get_sell_sol_amount_from_token_amount},
    },
};

use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};

use crate::{
    constants::trade::trade::DEFAULT_SLIPPAGE,
    trading::core::{
        params::{BuyParams, PumpFunParams, SellParams},
        traits::InstructionBuilder,
    },
};

/// Instruction builder for PumpFun protocol
pub struct PumpFunInstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for PumpFunInstructionBuilder {
    async fn build_buy_instructions(&self, params: &BuyParams) -> Result<Vec<Instruction>> {
        // Get PumpFun specific parameters
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpFun"))?;

        if params.sol_amount == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        let bonding_curve = &protocol_params.bonding_curve;

        let max_sol_cost = calculate_with_slippage_buy(
            params.sol_amount,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );
        let creator_vault_pda = protocol_params.creator_vault;

        // Optimize creator lookup - avoid PDA calculation if not default
        let creator = if creator_vault_pda == Pubkey::default() {
            Pubkey::default()
        } else {
            // Fast check against cached default creator vault
            static DEFAULT_CREATOR_VAULT: std::sync::LazyLock<Option<Pubkey>> =
                std::sync::LazyLock::new(|| get_creator_vault_pda(&Pubkey::default()));

            if Some(creator_vault_pda) == *DEFAULT_CREATOR_VAULT {
                Pubkey::default()
            } else {
                creator_vault_pda
            }
        };

        let buy_token_amount = get_buy_token_amount_from_sol_amount(
            bonding_curve.virtual_token_reserves as u128,
            bonding_curve.virtual_sol_reserves as u128,
            bonding_curve.real_token_reserves as u128,
            creator,
            params.sol_amount,
        );

        let mut instructions = Vec::with_capacity(2);

        // Create associated token account
        instructions.push(create_associated_token_account(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &accounts::TOKEN_PROGRAM,
        ));

        // Create buy instruction data
        let mut buy_data = Vec::with_capacity(8 + 8 + 8);
        buy_data.extend_from_slice(&[102, 6, 61, 18, 1, 218, 235, 234]); // discriminator
        buy_data.extend_from_slice(&buy_token_amount.to_le_bytes());
        buy_data.extend_from_slice(&max_sol_cost.to_le_bytes());

        // Create buy instruction
        instructions.push(Instruction::new_with_bytes(
            accounts::PUMPFUN,
            &buy_data,
            vec![
                AccountMeta::new_readonly(global_constants::GLOBAL_ACCOUNT, false),
                AccountMeta::new(FEE_RECIPIENT, false),
                AccountMeta::new_readonly(params.mint, false),
                AccountMeta::new(bonding_curve.account, false),
                AccountMeta::new(
                    get_associated_token_address(&bonding_curve.account, &params.mint),
                    false,
                ),
                AccountMeta::new(
                    get_associated_token_address(&params.payer.pubkey(), &params.mint),
                    false,
                ),
                AccountMeta::new(params.payer.pubkey(), true),
                AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false),
                AccountMeta::new(creator_vault_pda, false),
                AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(accounts::PUMPFUN, false),
                AccountMeta::new(get_global_volume_accumulator_pda().unwrap(), false),
                AccountMeta::new(
                    get_user_volume_accumulator_pda(&params.payer.pubkey()).unwrap(),
                    false,
                ),
                AccountMeta::new_readonly(get_fee_config_pda().unwrap(), false),
                AccountMeta::new_readonly(accounts::FEE_PROGRAM, false),
            ],
        ));

        Ok(instructions)
    }

    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>> {
        // Get PumpFun specific parameters
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpFun"))?;

        let bonding_curve = &protocol_params.bonding_curve;

        let token_amount = if let Some(amount) = params.token_amount {
            if amount == 0 {
                return Err(anyhow!("Amount cannot be zero"));
            }
            amount
        } else {
            return Err(anyhow!("Amount token is required"));
        };
        let creator_vault_pda = protocol_params.creator_vault;
        let ata = get_associated_token_address(&params.payer.pubkey(), &params.mint);

        let mut creator = Pubkey::default();
        if let Some(default_creator_ata) = get_creator_vault_pda(&creator) {
            if default_creator_ata != creator_vault_pda {
                creator = creator_vault_pda;
            }
        }

        let sol_amount = get_sell_sol_amount_from_token_amount(
            bonding_curve.virtual_token_reserves as u128,
            bonding_curve.virtual_sol_reserves as u128,
            creator,
            token_amount,
        );
        let min_sol_output = calculate_with_slippage_sell(
            sol_amount,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );

        // Create sell instruction data
        let mut sell_data = Vec::with_capacity(8 + 8 + 8);
        sell_data.extend_from_slice(&[51, 230, 133, 164, 1, 127, 131, 173]); // discriminator
        sell_data.extend_from_slice(&token_amount.to_le_bytes());
        sell_data.extend_from_slice(&min_sol_output.to_le_bytes());

        let bonding_curve = get_bonding_curve_pda(&params.mint).unwrap();

        // Create sell instruction
        let mut instructions = vec![Instruction::new_with_bytes(
            accounts::PUMPFUN,
            &sell_data,
            vec![
                AccountMeta::new_readonly(global_constants::GLOBAL_ACCOUNT, false),
                AccountMeta::new(FEE_RECIPIENT, false),
                AccountMeta::new_readonly(params.mint, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(get_associated_token_address(&bonding_curve, &params.mint), false),
                AccountMeta::new(
                    get_associated_token_address(&params.payer.pubkey(), &params.mint),
                    false,
                ),
                AccountMeta::new(params.payer.pubkey(), true),
                AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false),
                AccountMeta::new(creator_vault_pda, false),
                AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false),
                AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(accounts::PUMPFUN, false),
                AccountMeta::new_readonly(get_fee_config_pda().unwrap(), false),
                AccountMeta::new_readonly(accounts::FEE_PROGRAM, false),
            ],
        )];

        // If selling all tokens, close the account
        if protocol_params.close_token_account_when_sell.unwrap_or(false) {
            instructions.push(close_account(
                &spl_token::ID,
                &ata,
                &params.payer.pubkey(),
                &params.payer.pubkey(),
                &[&params.payer.pubkey()],
            )?);
        }

        Ok(instructions)
    }
}
