use crate::constants;
use solana_sdk::pubkey::Pubkey;

pub fn get_amount_out(
    amount_in: u64,
    protocol_fee_rate: u128,
    platform_fee_rate: u128,
    share_fee_rate: u128,
    virtual_base: u128,
    virtual_quote: u128,
    real_base: u128,
    real_quote: u128,
    slippage_basis_points: u128,
) -> u64 {
    let amount_in_u128 = amount_in as u128;
    let protocol_fee = (amount_in_u128 * protocol_fee_rate / 10000) as u128;
    let platform_fee = (amount_in_u128 * platform_fee_rate / 10000) as u128;
    let share_fee = (amount_in_u128 * share_fee_rate / 10000) as u128;
    let amount_in_net = amount_in_u128
        .checked_sub(protocol_fee)
        .unwrap()
        .checked_sub(platform_fee)
        .unwrap()
        .checked_sub(share_fee)
        .unwrap();
    let input_reserve = virtual_quote.checked_add(real_quote).unwrap();
    let output_reserve = virtual_base.checked_sub(real_base).unwrap();
    let numerator = amount_in_net.checked_mul(output_reserve).unwrap();
    let denominator = input_reserve.checked_add(amount_in_net).unwrap();
    let mut amount_out = numerator.checked_div(denominator).unwrap();

    amount_out = amount_out - (amount_out * slippage_basis_points) / 10000;
    amount_out as u64
}

pub fn get_pool_pda(base_mint: &Pubkey, quote_mint: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 3] = &[
        constants::bonk::seeds::POOL_SEED,
        base_mint.as_ref(),
        quote_mint.as_ref(),
    ];
    let program_id: &Pubkey = &constants::bonk::accounts::BONK;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub fn get_vault_pda(pool_state: &Pubkey, mint: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 3] = &[
        constants::bonk::seeds::POOL_VAULT_SEED,
        pool_state.as_ref(),
        mint.as_ref(),
    ];
    let program_id: &Pubkey = &constants::bonk::accounts::BONK;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub fn get_token_price(
    virtual_base: u128,
    virtual_quote: u128,
    real_base: u128,
    real_quote: u128,
    decimal_base: u64,
    decimal_quote: u64,
) -> f64 {
    // 计算小数位数差异
    let decimal_diff = decimal_quote as i32 - decimal_base as i32;
    let decimal_factor = if decimal_diff >= 0 {
        10_f64.powi(decimal_diff)
    } else {
        1.0 / 10_f64.powi(-decimal_diff)
    };

    // 计算价格前的状态
    let quote_reserves = virtual_quote.checked_add(real_quote).unwrap();
    let base_reserves = virtual_base.checked_sub(real_base).unwrap();

    // 使用浮点数计算价格，避免整数除法的精度丢失
    let price = (quote_reserves as f64) / (base_reserves as f64) / decimal_factor;

    price
}
