use anyhow::{anyhow, Result};
use solana_sdk::{
    instruction::Instruction, native_token::sol_to_lamports, pubkey::Pubkey, signer::Signer,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::instruction::close_account;
use std::sync::Arc;

use crate::{
    accounts::BondingCurveAccount,
    constants::{self, pumpfun::global_constants::FEE_RECIPIENT, trade_type::SNIPER_BUY},
    instruction,
    pumpfun::common::{
        calculate_with_slippage_buy, get_bonding_curve_account_v2, get_bonding_curve_pda,
        get_buy_token_amount_from_sol_amount, get_creator_vault_pda, init_bonding_curve_account,
    },
    trading::core::{
        constants::DEFAULT_SLIPPAGE_BASIS_POINTS,
        params::{BuyParams, PumpFunParams, SellParams},
        traits::InstructionBuilder,
    },
    PumpFun,
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

        if params.amount_sol == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        // 获取或初始化bonding curve账户
        let bonding_curve = if protocol_params.trade_type == SNIPER_BUY {
            init_bonding_curve_account(
                &params.mint,
                protocol_params.dev_buy_token,
                protocol_params.dev_sol_cost,
                params.creator,
            )
            .await?
        } else {
            protocol_params.bonding_curve.clone().unwrap()
        };

        let max_sol_cost = calculate_with_slippage_buy(
            params.amount_sol,
            params
                .slippage_basis_points
                .unwrap_or(DEFAULT_SLIPPAGE_BASIS_POINTS),
        );
        let creator_vault_pda = bonding_curve.get_creator_vault_pda();

        let mut buy_token_amount =
            get_buy_token_amount_from_sol_amount(&bonding_curve, params.amount_sol);
        if buy_token_amount <= 100 * 1_000_000_u64 {
            buy_token_amount = if max_sol_cost > sol_to_lamports(0.01) {
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
        instructions.push(instruction::buy(
            params.payer.as_ref(),
            &params.mint,
            &bonding_curve.account,
            &creator_vault_pda,
            &FEE_RECIPIENT,
            instruction::Buy {
                _amount: buy_token_amount,
                _max_sol_cost: max_sol_cost,
            },
        ));

        Ok(instructions)
    }

    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>> {
        let amount_token = if let Some(amount) = params.amount_token {
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

        let mut amount_token = amount_token;
        if amount_token > balance_u64 {
            amount_token = balance_u64;
        }

        let mut instructions = vec![instruction::sell(
            params.payer.as_ref(),
            &params.mint,
            &creator_vault_pda,
            &FEE_RECIPIENT,
            instruction::Sell {
                _amount: amount_token,
                _min_sol_output: 1,
            },
        )];

        // 如果卖出全部代币，关闭账户
        if amount_token >= balance_u64 {
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
