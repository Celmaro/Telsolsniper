use anyhow::{anyhow, Result};
use solana_sdk::instruction::Instruction;
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::instruction::close_account;

use crate::{
    constants,
    trading::pumpfun::common::{
        get_bonding_curve_pda, get_global_volume_accumulator_pda, get_user_volume_accumulator_pda,
    },
    utils::calc::{
        common::{calculate_with_slippage_buy, calculate_with_slippage_sell},
        pumpfun::{get_buy_token_amount_from_sol_amount, get_sell_sol_amount_from_token_amount},
    },
};

use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, signer::Signer};

use crate::{
    constants::pumpfun::global_constants::FEE_RECIPIENT,
    constants::trade::trade::DEFAULT_SLIPPAGE,
    trading::core::{
        params::{BuyParams, PumpFunParams, SellParams},
        traits::InstructionBuilder,
    },
    trading::pumpfun::common::get_creator_vault_pda,
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

        let bonding_curve = protocol_params.bonding_curve.clone();

        let max_sol_cost = calculate_with_slippage_buy(
            params.sol_amount,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );
        let creator_vault_pda = bonding_curve.get_creator_vault_pda();

        let buy_token_amount = get_buy_token_amount_from_sol_amount(
            bonding_curve.virtual_token_reserves as u128,
            bonding_curve.virtual_sol_reserves as u128,
            bonding_curve.real_token_reserves as u128,
            bonding_curve.creator,
            params.sol_amount,
        );

        let mut instructions = vec![];

        // Create associated token account
        instructions.push(create_associated_token_account(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &constants::pumpfun::accounts::TOKEN_PROGRAM,
        ));

        // Create buy instruction
        instructions.push(buy(
            params.payer.as_ref(),
            &params.mint,
            &bonding_curve.account,
            &creator_vault_pda,
            &FEE_RECIPIENT,
            Buy { _amount: buy_token_amount, _max_sol_cost: max_sol_cost },
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

        let bonding_curve = protocol_params.bonding_curve.clone();

        let token_amount = if let Some(amount) = params.token_amount {
            if amount == 0 {
                return Err(anyhow!("Amount cannot be zero"));
            }
            amount
        } else {
            return Err(anyhow!("Amount token is required"));
        };
        let creator_vault_pda = get_creator_vault_pda(&params.creator).unwrap();
        let ata = get_associated_token_address(&params.payer.pubkey(), &params.mint);

        let sol_amount = get_sell_sol_amount_from_token_amount(
            bonding_curve.virtual_token_reserves as u128,
            bonding_curve.virtual_sol_reserves as u128,
            bonding_curve.creator,
            token_amount,
        );
        let min_sol_output = calculate_with_slippage_sell(
            sol_amount,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );

        let mut instructions = vec![sell(
            params.payer.as_ref(),
            &params.mint,
            &creator_vault_pda,
            &FEE_RECIPIENT,
            Sell { _amount: token_amount, _min_sol_output: min_sol_output },
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

pub struct Buy {
    pub _amount: u64,
    pub _max_sol_cost: u64,
}

impl Buy {
    pub fn data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(8 + 8 + 8);
        data.extend_from_slice(&[102, 6, 61, 18, 1, 218, 235, 234]); // discriminator
        data.extend_from_slice(&self._amount.to_le_bytes());
        data.extend_from_slice(&self._max_sol_cost.to_le_bytes());
        data
    }
}

pub struct Sell {
    pub _amount: u64,
    pub _min_sol_output: u64,
}

impl Sell {
    pub fn data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(8 + 8 + 8);
        data.extend_from_slice(&[51, 230, 133, 164, 1, 127, 131, 173]); // discriminator
        data.extend_from_slice(&self._amount.to_le_bytes());
        data.extend_from_slice(&self._min_sol_output.to_le_bytes());
        data
    }
}

pub fn buy(
    payer: &Keypair,
    mint: &Pubkey,
    bonding_curve_pda: &Pubkey,
    creator_vault_pda: &Pubkey,
    fee_recipient: &Pubkey,
    args: Buy,
) -> Instruction {
    Instruction::new_with_bytes(
        constants::pumpfun::accounts::PUMPFUN,
        &args.data(),
        vec![
            AccountMeta::new_readonly(constants::pumpfun::global_constants::GLOBAL_ACCOUNT, false),
            AccountMeta::new(*fee_recipient, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*bonding_curve_pda, false),
            AccountMeta::new(get_associated_token_address(bonding_curve_pda, mint), false),
            AccountMeta::new(get_associated_token_address(&payer.pubkey(), mint), false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(constants::pumpfun::accounts::SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(constants::pumpfun::accounts::TOKEN_PROGRAM, false),
            AccountMeta::new(*creator_vault_pda, false),
            AccountMeta::new_readonly(constants::pumpfun::accounts::EVENT_AUTHORITY, false),
            AccountMeta::new_readonly(constants::pumpfun::accounts::PUMPFUN, false),
            AccountMeta::new(get_global_volume_accumulator_pda().unwrap(), false),
            AccountMeta::new(get_user_volume_accumulator_pda(&payer.pubkey()).unwrap(), false),
        ],
    )
}

pub fn sell(
    payer: &Keypair,
    mint: &Pubkey,
    creator_vault_pda: &Pubkey,
    fee_recipient: &Pubkey,
    args: Sell,
) -> Instruction {
    let bonding_curve: Pubkey = get_bonding_curve_pda(mint).unwrap();
    Instruction::new_with_bytes(
        constants::pumpfun::accounts::PUMPFUN,
        &args.data(),
        vec![
            AccountMeta::new_readonly(constants::pumpfun::global_constants::GLOBAL_ACCOUNT, false),
            AccountMeta::new(*fee_recipient, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(get_associated_token_address(&bonding_curve, mint), false),
            AccountMeta::new(get_associated_token_address(&payer.pubkey(), mint), false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(constants::pumpfun::accounts::SYSTEM_PROGRAM, false),
            AccountMeta::new(*creator_vault_pda, false),
            AccountMeta::new_readonly(constants::pumpfun::accounts::TOKEN_PROGRAM, false),
            AccountMeta::new_readonly(constants::pumpfun::accounts::EVENT_AUTHORITY, false),
            AccountMeta::new_readonly(constants::pumpfun::accounts::PUMPFUN, false),
            AccountMeta::new(get_global_volume_accumulator_pda().unwrap(), false),
            AccountMeta::new(get_user_volume_accumulator_pda(&payer.pubkey()).unwrap(), false),
        ],
    )
}
