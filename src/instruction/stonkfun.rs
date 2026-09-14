//! StonkFun trading across its LaunchLab curve and graduated CPMM pools.

use super::{bonk::BonkInstructionBuilder, raydium_cpmm::RaydiumCpmmInstructionBuilder};
use crate::trading::core::{params::DexParamEnum, traits::InstructionBuilder};
use anyhow::{anyhow, Result};
use solana_sdk::instruction::Instruction;

/// User-facing StonkFun builder that selects the curve or graduated swap path
/// from [`DexParamEnum::StonkFun`] or [`DexParamEnum::StonkFunSwap`].
pub struct StonkFunInstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for StonkFunInstructionBuilder {
    async fn build_buy_instructions(
        &self,
        params: &crate::trading::core::params::SwapParams,
    ) -> Result<Vec<Instruction>> {
        match &params.protocol_params {
            DexParamEnum::StonkFun(_) => {
                BonkInstructionBuilder.build_buy_instructions(params).await
            }
            DexParamEnum::StonkFunSwap(_) => {
                RaydiumCpmmInstructionBuilder.build_buy_instructions(params).await
            }
            _ => Err(anyhow!("Invalid protocol params for StonkFun")),
        }
    }

    async fn build_sell_instructions(
        &self,
        params: &crate::trading::core::params::SwapParams,
    ) -> Result<Vec<Instruction>> {
        match &params.protocol_params {
            DexParamEnum::StonkFun(_) => {
                BonkInstructionBuilder.build_sell_instructions(params).await
            }
            DexParamEnum::StonkFunSwap(_) => {
                RaydiumCpmmInstructionBuilder.build_sell_instructions(params).await
            }
            _ => Err(anyhow!("Invalid protocol params for StonkFun")),
        }
    }
}
