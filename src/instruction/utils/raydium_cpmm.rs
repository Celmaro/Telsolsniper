use crate::{common::SolanaRpcClient, trading::core::params::RaydiumCpmmParams};
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::event_parser::protocols::raydium_cpmm::types::{
    pool_state_decode, PoolState,
};

/// Constants used as seeds for deriving PDAs (Program Derived Addresses)
pub mod seeds {
    pub const POOL_SEED: &[u8] = b"pool";
    pub const POOL_VAULT_SEED: &[u8] = b"pool_vault";
    pub const OBSERVATION_STATE_SEED: &[u8] = b"observation";
}

/// Constants related to program accounts and authorities
pub mod accounts {
    use solana_sdk::{pubkey, pubkey::Pubkey};
    pub const AUTHORITY: Pubkey = pubkey!("GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL");
    pub const AMM_CONFIG: Pubkey = pubkey!("D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2");
    pub const RAYDIUM_CPMM: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
    pub const FEE_RATE_DENOMINATOR_VALUE: u128 = 1_000_000;
    pub const TRADE_FEE_RATE: u64 = 2500;
    pub const CREATOR_FEE_RATE: u64 = 0;
    pub const PROTOCOL_FEE_RATE: u64 = 120000;
    pub const FUND_FEE_RATE: u64 = 40000;
    // META
    pub const AUTHORITY_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: AUTHORITY,
            is_signer: false,
            is_writable: false,
        };

    pub const AMM_CONFIG_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: AMM_CONFIG,
            is_signer: false,
            is_writable: false,
        };
}

pub const SWAP_BASE_IN_DISCRIMINATOR: &[u8] = &[143, 190, 90, 218, 196, 30, 51, 222];
pub const SWAP_BASE_OUT_DISCRIMINATOR: &[u8] = &[55, 217, 98, 86, 163, 74, 180, 173];

pub async fn fetch_pool_state(
    rpc: &SolanaRpcClient,
    pool_address: &Pubkey,
) -> Result<PoolState, anyhow::Error> {
    let account = rpc.get_account(pool_address).await?;
    if account.owner != accounts::RAYDIUM_CPMM {
        return Err(anyhow!("Account is not owned by Raydium Cpmm program"));
    }
    let pool_state = pool_state_decode(&account.data[8..])
        .ok_or_else(|| anyhow!("Failed to decode pool state"))?;
    Ok(pool_state)
}

pub fn get_pool_pda(amm_config: &Pubkey, mint1: &Pubkey, mint2: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 4] =
        &[seeds::POOL_SEED, amm_config.as_ref(), mint1.as_ref(), mint2.as_ref()];
    let program_id: &Pubkey = &accounts::RAYDIUM_CPMM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub fn get_vault_pda(pool_state: &Pubkey, mint: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 3] = &[seeds::POOL_VAULT_SEED, pool_state.as_ref(), mint.as_ref()];
    let program_id: &Pubkey = &accounts::RAYDIUM_CPMM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub fn get_observation_state_pda(pool_state: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[seeds::OBSERVATION_STATE_SEED, pool_state.as_ref()];
    let program_id: &Pubkey = &accounts::RAYDIUM_CPMM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

/// 获取池子中两个代币的余额
///
/// # 返回值
/// 返回 token0_balance, token1_balance
pub async fn get_pool_token_balances(
    rpc: &SolanaRpcClient,
    pool_state: &Pubkey,
    token0_mint: &Pubkey,
    token1_mint: &Pubkey,
) -> Result<(u64, u64), anyhow::Error> {
    let token0_vault = get_vault_pda(pool_state, token0_mint).unwrap();
    let token0_balance = rpc.get_token_account_balance(&token0_vault).await?;
    let token1_vault = get_vault_pda(pool_state, token1_mint).unwrap();
    let token1_balance = rpc.get_token_account_balance(&token1_vault).await?;

    // 解析余额字符串为 u64
    let token0_amount =
        token0_balance.amount.parse::<u64>().map_err(|e| anyhow!("解析 token0 余额失败: {}", e))?;

    let token1_amount =
        token1_balance.amount.parse::<u64>().map_err(|e| anyhow!("解析 token1 余额失败: {}", e))?;

    Ok((token0_amount, token1_amount))
}

/// 计算代币价格 (token1/token0)
///
/// # 返回值
/// 返回 token1 相对于 token0 的价格
pub async fn calculate_price(
    token0_amount: u64,
    token1_amount: u64,
    mint0_decimals: u8,
    mint1_decimals: u8,
) -> Result<f64, anyhow::Error> {
    if token0_amount == 0 {
        return Err(anyhow!("Token0 余额为零，无法计算价格"));
    }
    // 考虑小数位精度
    let token0_adjusted = token0_amount as f64 / 10_f64.powi(mint0_decimals as i32);
    let token1_adjusted = token1_amount as f64 / 10_f64.powi(mint1_decimals as i32);
    let price = token1_adjusted / token0_adjusted;
    Ok(price)
}

/// 获取 vault 账户地址的辅助函数
///
/// # 参数
/// - `pool_state`: 池子状态账户地址
/// - `token_mint`: 代币 mint 地址
/// - `protocol_params`: 协议参数
/// - `is_wsol`: 是否为 wSOL 代币
///
/// # 返回值
/// 返回对应的 vault 账户地址
pub fn get_vault_account(
    pool_state: &Pubkey,
    token_mint: &Pubkey,
    protocol_params: &RaydiumCpmmParams,
    is_wsol: bool,
) -> Pubkey {
    if is_wsol {
        // 如果是 wSOL，检查是否为 base mint
        if protocol_params.base_mint == crate::constants::WSOL_TOKEN_ACCOUNT
            && protocol_params.base_vault != Pubkey::default()
        {
            protocol_params.base_vault
        } else if protocol_params.quote_mint == crate::constants::WSOL_TOKEN_ACCOUNT
            && protocol_params.quote_vault != Pubkey::default()
        {
            protocol_params.quote_vault
        } else {
            get_vault_pda(pool_state, &crate::constants::WSOL_TOKEN_ACCOUNT).unwrap()
        }
    } else {
        // 对于其他代币，检查是否为 base 或 quote mint
        if *token_mint == protocol_params.base_mint
            && protocol_params.base_vault != Pubkey::default()
        {
            protocol_params.base_vault
        } else if *token_mint == protocol_params.quote_mint
            && protocol_params.quote_vault != Pubkey::default()
        {
            protocol_params.quote_vault
        } else {
            get_vault_pda(pool_state, token_mint).unwrap()
        }
    }
}
