use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

use crate::error::{ClientError, ClientResult};

/// Raydium指令类型
#[derive(Debug)]
pub enum RaydiumInstruction {
    V4Swap(V4SwapEvent),
    SwapBaseInput(SwapBaseInputEvent),
    SwapBaseOutput(SwapBaseOutputEvent),
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct SwapBaseOutputEvent {
    #[borsh(skip)]
    pub timestamp: i64,
    #[borsh(skip)]
    pub slot: u64,
    #[borsh(skip)]
    pub signature: String,
    pub max_amount_in: u64,
    pub amount_out: u64,

    #[borsh(skip)]
    pub payer: Pubkey,
    #[borsh(skip)]
    pub authority: Pubkey,
    #[borsh(skip)]
    pub amm_config: Pubkey,
    #[borsh(skip)]
    pub pool_state: Pubkey,
    #[borsh(skip)]
    pub input_token_account: Pubkey,
    #[borsh(skip)]
    pub output_token_account: Pubkey,
    #[borsh(skip)]
    pub input_vault: Pubkey,
    #[borsh(skip)]
    pub output_vault: Pubkey,
    #[borsh(skip)]
    pub input_token_program: Pubkey,
    #[borsh(skip)]
    pub output_token_program: Pubkey,
    #[borsh(skip)]
    pub input_token_mint: Pubkey,
    #[borsh(skip)]
    pub output_token_mint: Pubkey,
    #[borsh(skip)]
    pub observation_state: Pubkey,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct SwapBaseInputEvent {
    #[borsh(skip)]
    pub timestamp: i64,
    #[borsh(skip)]
    pub slot: u64,
    #[borsh(skip)]
    pub signature: String,
    pub amount_in: u64,
    pub minimum_amount_out: u64,

    #[borsh(skip)]
    pub payer: Pubkey,
    #[borsh(skip)]
    pub authority: Pubkey,
    #[borsh(skip)]
    pub amm_config: Pubkey,
    #[borsh(skip)]
    pub pool_state: Pubkey,
    #[borsh(skip)]
    pub input_token_account: Pubkey,
    #[borsh(skip)]
    pub output_token_account: Pubkey,
    #[borsh(skip)]
    pub input_vault: Pubkey,
    #[borsh(skip)]
    pub output_vault: Pubkey,
    #[borsh(skip)]
    pub input_token_program: Pubkey,
    #[borsh(skip)]
    pub output_token_program: Pubkey,
    #[borsh(skip)]
    pub input_token_mint: Pubkey,
    #[borsh(skip)]
    pub output_token_mint: Pubkey,
    #[borsh(skip)]
    pub observation_state: Pubkey,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct V4SwapEvent {
    #[borsh(skip)]
    pub timestamp: i64,
    #[borsh(skip)]
    pub slot: u64,
    #[borsh(skip)]
    pub signature: String,
    pub amount_in: u64,
    pub minimum_amount_out: u64,

    #[borsh(skip)]
    pub amm: Pubkey,
    #[borsh(skip)]
    pub amm_authority: Pubkey,
    #[borsh(skip)]
    pub amm_open_orders: Pubkey,
    #[borsh(skip)]
    pub amm_target_orders: Pubkey,
    #[borsh(skip)]
    pub pool_coin_token_account: Pubkey,
    #[borsh(skip)]
    pub pool_pc_token_account: Pubkey,
    #[borsh(skip)]
    pub serum_program: Pubkey,
    #[borsh(skip)]
    pub serum_market: Pubkey,
    #[borsh(skip)]
    pub serum_bids: Pubkey,
    #[borsh(skip)]
    pub serum_asks: Pubkey,
    #[borsh(skip)]
    pub serum_event_queue: Pubkey,
    #[borsh(skip)]
    pub serum_coin_vault_account: Pubkey,
    #[borsh(skip)]
    pub serum_pc_vault_account: Pubkey,
    #[borsh(skip)]
    pub serum_vault_signer: Pubkey,
    #[borsh(skip)]
    pub user_source_token_account: Pubkey,
    #[borsh(skip)]
    pub user_destination_token_account: Pubkey,
    #[borsh(skip)]
    pub user_source_owner: Pubkey,
}

/// 事件特性
pub trait EventTrait: Sized + std::fmt::Debug {
    fn from_bytes(bytes: &[u8]) -> ClientResult<Self>;
    fn discriminator() -> &'static [u8];
}

/// 从字节中提取鉴别器
pub fn extract_discriminator(length: usize, data: &[u8]) -> Option<(&[u8], &[u8])> {
    if data.len() < length {
        return None;
    }
    Some((&data[..length], &data[length..]))
}

/// 事件鉴别器常量
pub mod discriminators {
    pub const V4_SWAP_IX: &u8 = &9;

    pub const SWAP_BASE_INPUT_IX: &[u8] = &[143, 190, 90, 218, 196, 30, 51, 222];
    pub const SWAP_BASE_OUTPUT_IX: &[u8] = &[55, 217, 98, 86, 163, 74, 180, 173];
}
