use anyhow::Result;
use std::sync::Arc;

use super::{
    parallel::parallel_execute_with_tips,
    params::{BuyParams, SellParams},
    timer::TradeTimer,
    traits::{InstructionBuilder, TradeExecutor},
};
use crate::{swqos::SwqosClient, trading::middleware::MiddlewareManager};

const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 256 * 1024;

/// Generic trade executor implementation
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
    async fn buy_with_tip(
        &self,
        params: BuyParams,
        swqos_clients: Vec<Arc<SwqosClient>>,
        middleware_manager: Option<Arc<MiddlewareManager>>,
    ) -> Result<()> {
        let mut data_size_limit = params.data_size_limit;
        if data_size_limit == 0 {
            data_size_limit = MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT;
        }
        let timer = TradeTimer::new("Building buy transaction instructions");

        // Validate parameters - convert to BuyParams for validation
        let buy_params = BuyParams {
            rpc: params.rpc,
            payer: params.payer.clone(),
            mint: params.mint,
            sol_amount: params.sol_amount,
            slippage_basis_points: params.slippage_basis_points,
            priority_fee: params.priority_fee.clone(),
            lookup_table_key: params.lookup_table_key,
            recent_blockhash: params.recent_blockhash,
            data_size_limit: data_size_limit,
            wait_transaction_confirmed: params.wait_transaction_confirmed,
            protocol_params: params.protocol_params.clone(),
        };

        // Build instructions
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

        // Execute transactions in parallel
        parallel_execute_with_tips(
            swqos_clients,
            params.payer,
            final_instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            data_size_limit,
            middleware_manager,
            self.protocol_name.to_string(),
            true,
            params.wait_transaction_confirmed,
            true,
        )
        .await?;

        Ok(())
    }

    async fn sell_with_tip(
        &self,
        params: SellParams,
        swqos_clients: Vec<Arc<SwqosClient>>,
        middleware_manager: Option<Arc<MiddlewareManager>>,
    ) -> Result<()> {
        let timer = TradeTimer::new("Building sell transaction instructions");

        // Convert to SellParams for instruction building
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
            with_tip: params.with_tip,
        };

        // Build instructions
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

        // Execute transactions in parallel
        parallel_execute_with_tips(
            swqos_clients,
            params.payer,
            final_instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            0,
            middleware_manager,
            self.protocol_name.to_string(),
            false,
            params.wait_transaction_confirmed,
            params.with_tip,
        )
        .await?;

        Ok(())
    }

    fn protocol_name(&self) -> &'static str {
        self.protocol_name
    }
}
