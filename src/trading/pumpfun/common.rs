use crate::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::PumpFunTradeEvent;
use crate::{
    common::{
        bonding_curve::BondingCurveAccount, global::GlobalAccount, PriorityFee, SolanaRpcClient,
    },
    constants::{self, trade::trade::DEFAULT_SLIPPAGE},
};
use anyhow::anyhow;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, pubkey::Pubkey,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

lazy_static::lazy_static! {
    static ref ACCOUNT_CACHE: RwLock<HashMap<Pubkey, Arc<GlobalAccount>>> = RwLock::new(HashMap::new());
}

#[inline]
pub fn create_priority_fee_instructions(priority_fee: PriorityFee) -> Vec<Instruction> {
    let mut instructions = Vec::with_capacity(2);
    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit));
    instructions.push(ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price));

    instructions
}

#[inline]
pub fn get_global_pda() -> Pubkey {
    static GLOBAL_PDA: once_cell::sync::Lazy<Pubkey> = once_cell::sync::Lazy::new(|| {
        Pubkey::find_program_address(
            &[constants::pumpfun::seeds::GLOBAL_SEED],
            &constants::pumpfun::accounts::PUMPFUN,
        )
        .0
    });
    *GLOBAL_PDA
}

#[inline]
pub fn get_mint_authority_pda() -> Pubkey {
    static MINT_AUTHORITY_PDA: once_cell::sync::Lazy<Pubkey> = once_cell::sync::Lazy::new(|| {
        Pubkey::find_program_address(
            &[constants::pumpfun::seeds::MINT_AUTHORITY_SEED],
            &constants::pumpfun::accounts::PUMPFUN,
        )
        .0
    });
    *MINT_AUTHORITY_PDA
}

#[inline]
pub fn get_bonding_curve_pda(mint: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[constants::pumpfun::seeds::BONDING_CURVE_SEED, mint.as_ref()];
    let program_id: &Pubkey = &constants::pumpfun::accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_creator_vault_pda(creator: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[constants::pumpfun::seeds::CREATOR_VAULT_SEED, creator.as_ref()];
    let program_id: &Pubkey = &constants::pumpfun::accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_user_volume_accumulator_pda(user: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] =
        &[constants::pumpfun::seeds::USER_VOLUME_ACCUMULATOR_SEED, user.as_ref()];
    let program_id: &Pubkey = &constants::pumpfun::accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_global_volume_accumulator_pda() -> Option<Pubkey> {
    let seeds: &[&[u8]; 1] = &[constants::pumpfun::seeds::GLOBAL_VOLUME_ACCUMULATOR_SEED];
    let program_id: &Pubkey = &constants::pumpfun::accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_metadata_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            constants::pumpfun::seeds::METADATA_SEED,
            constants::pumpfun::accounts::MPL_TOKEN_METADATA.as_ref(),
            mint.as_ref(),
        ],
        &constants::pumpfun::accounts::MPL_TOKEN_METADATA,
    )
    .0
}

#[inline]
pub async fn get_global_account(/*rpc: &SolanaRpcClient*/
) -> Result<Arc<GlobalAccount>, anyhow::Error> {
    let global_account = GlobalAccount::new();
    let global_account = Arc::new(global_account);
    Ok(global_account)
}

#[inline]
pub async fn get_initial_buy_price(
    global_account: &Arc<GlobalAccount>,
    amount_sol: u64,
) -> Result<u64, anyhow::Error> {
    let buy_amount = global_account.get_initial_buy_price(amount_sol);
    Ok(buy_amount)
}

#[inline]
pub async fn fetch_bonding_curve_account(
    rpc: &SolanaRpcClient,
    mint: &Pubkey,
) -> Result<(Arc<crate::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::types::BondingCurve>, Pubkey), anyhow::Error>{
    let bonding_curve_pda: Pubkey =
        get_bonding_curve_pda(mint).ok_or(anyhow!("Bonding curve not found"))?;

    let account = rpc.get_account(&bonding_curve_pda).await?;
    if account.data.is_empty() {
        return Err(anyhow!("Bonding curve not found"));
    }

    let bonding_curve = solana_sdk::borsh1::try_from_slice_unchecked::<crate::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::types::BondingCurve>(&account.data[8..])
        .map_err(|e| anyhow::anyhow!("Failed to deserialize bonding curve account: {}", e))?;

    Ok((Arc::new(bonding_curve), bonding_curve_pda))
}

#[inline]
pub async fn init_bonding_curve_account(
    mint: &Pubkey,
    dev_buy_token: u64,
    dev_sol_cost: u64,
    creator: Pubkey,
) -> Result<Arc<BondingCurveAccount>, anyhow::Error> {
    let bonding_curve =
        BondingCurveAccount::from_dev_trade(mint, dev_buy_token, dev_sol_cost, creator);
    let bonding_curve = Arc::new(bonding_curve);
    Ok(bonding_curve)
}

#[inline]
pub fn get_buy_amount_with_slippage(amount_sol: u64, slippage_basis_points: Option<u64>) -> u64 {
    let slippage = slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE);
    amount_sol + (amount_sol * slippage / 10000)
}

#[inline]
pub fn get_buy_price(amount: u64, trade_info: &PumpFunTradeEvent) -> u64 {
    if amount == 0 {
        return 0;
    }

    let n: u128 =
        (trade_info.virtual_sol_reserves as u128) * (trade_info.virtual_token_reserves as u128);
    let i: u128 = (trade_info.virtual_sol_reserves as u128) + (amount as u128);
    let r: u128 = n / i + 1;
    let s: u128 = (trade_info.virtual_token_reserves as u128) - r;
    let s_u64 = s as u64;

    s_u64.min(trade_info.real_token_reserves)
}
