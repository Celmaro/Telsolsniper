use anyhow::{anyhow, Result};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signer::Signer};
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use std::sync::Arc;

use crate::{
    constants::pumpswap::{
        accounts, trade::DEFAULT_SLIPPAGE, BUY_DISCRIMINATOR, SELL_DISCRIMINATOR,
    },
    pumpswap::common::{
        calculate_with_slippage_buy, calculate_with_slippage_sell, coin_creator_vault_ata,
        coin_creator_vault_authority, find_pool, get_buy_token_amount, get_sell_sol_amount,
        get_token_balance,
    },
    trading::core::{
        constants::DEFAULT_SLIPPAGE_BASIS_POINTS,
        params::{BuyParams, PumpSwapParams, SellParams},
        traits::InstructionBuilder,
    },
};

/// PumpSwap协议的指令构建器
pub struct PumpSwapInstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for PumpSwapInstructionBuilder {
    async fn build_buy_instructions(&self, params: &BuyParams) -> Result<Vec<Instruction>> {
        // 获取PumpSwap特定参数
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpSwap"))?;

        if params.amount_sol == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        // 根据是否提供了账户信息来构建指令
        match (
            &protocol_params.pool,
            &protocol_params.pool_base_token_account,
            &protocol_params.pool_quote_token_account,
            &protocol_params.user_base_token_account,
            &protocol_params.user_quote_token_account,
        ) {
            (
                Some(pool),
                Some(pool_base_token_account),
                Some(pool_quote_token_account),
                Some(user_base_token_account),
                Some(user_quote_token_account),
            ) => {
                self.build_buy_instructions_with_accounts(
                    params,
                    *pool,
                    *pool_base_token_account,
                    *pool_quote_token_account,
                    *user_base_token_account,
                    *user_quote_token_account,
                )
                .await
            }
            _ => self.build_buy_instructions_auto_discover(params).await,
        }
    }

    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>> {
        // 获取PumpSwap特定参数
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpSwap"))?;

        // 根据是否提供了账户信息来构建指令
        match (
            &protocol_params.pool,
            &protocol_params.pool_base_token_account,
            &protocol_params.pool_quote_token_account,
            &protocol_params.user_base_token_account,
            &protocol_params.user_quote_token_account,
        ) {
            (
                Some(pool),
                Some(pool_base_token_account),
                Some(pool_quote_token_account),
                Some(user_base_token_account),
                Some(user_quote_token_account),
            ) => {
                self.build_sell_instructions_with_accounts(
                    params,
                    *pool,
                    *pool_base_token_account,
                    *pool_quote_token_account,
                    *user_base_token_account,
                    *user_quote_token_account,
                )
                .await
            }
            _ => self.build_sell_instructions_auto_discover(params).await,
        }
    }
}

impl PumpSwapInstructionBuilder {
    /// 自动发现池和账户信息并构建买入指令
    async fn build_buy_instructions_auto_discover(
        &self,
        params: &BuyParams,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        // 查找池
        let pool = find_pool(rpc.as_ref(), &params.mint).await?;

        // 创建用户代币账户
        let user_base_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &params.mint,
        );
        let user_quote_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );

        // 获取池的代币账户
        let pool_base_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &params.mint,
                &accounts::TOKEN_PROGRAM,
            );

        let pool_quote_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &accounts::WSOL_TOKEN_ACCOUNT,
                &accounts::TOKEN_PROGRAM,
            );

        self.build_buy_instructions_with_accounts(
            params,
            pool,
            pool_base_token_account,
            pool_quote_token_account,
            user_base_token_account,
            user_quote_token_account,
        )
        .await
    }

    /// 自动发现池和账户信息并构建卖出指令
    async fn build_sell_instructions_auto_discover(
        &self,
        params: &SellParams,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();

        // 查找池
        let pool = find_pool(rpc.as_ref(), &params.mint).await?;

        // 创建用户代币账户
        let user_base_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &params.mint,
        );
        let user_quote_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );

        // 获取池的代币账户
        let pool_base_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &params.mint,
                &accounts::TOKEN_PROGRAM,
            );

        let pool_quote_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &accounts::WSOL_TOKEN_ACCOUNT,
                &accounts::TOKEN_PROGRAM,
            );

        self.build_sell_instructions_with_accounts(
            params,
            pool,
            pool_base_token_account,
            pool_quote_token_account,
            user_base_token_account,
            user_quote_token_account,
        )
        .await
    }

    /// 使用提供的账户信息构建买入指令
    async fn build_buy_instructions_with_accounts(
        &self,
        params: &BuyParams,
        pool: Pubkey,
        pool_base_token_account: Pubkey,
        pool_quote_token_account: Pubkey,
        user_base_token_account: Pubkey,
        user_quote_token_account: Pubkey,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        // 计算预期的代币数量
        let token_amount = get_buy_token_amount(rpc.as_ref(), &pool, params.amount_sol).await?;

        // 计算滑点后的最大SOL数量
        let max_sol_amount = calculate_with_slippage_buy(
            params.amount_sol,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );

        let mut instructions = vec![];

        // 创建用户的基础代币账户
        instructions.push(create_associated_token_account_idempotent(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &accounts::TOKEN_PROGRAM,
        ));

        let coin_creator_vault_ata = coin_creator_vault_ata(params.creator);
        let coin_creator_vault_authority = coin_creator_vault_authority(params.creator);

        // 创建买入指令
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(pool, false), // pool_id (readonly)
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // user (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_ACCOUNT, false), // global (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(params.mint, false), // mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // WSOL_TOKEN_ACCOUNT (readonly)
            solana_sdk::instruction::AccountMeta::new(user_base_token_account, false), // user_base_token_account
            solana_sdk::instruction::AccountMeta::new(user_quote_token_account, false), // user_quote_token_account
            solana_sdk::instruction::AccountMeta::new(pool_base_token_account, false), // pool_base_token_account
            solana_sdk::instruction::AccountMeta::new(pool_quote_token_account, false), // pool_quote_token_account
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::FEE_RECIPIENT, false), // fee_recipient (readonly)
            solana_sdk::instruction::AccountMeta::new(accounts::FEE_RECIPIENT_ATA, false), // fee_recipient_ata
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(
                accounts::ASSOCIATED_TOKEN_PROGRAM,
                false,
            ), // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // event_authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_PROGRAM, false), // PUMP_AMM_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new(coin_creator_vault_ata, false), // coin_creator_vault_ata
            solana_sdk::instruction::AccountMeta::new_readonly(coin_creator_vault_authority, false), // coin_creator_vault_authority (readonly)
        ];

        // 创建指令数据
        let mut data = vec![];
        data.extend_from_slice(&BUY_DISCRIMINATOR);
        data.extend_from_slice(&token_amount.to_le_bytes());
        data.extend_from_slice(&max_sol_amount.to_le_bytes());

        instructions.push(Instruction {
            program_id: accounts::AMM_PROGRAM,
            accounts,
            data,
        });

        Ok(instructions)
    }

    /// 使用提供的账户信息构建卖出指令
    async fn build_sell_instructions_with_accounts(
        &self,
        params: &SellParams,
        pool: Pubkey,
        pool_base_token_account: Pubkey,
        pool_quote_token_account: Pubkey,
        user_base_token_account: Pubkey,
        user_quote_token_account: Pubkey,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();

        // 获取代币余额
        let mut amount = params.amount_token;
        if params.amount_token.is_none() {
            let (balance_u64, _) =
                get_token_balance(rpc.as_ref(), params.payer.as_ref(), &params.mint).await?;
            amount = Some(balance_u64);
        }
        let amount = amount.unwrap_or(0);

        if amount == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        // 计算预期的SOL数量
        let sol_amount = get_sell_sol_amount(rpc.as_ref(), &pool, amount).await?;

        // 计算滑点后的最小SOL数量
        let min_sol_amount = calculate_with_slippage_sell(
            sol_amount,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );

        let coin_creator_vault_ata = coin_creator_vault_ata(params.creator);
        let coin_creator_vault_authority = coin_creator_vault_authority(params.creator);

        let mut instructions = vec![];

        // 创建用户的代币账户
        instructions.push(create_associated_token_account_idempotent(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &accounts::TOKEN_PROGRAM,
        ));

        // 创建卖出指令
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(pool, false), // pool_id (readonly)
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // user (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_ACCOUNT, false), // global (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(params.mint, false), // mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // WSOL_TOKEN_ACCOUNT (readonly)
            solana_sdk::instruction::AccountMeta::new(user_base_token_account, false), // user_base_token_account
            solana_sdk::instruction::AccountMeta::new(user_quote_token_account, false), // user_quote_token_account
            solana_sdk::instruction::AccountMeta::new(pool_base_token_account, false), // pool_base_token_account
            solana_sdk::instruction::AccountMeta::new(pool_quote_token_account, false), // pool_quote_token_account
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::FEE_RECIPIENT, false), // fee_recipient (readonly)
            solana_sdk::instruction::AccountMeta::new(accounts::FEE_RECIPIENT_ATA, false), // fee_recipient_ata
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(
                accounts::ASSOCIATED_TOKEN_PROGRAM,
                false,
            ), // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // event_authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_PROGRAM, false), // PUMP_AMM_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new(coin_creator_vault_ata, false), // coin_creator_vault_ata
            solana_sdk::instruction::AccountMeta::new_readonly(coin_creator_vault_authority, false), // coin_creator_vault_authority (readonly)
        ];

        // 创建指令数据
        let mut data = vec![];
        data.extend_from_slice(&SELL_DISCRIMINATOR);
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(&min_sol_amount.to_le_bytes());

        instructions.push(Instruction {
            program_id: accounts::AMM_PROGRAM,
            accounts,
            data,
        });

        Ok(instructions)
    }
}
