use crate::impl_unified_event;
use crate::streaming::event_parser::common::EventMetadata;
// use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// 交易
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmSwapEvent {
    pub metadata: EventMetadata,
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit_x64: u128,
    pub is_base_input: bool,
    pub payer: Pubkey,
    pub amm_config: Pubkey,
    pub pool_state: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
    pub input_vault: Pubkey,
    pub output_vault: Pubkey,
    pub observation_state: Pubkey,
    pub token_program: Pubkey,
    pub tick_array: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}

impl_unified_event!(RaydiumClmmSwapEvent,);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaydiumClmmSwapV2Event {
    pub metadata: EventMetadata,
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit_x64: u128,
    pub is_base_input: bool,
    pub payer: Pubkey,
    pub amm_config: Pubkey,
    pub pool_state: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
    pub input_vault: Pubkey,
    pub output_vault: Pubkey,
    pub observation_state: Pubkey,
    pub token_program: Pubkey,
    pub token_program2022: Pubkey,
    pub memo_program: Pubkey,
    pub input_vault_mint: Pubkey,
    pub output_vault_mint: Pubkey,
    pub remaining_accounts: Vec<Pubkey>,
}
impl_unified_event!(RaydiumClmmSwapV2Event,);

/// 事件鉴别器常量
pub mod discriminators {
    // 指令鉴别器
    pub const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
    pub const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];
}
