use crate::instruction::utils::bonk::accounts;
use crate::trading::core::params::BonkParams;

use super::common::clamp_slippage_basis_points_u128;

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
/// * `slippage_basis_points` - Maximum slippage tolerance in basis points (e.g., 100 = 1%).
///   Clamped to [`MAX_SLIPPAGE_BASIS_POINTS`](super::common::MAX_SLIPPAGE_BASIS_POINTS) (9999 = 99.99%).
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
    let bps = clamp_slippage_basis_points_u128(slippage_basis_points);

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

    // Apply slippage protection (bps already clamped)
    amount_out = amount_out - (amount_out * bps) / 10000;
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
/// * `slippage_basis_points` - Maximum slippage tolerance in basis points (e.g., 100 = 1%).
///   Clamped to [`MAX_SLIPPAGE_BASIS_POINTS`](super::common::MAX_SLIPPAGE_BASIS_POINTS) (9999 = 99.99%).
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
    let bps = clamp_slippage_basis_points_u128(slippage_basis_points);

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

    // Apply slippage protection (bps already clamped)
    let final_amount = sol_amount_net - (sol_amount_net * bps) / 10000;

    final_amount as u64
}

const FEE_RATE_DENOMINATOR: u128 = 1_000_000;

fn current_total_fee_rate(params: &BonkParams, share_fee_rate: u64) -> Result<u128, anyhow::Error> {
    let rate = (params.trade_fee_rate as u128)
        .checked_add(params.platform_fee_rate as u128)
        .and_then(|value| value.checked_add(params.creator_fee_rate as u128))
        .and_then(|value| value.checked_add(share_fee_rate as u128))
        .ok_or_else(|| anyhow::anyhow!("LaunchLab fee rate overflow"))?;
    if rate > FEE_RATE_DENOMINATOR {
        return Err(anyhow::anyhow!("LaunchLab total fee rate exceeds 1,000,000"));
    }
    Ok(rate)
}

fn fee_ceil(amount: u128, rate: u128) -> u128 {
    amount.saturating_mul(rate).div_ceil(FEE_RATE_DENOMINATOR)
}

fn pre_fee_amount(post_fee_amount: u128, fee_rate: u128) -> Result<u128, anyhow::Error> {
    let denominator = FEE_RATE_DENOMINATOR
        .checked_sub(fee_rate)
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab total fee rate must be below 1,000,000"))?;
    post_fee_amount
        .checked_mul(FEE_RATE_DENOMINATOR)
        .map(|value| value.div_ceil(denominator))
        .ok_or_else(|| anyhow::anyhow!("LaunchLab pre-fee input overflow"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchLabBuyQuote {
    /// Actual user input. This can be lower than requested at the graduation boundary.
    pub amount_in: u64,
    pub minimum_amount_out: u64,
}

/// Quotes a current constant-product LaunchLab exact-input buy.
pub fn get_buy_quote(
    amount_in: u64,
    params: &BonkParams,
    share_fee_rate: u64,
    slippage_basis_points: u128,
) -> Result<LaunchLabBuyQuote, anyhow::Error> {
    if params.curve_type != 0 {
        return Err(anyhow::anyhow!("Unsupported LaunchLab curve type: {}", params.curve_type));
    }
    let total_fee_rate = current_total_fee_rate(params, share_fee_rate)?;
    let quote_transfer_fee = params.quote_transfer_fee.calculate(amount_in);
    let vault_input = amount_in
        .checked_sub(quote_transfer_fee)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab quote transfer fee exceeds input"))?
        as u128;
    let fee = fee_ceil(vault_input, total_fee_rate);
    let curve_input = vault_input
        .checked_sub(fee)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab fees exceed input"))?;
    let input_reserve = params
        .virtual_quote
        .checked_add(params.real_quote)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab quote reserve overflow"))?;
    let output_reserve = params
        .virtual_base
        .checked_sub(params.real_base)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab base reserve underflow"))?;
    let denominator = input_reserve
        .checked_add(curve_input)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab buy denominator overflow"))?;
    let quoted_output = curve_input
        .checked_mul(output_reserve)
        .and_then(|value| value.checked_div(denominator))
        .ok_or_else(|| anyhow::anyhow!("Failed to quote LaunchLab buy"))?;

    let (actual_amount_in, gross_output) = if params.total_base_sell == 0 {
        (amount_in, quoted_output)
    } else {
        let remaining_base = params
            .total_base_sell
            .checked_sub(params.real_base)
            .ok_or_else(|| anyhow::anyhow!("LaunchLab sold amount exceeds total base sell"))?;
        if quoted_output <= remaining_base {
            (amount_in, quoted_output)
        } else {
            let output_after_buy = output_reserve
                .checked_sub(remaining_base)
                .filter(|value| *value > 0)
                .ok_or_else(|| anyhow::anyhow!("LaunchLab graduation output exhausts reserve"))?;
            let required_curve_input = input_reserve
                .checked_mul(remaining_base)
                .map(|value| value.div_ceil(output_after_buy))
                .ok_or_else(|| anyhow::anyhow!("LaunchLab graduation input overflow"))?;
            let required_vault_input = pre_fee_amount(required_curve_input, total_fee_rate)?;
            let required_vault_input = u64::try_from(required_vault_input)
                .map_err(|_| anyhow::anyhow!("LaunchLab graduation input exceeds u64"))?;
            let inverse_transfer_fee =
                params.quote_transfer_fee.calculate_inverse(required_vault_input);
            let actual_amount_in = required_vault_input
                .checked_add(inverse_transfer_fee)
                .ok_or_else(|| anyhow::anyhow!("LaunchLab transfer-fee input overflow"))?;
            (actual_amount_in.min(amount_in), remaining_base)
        }
    };
    let gross_output = u64::try_from(gross_output)
        .map_err(|_| anyhow::anyhow!("LaunchLab buy output exceeds u64"))?;
    let received = gross_output.saturating_sub(params.base_transfer_fee.calculate(gross_output));
    let bps = clamp_slippage_basis_points_u128(slippage_basis_points);
    let minimum_amount_out = (received as u128 - (received as u128 * bps) / 10_000) as u64;
    Ok(LaunchLabBuyQuote { amount_in: actual_amount_in, minimum_amount_out })
}

/// Returns the minimum output for a current constant-product LaunchLab exact-input buy.
pub fn get_buy_min_amount_out(
    amount_in: u64,
    params: &BonkParams,
    share_fee_rate: u64,
    slippage_basis_points: u128,
) -> Result<u64, anyhow::Error> {
    Ok(get_buy_quote(amount_in, params, share_fee_rate, slippage_basis_points)?.minimum_amount_out)
}

/// Quotes a current constant-product LaunchLab exact-input sell.
pub fn get_sell_min_amount_out(
    amount_in: u64,
    params: &BonkParams,
    share_fee_rate: u64,
    slippage_basis_points: u128,
) -> Result<u64, anyhow::Error> {
    if params.curve_type != 0 {
        return Err(anyhow::anyhow!("Unsupported LaunchLab curve type: {}", params.curve_type));
    }
    let base_transfer_fee = params.base_transfer_fee.calculate(amount_in);
    let curve_input = amount_in
        .checked_sub(base_transfer_fee)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab base transfer fee exceeds input"))?
        as u128;
    let input_reserve = params
        .virtual_base
        .checked_sub(params.real_base)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab base reserve underflow"))?;
    let output_reserve = params
        .virtual_quote
        .checked_add(params.real_quote)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab quote reserve overflow"))?;
    let denominator = input_reserve
        .checked_add(curve_input)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab sell denominator overflow"))?;
    let gross_output = curve_input
        .checked_mul(output_reserve)
        .and_then(|value| value.checked_div(denominator))
        .ok_or_else(|| anyhow::anyhow!("Failed to quote LaunchLab sell"))?;
    let fee = fee_ceil(gross_output, current_total_fee_rate(params, share_fee_rate)?);
    let vault_output = gross_output
        .checked_sub(fee)
        .ok_or_else(|| anyhow::anyhow!("LaunchLab fees exceed output"))?;
    let vault_output = u64::try_from(vault_output)
        .map_err(|_| anyhow::anyhow!("LaunchLab sell output exceeds u64"))?;
    let received = vault_output.saturating_sub(params.quote_transfer_fee.calculate(vault_output));
    let bps = clamp_slippage_basis_points_u128(slippage_basis_points);
    Ok((received as u128 - (received as u128 * bps) / 10_000) as u64)
}

#[cfg(test)]
mod tests {
    use super::super::common::MAX_SLIPPAGE_BASIS_POINTS;
    use super::*;

    // Matches defaults used by BonkParams::from_dev_trade
    const VIRTUAL_BASE: u128 = 1_073_025_605_596_382;
    const VIRTUAL_QUOTE: u128 = 30_000_852_951;

    #[test]
    fn buy_slippage_at_10000_bps_does_not_zero_min_out() {
        let with_max = get_buy_token_amount_from_sol_amount(
            1_000_000,
            VIRTUAL_BASE,
            VIRTUAL_QUOTE,
            0,
            0,
            10_000,
        );
        let with_clamp_cap = get_buy_token_amount_from_sol_amount(
            1_000_000,
            VIRTUAL_BASE,
            VIRTUAL_QUOTE,
            0,
            0,
            MAX_SLIPPAGE_BASIS_POINTS as u128,
        );
        assert_eq!(with_max, with_clamp_cap);
        assert!(with_max > 0);
    }

    #[test]
    fn sell_slippage_above_10000_bps_does_not_underflow() {
        let with_overflow = get_sell_sol_amount_from_token_amount(
            1_000_000_000,
            VIRTUAL_BASE,
            VIRTUAL_QUOTE,
            0,
            0,
            50_000,
        );
        let with_clamp_cap = get_sell_sol_amount_from_token_amount(
            1_000_000_000,
            VIRTUAL_BASE,
            VIRTUAL_QUOTE,
            0,
            0,
            MAX_SLIPPAGE_BASIS_POINTS as u128,
        );
        assert_eq!(with_overflow, with_clamp_cap);
        assert!(with_overflow > 0);
    }

    #[test]
    fn normal_slippage_matches_uncapped_formula() {
        const BPS: u128 = 100;

        let buy_no_slip =
            get_buy_token_amount_from_sol_amount(1_000_000, VIRTUAL_BASE, VIRTUAL_QUOTE, 0, 0, 0);
        let buy_with_slip =
            get_buy_token_amount_from_sol_amount(1_000_000, VIRTUAL_BASE, VIRTUAL_QUOTE, 0, 0, BPS);
        let expected_buy = buy_no_slip - ((buy_no_slip as u128 * BPS) / 10_000) as u64;
        assert_eq!(buy_with_slip, expected_buy);
        assert!(buy_with_slip < buy_no_slip);

        let sell_no_slip = get_sell_sol_amount_from_token_amount(
            1_000_000_000,
            VIRTUAL_BASE,
            VIRTUAL_QUOTE,
            0,
            0,
            0,
        );
        let sell_with_slip = get_sell_sol_amount_from_token_amount(
            1_000_000_000,
            VIRTUAL_BASE,
            VIRTUAL_QUOTE,
            0,
            0,
            BPS,
        );
        let expected_sell = sell_no_slip - ((sell_no_slip as u128 * BPS) / 10_000) as u64;
        assert_eq!(sell_with_slip, expected_sell);
        assert!(sell_with_slip < sell_no_slip);
    }

    #[test]
    fn launchlab_buy_reduces_input_at_graduation_boundary() {
        let params = BonkParams {
            virtual_base: 2_000,
            virtual_quote: 1_000,
            real_base: 900,
            real_quote: 0,
            total_base_sell: 1_000,
            trade_fee_rate: 2_500,
            platform_fee_rate: 10_000,
            quote_transfer_fee: crate::trading::core::params::TokenTransferFee {
                basis_points: 300,
                maximum_fee: 1_000_000,
            },
            ..Default::default()
        };

        let quote = get_buy_quote(10_000, &params, 0, 0).expect("graduation quote");

        assert_eq!(quote.minimum_amount_out, 100);
        assert!(quote.amount_in < 10_000);
        let received_by_vault =
            quote.amount_in - params.quote_transfer_fee.calculate(quote.amount_in);
        let fee = fee_ceil(received_by_vault as u128, current_total_fee_rate(&params, 0).unwrap());
        let curve_input = received_by_vault as u128 - fee;
        assert_eq!(curve_input, 100);
    }
}
