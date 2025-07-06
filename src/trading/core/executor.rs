use anyhow::{anyhow, Result};
use solana_sdk::signer::Signer;
use std::sync::Arc;

use super::{
    parallel::parallel_execute_with_tips,
    params::{BuyParams, BuyWithTipParams, SellParams, SellWithTipParams},
    timer::TradeTimer,
    traits::{InstructionBuilder, TradeExecutor},
};
use crate::{
    swqos::TradeType,
    trading::common::{build_rpc_transaction, build_sell_transaction},
};

/// 通用交易执行器实现
pub struct GenericTradeExecutor {
    instruction_builder: Arc<dyn InstructionBuilder>,
    protocol_name: &'static str,
}

impl GenericTradeExecutor {
    pub fn new(
        instruction_builder: Arc<dyn InstructionBuilder>,
        protocol_name: &'static str,
    ) -> Self {
        Self {
            instruction_builder,
            protocol_name,
        }
    }

    /// 获取代币余额
    async fn get_token_balance(
        &self,
        rpc: Arc<crate::common::SolanaRpcClient>,
        payer: &solana_sdk::signature::Keypair,
        mint: &solana_sdk::pubkey::Pubkey,
    ) -> Result<u64> {
        let ata = spl_associated_token_account::get_associated_token_address(&payer.pubkey(), mint);
        let balance = rpc.get_token_account_balance(&ata).await?;
        balance
            .amount
            .parse::<u64>()
            .map_err(|_| anyhow!("Failed to parse token balance"))
    }
}

#[async_trait::async_trait]
impl TradeExecutor for GenericTradeExecutor {
    async fn buy(&self, params: BuyParams) -> Result<()> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        let mut timer = TradeTimer::new("构建买入交易指令");
        // 构建指令
        let instructions = self
            .instruction_builder
            .build_buy_instructions(&params)
            .await?;
        timer.stage("买入交易指令");

        // 构建交易
        let transaction = build_rpc_transaction(
            params.payer.clone(),
            &params.priority_fee,
            instructions,
            params.lookup_table_key,
            params.recent_blockhash,
            params.data_size_limit,
        )
        .await?;
        timer.stage("买入交易签名");

        // 发送交易
        rpc.send_and_confirm_transaction(&transaction).await?;
        timer.finish();

        Ok(())
    }

    async fn buy_with_tip(&self, params: BuyWithTipParams) -> Result<()> {
        let mut timer = TradeTimer::new("构建买入交易指令");

        // 验证参数 - 转换为BuyParams进行验证
        let buy_params = BuyParams {
            rpc: params.rpc,
            payer: params.payer.clone(),
            mint: params.mint,
            creator: params.creator,
            amount_sol: params.amount_sol,
            slippage_basis_points: params.slippage_basis_points,
            priority_fee: params.priority_fee.clone(),
            lookup_table_key: params.lookup_table_key,
            recent_blockhash: params.recent_blockhash,
            data_size_limit: params.data_size_limit,
            protocol_params: params.protocol_params.clone(),
        };

        // 构建指令
        let instructions = self
            .instruction_builder
            .build_buy_instructions(&buy_params)
            .await?;
        timer.stage("买入交易指令");

        // 并行执行交易
        parallel_execute_with_tips(
            params.swqos_clients,
            params.payer,
            instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            params.data_size_limit,
            TradeType::Buy,
        )
        .await?;

        timer.finish();
        Ok(())
    }

    async fn sell(&self, params: SellParams) -> Result<()> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        let mut timer = TradeTimer::new("构建卖出交易指令");

        // 构建指令
        let instructions = self
            .instruction_builder
            .build_sell_instructions(&params)
            .await?;
        timer.stage("卖出交易指令");

        // 构建交易
        let transaction = build_sell_transaction(
            params.payer.clone(),
            &params.priority_fee,
            instructions,
            params.lookup_table_key,
            params.recent_blockhash,
        )
        .await?;
        timer.stage("卖出交易签名");

        // 发送交易
        rpc.send_and_confirm_transaction(&transaction).await?;
        timer.finish();

        Ok(())
    }

    async fn sell_with_tip(&self, params: SellWithTipParams) -> Result<()> {
        let mut timer = TradeTimer::new("构建卖出交易指令");

        // 转换为SellParams进行指令构建
        let sell_params = SellParams {
            rpc: params.rpc,
            payer: params.payer.clone(),
            mint: params.mint,
            creator: params.creator,
            amount_token: params.amount_token,
            slippage_basis_points: params.slippage_basis_points,
            priority_fee: params.priority_fee.clone(),
            lookup_table_key: params.lookup_table_key,
            recent_blockhash: params.recent_blockhash,
            protocol_params: params.protocol_params.clone(),
        };

        // 构建指令
        let instructions = self
            .instruction_builder
            .build_sell_instructions(&sell_params)
            .await?;
        timer.stage("卖出交易指令");

        // 并行执行交易
        parallel_execute_with_tips(
            params.swqos_clients,
            params.payer,
            instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            0,
            TradeType::Sell,
        )
        .await?;

        timer.finish();
        Ok(())
    }

    fn protocol_name(&self) -> &'static str {
        self.protocol_name
    }
}
