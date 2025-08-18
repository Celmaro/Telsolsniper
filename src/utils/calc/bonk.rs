use crate::constants::bonk::accounts;

/// Calculates the amount of tokens to receive when buying with SOL
///
/// This function implements the constant product formula (x * y = k) for token swaps,
/// taking into account various fees and slippage protection.
///
/// # Arguments
///
/// * `amount_in` - The amount of SOL to spend (in lamports)
/// * `virtual_base` - Virtual base token reserves
/// * `virtual_quote` - Virtual quote token (SOL) reserves
/// * `real_base` - Real base token reserves
/// * `real_quote` - Real quote token (SOL) reserves
/// * `slippage_basis_points` - Maximum slippage tolerance in basis points (e.g., 100 = 1%)
///
/// # Returns
///
/// The minimum amount of tokens that will be received after fees and slippage
pub fn get_buy_token_amount_from_sol_amount(
    amount_in: u64,
    virtual_base: u128,
    virtual_quote: u128,
    real_base: u128,
    real_quote: u128,
    slippage_basis_points: u128,
) -> u64 {
    let amount_in_u128 = amount_in as u128;

    // Calculate various fees deducted from input amount
    let protocol_fee = (amount_in_u128 * accounts::PROTOCOL_FEE_RATE / 10000) as u128;
    let platform_fee = (amount_in_u128 * accounts::PLATFORM_FEE_RATE / 10000) as u128;
    let share_fee = (amount_in_u128 * accounts::SHARE_FEE_RATE / 10000) as u128;

    // Calculate net input amount after deducting all fees
    let amount_in_net = amount_in_u128
        .checked_sub(protocol_fee)
        .unwrap()
        .checked_sub(platform_fee)
        .unwrap()
        .checked_sub(share_fee)
        .unwrap();

    // Calculate total reserves (virtual + real)
    let input_reserve = virtual_quote.checked_add(real_quote).unwrap();
    let output_reserve = virtual_base.checked_sub(real_base).unwrap();

    // Apply constant product formula: amount_out = (amount_in * output_reserve) / (input_reserve + amount_in)
    let numerator = amount_in_net.checked_mul(output_reserve).unwrap();
    let denominator = input_reserve.checked_add(amount_in_net).unwrap();
    let mut amount_out = numerator.checked_div(denominator).unwrap();

    // Apply slippage protection
    amount_out = amount_out - (amount_out * slippage_basis_points) / 10000;
    amount_out as u64
}

/// Calculates the amount of SOL to receive when selling tokens
///
/// This function implements the constant product formula (x * y = k) for token swaps,
/// calculating the SOL output for a given token input amount, accounting for fees and slippage.
///
/// # Arguments
///
/// * `amount_in` - The amount of tokens to sell
/// * `virtual_base` - Virtual base token reserves
/// * `virtual_quote` - Virtual quote token (SOL) reserves
/// * `real_base` - Real base token reserves
/// * `real_quote` - Real quote token (SOL) reserves
/// * `slippage_basis_points` - Maximum slippage tolerance in basis points (e.g., 100 = 1%)
///
/// # Returns
///
/// The minimum amount of SOL that will be received after fees and slippage
pub fn get_sell_sol_amount_from_token_amount(
    amount_in: u64,
    virtual_base: u128,
    virtual_quote: u128,
    real_base: u128,
    real_quote: u128,
    slippage_basis_points: u128,
) -> u64 {
    let amount_in_u128 = amount_in as u128;

    // For sell operation, input_reserve is token reserves, output_reserve is SOL reserves
    let input_reserve = virtual_base.checked_sub(real_base).unwrap();
    let output_reserve = virtual_quote.checked_add(real_quote).unwrap();

    // Use constant product formula to calculate SOL amount received from selling tokens
    let numerator = amount_in_u128.checked_mul(output_reserve).unwrap();
    let denominator = input_reserve.checked_add(amount_in_u128).unwrap();
    let sol_amount_out = numerator.checked_div(denominator).unwrap();

    // Calculate various fees
    let protocol_fee = (sol_amount_out * accounts::PROTOCOL_FEE_RATE / 10000) as u128;
    let platform_fee = (sol_amount_out * accounts::PLATFORM_FEE_RATE / 10000) as u128;
    let share_fee = (sol_amount_out * accounts::SHARE_FEE_RATE / 10000) as u128;

    // Net SOL amount after deducting fees
    let sol_amount_net = sol_amount_out
        .checked_sub(protocol_fee)
        .unwrap()
        .checked_sub(platform_fee)
        .unwrap()
        .checked_sub(share_fee)
        .unwrap();

    // Apply slippage protection
    let final_amount = sol_amount_net - (sol_amount_net * slippage_basis_points) / 10000;

    final_amount as u64
}
