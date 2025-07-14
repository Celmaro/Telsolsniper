use crate::constants;
use solana_sdk::pubkey::Pubkey;

pub fn get_amount_in_net(
    amount_in: u64,
    protocol_fee_rate: u128,
    platform_fee_rate: u128,
    share_fee_rate: u128,
) -> u64 {
    let amount_in_u128 = amount_in as u128;
    let protocol_fee = (amount_in_u128 * protocol_fee_rate / 10000) as u128;
    let platform_fee = (amount_in_u128 * platform_fee_rate / 10000) as u128;
    let share_fee = (amount_in_u128 * share_fee_rate / 10000) as u128;
    amount_in_u128
        .checked_sub(protocol_fee)
        .unwrap()
        .checked_sub(platform_fee)
        .unwrap()
        .checked_sub(share_fee)
        .unwrap() as u64
}

pub fn get_amount_in(
    amount_out: u64,
    protocol_fee_rate: u128,
    platform_fee_rate: u128,
    share_fee_rate: u128,
    virtual_base: u128,
    virtual_quote: u128,
    real_base: u128,
    real_quote: u128,
    slippage_basis_points: u128,
) -> u64 {
    let amount_out_u128 = amount_out as u128;

    // 考虑滑点，实际需要的输出金额更高
    let amount_out_with_slippage = amount_out_u128 * 10000 / (10000 - slippage_basis_points);

    let input_reserve = virtual_quote.checked_add(real_quote).unwrap();
    let output_reserve = virtual_base.checked_sub(real_base).unwrap();

    // 根据 AMM 公式反推: amount_in_net = (amount_out * input_reserve) / (output_reserve - amount_out)
    let numerator = amount_out_with_slippage.checked_mul(input_reserve).unwrap();
    let denominator = output_reserve
        .checked_sub(amount_out_with_slippage)
        .unwrap();
    let amount_in_net = numerator.checked_div(denominator).unwrap();

    // 计算总费用率
    let total_fee_rate = protocol_fee_rate + platform_fee_rate + share_fee_rate;

    let amount_in = amount_in_net * 10000 / (10000 - total_fee_rate);

    amount_in as u64
}

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

#[cfg(test)]
mod tests {
    use crate::constants::bonk::accounts::{PLATFORM_FEE_RATE, PROTOCOL_FEE_RATE, SHARE_FEE_RATE};

    use super::*;

    #[test]
    fn test_amount_in_out_consistency() {
        // 测试参数
        let protocol_fee_rate = PROTOCOL_FEE_RATE;
        let platform_fee_rate = PLATFORM_FEE_RATE;
        let share_fee_rate = SHARE_FEE_RATE;
        let virtual_base = 1073025605596382;
        let virtual_quote = 30000852951;
        let real_base = 0;
        let real_quote = 0;
        let slippage_basis_points = 0;

        let original_amount_in = 2000000000;

        let geet_amount_out_result = get_amount_out(
            original_amount_in,
            protocol_fee_rate,
            platform_fee_rate,
            share_fee_rate,
            virtual_base,
            virtual_quote,
            real_base,
            real_quote,
            slippage_basis_points,
        );

        let amount_out = 25959582643397;
        let get_amount_in_result = get_amount_in(
            amount_out,
            protocol_fee_rate,
            platform_fee_rate,
            share_fee_rate,
            virtual_base,
            virtual_quote,
            real_base,
            real_quote,
            slippage_basis_points,
        );

        println!("Original amount_in: {}", original_amount_in);
        println!("Amount_out: {}", geet_amount_out_result);
        println!("Calculated amount_in: {}", get_amount_in_result);

        assert!(geet_amount_out_result == 66275810509273);
        assert!(get_amount_in_result == 753217040);
    }
}
