use anyhow::{anyhow, Result};
use solana_sdk::{instruction::Instruction, native_token::sol_str_to_lamports};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::instruction::close_account;

use crate::{
    constants,
    trading::pumpfun::common::{
        get_bonding_curve_pda, get_global_volume_accumulator_pda, get_user_volume_accumulator_pda,
    },
};

use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, signer::Signer};

use crate::{
    constants::pumpfun::global_constants::FEE_RECIPIENT,
    constants::trade::trade::DEFAULT_SLIPPAGE,
    trading::common::utils::calculate_with_slippage_buy,
    trading::core::{
        params::{BuyParams, PumpFunParams, SellParams},
        traits::InstructionBuilder,
    },
    trading::pumpfun::common::{get_buy_token_amount_from_sol_amount, get_creator_vault_pda},
};

/// PumpFun协议的指令构建器
pub struct PumpFunInstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for PumpFunInstructionBuilder {
    async fn build_buy_instructions(&self, params: &BuyParams) -> Result<Vec<Instruction>> {
        // 获取PumpFun特定参数
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpFun"))?;

        if params.sol_amount == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        let bonding_curve = if protocol_params.bonding_curve.is_some() {
            protocol_params.bonding_curve.clone().unwrap()
        } else {
            return Err(anyhow!("Bonding curve not found"));
        };

        let max_sol_cost = calculate_with_slippage_buy(
            params.sol_amount,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );
        let creator_vault_pda = bonding_curve.get_creator_vault_pda();

        let mut buy_token_amount =
            get_buy_token_amount_from_sol_amount(&bonding_curve, params.sol_amount);
        if buy_token_amount <= 100 * 1_000_000_u64 {
            buy_token_amount = if max_sol_cost > sol_str_to_lamports("0.01").unwrap_or(0) {
                25547619 * 1_000_000_u64
            } else {
                255476 * 1_000_000_u64
            };
        }

        let mut instructions = vec![];

        // 创建关联代币账户
        instructions.push(create_associated_token_account(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &constants::pumpfun::accounts::TOKEN_PROGRAM,
        ));

        // 创建买入指令
        instructions.push(buy(
            params.payer.as_ref(),
            &params.mint,
            &bonding_curve.account,
            &creator_vault_pda,
            &FEE_RECIPIENT,
            Buy {
                _amount: buy_token_amount,
                _max_sol_cost: max_sol_cost,
            },
        ));

        Ok(instructions)
    }

    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>> {
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

        // 获取代币余额
        let balance_u64 = if let Some(rpc) = &params.rpc {
            let balance = rpc.get_token_account_balance(&ata).await?;
            balance
                .amount
                .parse::<u64>()
                .map_err(|_| anyhow!("Failed to parse token balance"))?
        } else {
            return Err(anyhow!("RPC client is required to get token balance"));
        };

        let mut token_amount = token_amount;
        if token_amount > balance_u64 {
            token_amount = balance_u64;
        }

        let mut instructions = vec![sell(
            params.payer.as_ref(),
            &params.mint,
            &creator_vault_pda,
            &FEE_RECIPIENT,
            Sell {
                _amount: token_amount,
                _min_sol_output: 1,
            },
        )];

        // 如果卖出全部代币，关闭账户
        if token_amount >= balance_u64 {
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
        ],
    )
}
