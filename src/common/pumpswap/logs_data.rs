use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};

use crate::error::{ClientError, ClientResult};

/// PumpSwap指令类型
#[derive(Debug)]
pub enum PumpSwapInstruction {
    Buy(BuyEvent),
    Sell(SellEvent),
    CreatePool(CreatePoolEvent),
    Deposit(DepositEvent),
    Withdraw(WithdrawEvent),
    Disable(DisableEvent),
    UpdateAdmin(UpdateAdminEvent),
    UpdateFeeConfig(UpdateFeeConfigEvent),
    Other,
}

/// 买入事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct BuyEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub base_amount_out: u64,
    pub max_quote_amount_in: u64,
    pub user_base_token_reserves: u64,
    pub user_quote_token_reserves: u64,
    pub pool_base_token_reserves: u64,
    pub pool_quote_token_reserves: u64,
    pub quote_amount_in: u64,
    pub lp_fee_basis_points: u64,
    pub lp_fee: u64,
    pub protocol_fee_basis_points: u64,
    pub protocol_fee: u64,
    pub quote_amount_in_with_lp_fee: u64,
    pub user_quote_amount_in: u64,
    pub pool: Pubkey,
    pub user: Pubkey,
    pub user_base_token_account: Pubkey,
    pub user_quote_token_account: Pubkey,
    pub protocol_fee_recipient: Pubkey,
    pub protocol_fee_recipient_token_account: Pubkey,
    pub coin_creator: Pubkey,
    pub coin_creator_fee_basis_points: u64,
    pub coin_creator_fee: u64,
    #[borsh(skip)]
    pub signature: String,
}

/// 卖出事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct SellEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub base_amount_in: u64,
    pub min_quote_amount_out: u64,
    pub user_base_token_reserves: u64,
    pub user_quote_token_reserves: u64,
    pub pool_base_token_reserves: u64,
    pub pool_quote_token_reserves: u64,
    pub quote_amount_out: u64,
    pub lp_fee_basis_points: u64,
    pub lp_fee: u64,
    pub protocol_fee_basis_points: u64,
    pub protocol_fee: u64,
    pub quote_amount_out_without_lp_fee: u64,
    pub user_quote_amount_out: u64,
    pub pool: Pubkey,
    pub user: Pubkey,
    pub user_base_token_account: Pubkey,
    pub user_quote_token_account: Pubkey,
    pub protocol_fee_recipient: Pubkey,
    pub protocol_fee_recipient_token_account: Pubkey,
    pub coin_creator: Pubkey,
    pub coin_creator_fee_basis_points: u64,
    pub coin_creator_fee: u64,
    #[borsh(skip)]
    pub signature: String,
}

/// 创建池子事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct CreatePoolEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub index: u16,
    pub creator: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_mint_decimals: u8,
    pub quote_mint_decimals: u8,
    pub base_amount_in: u64,
    pub quote_amount_in: u64,
    pub pool_base_amount: u64,
    pub pool_quote_amount: u64,
    pub minimum_liquidity: u64,
    pub initial_liquidity: u64,
    pub lp_token_amount_out: u64,
    pub pool_bump: u8,
    pub pool: Pubkey,
    pub lp_mint: Pubkey,
    pub user_base_token_account: Pubkey,
    pub user_quote_token_account: Pubkey,
    #[borsh(skip)]
    pub signature: String,
}

/// 存款事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct DepositEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub lp_token_amount_out: u64,
    pub max_base_amount_in: u64,
    pub max_quote_amount_in: u64,
    pub user_base_token_reserves: u64,
    pub user_quote_token_reserves: u64,
    pub pool_base_token_reserves: u64,
    pub pool_quote_token_reserves: u64,
    pub base_amount_in: u64,
    pub quote_amount_in: u64,
    pub lp_mint_supply: u64,
    pub pool: Pubkey,
    pub user: Pubkey,
    pub user_base_token_account: Pubkey,
    pub user_quote_token_account: Pubkey,
    pub user_pool_token_account: Pubkey,
    #[borsh(skip)]
    pub signature: String,
}

/// 提款事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct WithdrawEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub lp_token_amount_in: u64,
    pub min_base_amount_out: u64,
    pub min_quote_amount_out: u64,
    pub user_base_token_reserves: u64,
    pub user_quote_token_reserves: u64,
    pub pool_base_token_reserves: u64,
    pub pool_quote_token_reserves: u64,
    pub base_amount_out: u64,
    pub quote_amount_out: u64,
    pub lp_mint_supply: u64,
    pub pool: Pubkey,
    pub user: Pubkey,
    pub user_base_token_account: Pubkey,
    pub user_quote_token_account: Pubkey,
    pub user_pool_token_account: Pubkey,
    #[borsh(skip)]
    pub signature: String,
}

/// 禁用事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct DisableEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub admin: Pubkey,
    pub disable_create_pool: bool,
    pub disable_deposit: bool,
    pub disable_withdraw: bool,
    pub disable_buy: bool,
    pub disable_sell: bool,
    #[borsh(skip)]
    pub signature: String,
}

/// 更新管理员事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct UpdateAdminEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
    #[borsh(skip)]
    pub signature: String,
}

/// 更新费用配置事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct UpdateFeeConfigEvent {
    #[borsh(skip)]
    pub slot: u64,
    pub timestamp: i64,
    pub admin: Pubkey,
    pub old_lp_fee_basis_points: u64,
    pub new_lp_fee_basis_points: u64,
    pub old_protocol_fee_basis_points: u64,
    pub new_protocol_fee_basis_points: u64,
    pub old_protocol_fee_recipients: [Pubkey; 8],
    pub new_protocol_fee_recipients: [Pubkey; 8],
    #[borsh(skip)]
    pub signature: String,
}



/// 全局配置
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct GlobalConfig {
    pub admin: Pubkey,
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
    pub disable_flags: u8,
    pub protocol_fee_recipients: [Pubkey; 8],
}

/// 池子信息
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct Pool {
    pub index: u16,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub base_mint_decimals: u8,
    pub quote_mint_decimals: u8,
    pub lp_mint_decimals: u8,
    pub base_token_account: Pubkey,
    pub quote_token_account: Pubkey,
    pub bump: u8,
    pub is_disabled: bool,
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
    // 事件鉴别器
    pub const BUY_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x67, 0xf4, 0x52, 0x1f, 0x2c, 0xf5, 0x77, 0x77];
    pub const SELL_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x3e, 0x2f, 0x37, 0x0a, 0xa5, 0x03, 0xdc, 0x2a];
    pub const CREATE_POOL_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0xb1, 0x31, 0x0c, 0xd2, 0xa0, 0x76, 0xa7, 0x74];
    pub const DEPOSIT_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x78, 0xf8, 0x3d, 0x53, 0x1f, 0x8e, 0x6b, 0x90];
    pub const WITHDRAW_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x16, 0x09, 0x85, 0x1a, 0xa0, 0x2c, 0x47, 0xc0];
    pub const DISABLE_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x6b, 0xfd, 0xc1, 0x4c, 0xe4, 0xca, 0x1b, 0x68];
    pub const UPDATE_ADMIN_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0xe1, 0x98, 0xab, 0x57, 0xf6, 0x3f, 0x42, 0xea];
    pub const UPDATE_FEE_CONFIG_EVENT: &[u8] = &[0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x5a, 0x17, 0x41, 0x23, 0x3e, 0xf4, 0xbc, 0xd0];

    // 指令鉴别器
    pub const BUY_IX: &[u8] = &[102,
                6,
                61,
                18,
                1,
                218,
                235,
                234];
    pub const SELL_IX: &[u8] = &[51,
    230,
    133,
    164,
    1,
    127,
    131,
    173];
    pub const CREATE_POOL_IX: &[u8] = &[233,
    146,
    209,
    142,
    207,
    104,
    64,
    188];
    pub const DEPOSIT_IX: &[u8] = &[242,
    35,
    198,
    137,
    82,
    225,
    242,
    182];
    pub const WITHDRAW_IX: &[u8] = &[183,
    18,
    70,
    156,
    148,
    109,
    161,
    34];
    pub const DISABLE_IX: &[u8] = &[107,
    253,
    193,
    76,
    228,
    202,
    27,
    104];
    pub const UPDATE_ADMIN_IX: &[u8] = &[225,
    152,
    171,
    87,
    246,
    63,
    66,
    234];
    pub const UPDATE_FEE_CONFIG_IX: &[u8] = &[90,
    23,
    65,
    35,
    62,
    244,
    188,
    208];
}