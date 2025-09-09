use anyhow::Result;
use solana_sdk::signature::Signature;
use std::{sync::Arc, time::Instant};

use super::{
    parallel::parallel_execute_with_tips,
    params::{BuyParams, SellParams},
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
    ) -> Result<Signature> {
        let mut data_size_limit = params.data_size_limit;
        if data_size_limit == 0 {
            data_size_limit = MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT;
        }

        let start = Instant::now();

        // Build instructions directly from params to avoid unnecessary cloning
        let instructions = self.instruction_builder.build_buy_instructions(&params).await?;
        let final_instructions = match &middleware_manager {
            Some(middleware_manager) => middleware_manager
                .apply_middlewares_process_protocol_instructions(
                    instructions,
                    self.protocol_name.to_string(),
                    true,
                )?,
            None => instructions,
        };

        println!("Building buy transaction instructions time cost: {:?}", start.elapsed());

        // Execute transactions in parallel
        let sig = parallel_execute_with_tips(
            swqos_clients,
            params.payer,
            final_instructions,
            Arc::new(params.priority_fee),
            params.lookup_table_key,
            params.recent_blockhash,
            data_size_limit,
            middleware_manager,
            self.protocol_name,
            true,
            params.wait_transaction_confirmed,
            true,
        )
        .await?;

        Ok(sig)
    }

    async fn sell_with_tip(
        &self,
        params: SellParams,
        swqos_clients: Vec<Arc<SwqosClient>>,
        middleware_manager: Option<Arc<MiddlewareManager>>,
    ) -> Result<Signature> {
        let start = Instant::now();

        // Build instructions directly from params to avoid unnecessary cloning
        let instructions = self.instruction_builder.build_sell_instructions(&params).await?;
        let final_instructions = match &middleware_manager {
            Some(middleware_manager) => middleware_manager
                .apply_middlewares_process_protocol_instructions(
                    instructions,
                    self.protocol_name.to_string(),
                    false,
                )?,
            None => instructions,
        };

        println!("Building sell transaction instructions time cost: {:?}", start.elapsed());

        // Execute transactions in parallel
        let sig = parallel_execute_with_tips(
            swqos_clients,
            params.payer,
            final_instructions,
            Arc::new(params.priority_fee),
            params.lookup_table_key,
            params.recent_blockhash,
            0,
            middleware_manager,
            self.protocol_name,
            false,
            params.wait_transaction_confirmed,
            params.with_tip,
        )
        .await?;

        Ok(sig)
    }

    fn protocol_name(&self) -> &'static str {
        self.protocol_name
    }
}
