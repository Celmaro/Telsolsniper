use anyhow::{anyhow, Result};
use std::sync::Arc;

use super::{
    parallel::parallel_execute_with_tips,
    params::{BuyParams, BuyWithTipParams, SellParams, SellWithTipParams},
    timer::TradeTimer,
    traits::{InstructionBuilder, TradeExecutor},
};
use crate::{
    swqos::TradeType,
    trading::{
        common::{build_rpc_transaction, build_sell_transaction},
        middleware::MiddlewareManager,
    },
};

const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 256 * 1024;

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
        Self { instruction_builder, protocol_name }
    }
}

#[async_trait::async_trait]
impl TradeExecutor for GenericTradeExecutor {
    async fn buy(
        &self,
        mut params: BuyParams,
        middleware_manager: Option<Arc<MiddlewareManager>>,
    ) -> Result<()> {
        if params.data_size_limit == 0 {
            params.data_size_limit = MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT;
        }
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        let mut timer = TradeTimer::new("构建买入交易指令");
        // 构建指令
        let instructions = self.instruction_builder.build_buy_instructions(&params).await?;
        let final_instructions = match middleware_manager.clone() {
            Some(middleware_manager) => middleware_manager
                .apply_middlewares_process_protocol_instructions(
                    instructions,
                    self.protocol_name.to_string(),
                    true,
                )?,
            None => instructions,
        };
        timer.stage("构建rpc交易指令");

        // 构建交易
        let transaction = build_rpc_transaction(
            params.payer.clone(),
            &params.priority_fee,
            final_instructions,
            params.lookup_table_key,
            params.recent_blockhash,
            params.data_size_limit,
            middleware_manager,
            self.protocol_name.to_string(),
            true,
        )
        .await?;
        timer.stage("rpc提交确认");

        // 发送交易
        if params.wait_transaction_confirmed {
            rpc.send_and_confirm_transaction(&transaction).await?;
        } else {
            // 异步发送交易
            rpc.send_transaction(&transaction).await?;
        }
        timer.finish();

        Ok(())
    }

    async fn buy_with_tip(
        &self,
        mut params: BuyWithTipParams,
        middleware_manager: Option<Arc<MiddlewareManager>>,
    ) -> Result<()> {
        if params.data_size_limit == 0 {
            params.data_size_limit = MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT;
        }
        let timer = TradeTimer::new("构建买入交易指令");

        // 验证参数 - 转换为BuyParams进行验证
        let buy_params = BuyParams {
            rpc: params.rpc,
            payer: params.payer.clone(),
            mint: params.mint,
            sol_amount: params.sol_amount,
            slippage_basis_points: params.slippage_basis_points,
            priority_fee: params.priority_fee.clone(),
            lookup_table_key: params.lookup_table_key,
            recent_blockhash: params.recent_blockhash,
            data_size_limit: params.data_size_limit,
            wait_transaction_confirmed: params.wait_transaction_confirmed,
            protocol_params: params.protocol_params.clone(),
        };

        // 构建指令
        let instructions = self.instruction_builder.build_buy_instructions(&buy_params).await?;
        let final_instructions = match middleware_manager.clone() {
            Some(middleware_manager) => middleware_manager
                .apply_middlewares_process_protocol_instructions(
                    instructions,
                    self.protocol_name.to_string(),
                    true,
                )?,
            None => instructions,
        };

        timer.finish();

        // 并行执行交易
        parallel_execute_with_tips(
            params.swqos_clients,
            params.payer,
            final_instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            params.data_size_limit,
            TradeType::Buy,
            middleware_manager,
            self.protocol_name.to_string(),
            true,
            params.wait_transaction_confirmed,
        )
        .await?;

        Ok(())
    }

    async fn sell(
        &self,
        params: SellParams,
        middleware_manager: Option<Arc<MiddlewareManager>>,
    ) -> Result<()> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        let mut timer = TradeTimer::new("构建卖出交易指令");

        // 构建指令
        let instructions = self.instruction_builder.build_sell_instructions(&params).await?;
        let final_instructions = match middleware_manager.clone() {
            Some(middleware_manager) => middleware_manager
                .apply_middlewares_process_protocol_instructions(
                    instructions,
                    self.protocol_name.to_string(),
                    false,
                )?,
            None => instructions,
        };
        timer.stage("卖出交易指令");

        // 构建交易
        let transaction = build_sell_transaction(
            params.payer.clone(),
            &params.priority_fee,
            final_instructions,
            params.lookup_table_key,
            params.recent_blockhash,
            middleware_manager,
            self.protocol_name.to_string(),
            false,
        )
        .await?;
        timer.stage("卖出交易签名");

        // 发送交易
        if params.wait_transaction_confirmed {
            rpc.send_and_confirm_transaction(&transaction).await?;
        } else {
            rpc.send_transaction(&transaction).await?;
        }
        timer.finish();

        Ok(())
    }

    async fn sell_with_tip(
        &self,
        params: SellWithTipParams,
        middleware_manager: Option<Arc<MiddlewareManager>>,
    ) -> Result<()> {
        let timer = TradeTimer::new("构建卖出交易指令");

        // 转换为SellParams进行指令构建
        let sell_params = SellParams {
            rpc: params.rpc,
            payer: params.payer.clone(),
            mint: params.mint,
            token_amount: params.token_amount,
            slippage_basis_points: params.slippage_basis_points,
            priority_fee: params.priority_fee.clone(),
            lookup_table_key: params.lookup_table_key,
            recent_blockhash: params.recent_blockhash,
            wait_transaction_confirmed: params.wait_transaction_confirmed,
            protocol_params: params.protocol_params.clone(),
        };

        // 构建指令
        let instructions = self.instruction_builder.build_sell_instructions(&sell_params).await?;
        let final_instructions = match middleware_manager.clone() {
            Some(middleware_manager) => middleware_manager
                .apply_middlewares_process_protocol_instructions(
                    instructions,
                    self.protocol_name.to_string(),
                    false,
                )?,
            None => instructions,
        };

        timer.finish();

        // 并行执行交易
        parallel_execute_with_tips(
            params.swqos_clients,
            params.payer,
            final_instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            0,
            TradeType::Sell,
            middleware_manager,
            self.protocol_name.to_string(),
            false,
            params.wait_transaction_confirmed,
        )
        .await?;

        Ok(())
    }

    fn protocol_name(&self) -> &'static str {
        self.protocol_name
    }
}
