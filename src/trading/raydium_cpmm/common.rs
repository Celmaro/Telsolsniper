use crate::{
    common::SolanaRpcClient,
    constants::{self, raydium_cpmm::accounts::WSOL_TOKEN_ACCOUNT},
    trading::raydium_cpmm::pool::Pool,
};
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;

pub fn get_pool_pda(amm_config: &Pubkey, mint1: &Pubkey, mint2: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 4] = &[
        constants::raydium_cpmm::seeds::POOL_SEED,
        amm_config.as_ref(),
        mint1.as_ref(),
        mint2.as_ref(),
    ];
    let program_id: &Pubkey = &constants::raydium_cpmm::accounts::RAYDIUM_CPMM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub fn get_vault_pda(pool_state: &Pubkey, mint: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 3] = &[
        constants::raydium_cpmm::seeds::POOL_VAULT_SEED,
        pool_state.as_ref(),
        mint.as_ref(),
    ];
    let program_id: &Pubkey = &constants::raydium_cpmm::accounts::RAYDIUM_CPMM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub fn get_observation_state_pda(pool_state: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[
        constants::raydium_cpmm::seeds::OBSERVATION_STATE_SEED,
        pool_state.as_ref(),
    ];
    let program_id: &Pubkey = &constants::raydium_cpmm::accounts::RAYDIUM_CPMM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub async fn get_buy_token_amount(
    rpc: &SolanaRpcClient,
    pool_state: &Pubkey,
    sol_amount: u64,
) -> Result<u64, anyhow::Error> {
    let pool = Pool::fetch(rpc, pool_state).await?;
    let is_token0_input = if pool.token0_mint == WSOL_TOKEN_ACCOUNT {
        true
    } else {
        false
    };
    let (token0_balance, token1_balance) =
        get_pool_token_balances(rpc, pool_state, &pool.token0_mint, &pool.token1_mint).await?;

    // 使用恒定乘积公式计算

    let (reserve_in, reserve_out) = if is_token0_input {
        (token0_balance, token1_balance)
    } else {
        (token1_balance, token0_balance)
    };

    if reserve_in == 0 || reserve_out == 0 {
        return Err(anyhow!("池子储备金为零，无法进行交换"));
    }

    // 使用 u128 防止溢出
    let amount_in_128 = sol_amount as u128;
    let reserve_in_128 = reserve_in as u128;
    let reserve_out_128 = reserve_out as u128;

    // 恒定乘积公式: amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
    let numerator = amount_in_128 * reserve_out_128;
    let denominator = reserve_in_128 + amount_in_128;

    if denominator == 0 {
        return Err(anyhow!("分母为零，计算错误"));
    }

    let amount_out = numerator / denominator;

    // 检查是否超出储备金
    if amount_out >= reserve_out_128 {
        return Err(anyhow!("输出数量超过池子储备金"));
    }

    Ok(amount_out as u64)
}

pub async fn get_sell_sol_amount(
    rpc: &SolanaRpcClient,
    pool_state: &Pubkey,
    token_amount: u64,
) -> Result<u64, anyhow::Error> {
    let pool = Pool::fetch(rpc, pool_state).await?;
    let is_token0_sol = if pool.token0_mint == WSOL_TOKEN_ACCOUNT {
        true
    } else {
        false
    };
    let (token0_balance, token1_balance) =
        get_pool_token_balances(rpc, pool_state, &pool.token0_mint, &pool.token1_mint).await?;

    let (reserve_in, reserve_out) = if is_token0_sol {
        (token1_balance, token0_balance)
    } else {
        (token0_balance, token1_balance)
    };

    if reserve_in == 0 || reserve_out == 0 {
        return Err(anyhow!("池子储备金为零，无法进行交换"));
    }

    // 使用 u128 防止溢出
    let amount_in_128 = token_amount as u128;
    let reserve_in_128 = reserve_in as u128;
    let reserve_out_128 = reserve_out as u128;

    // 恒定乘积公式: amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
    let numerator = amount_in_128 * reserve_out_128;
    let denominator = reserve_in_128 + amount_in_128;

    if denominator == 0 {
        return Err(anyhow!("分母为零，计算错误"));
    }

    let amount_out = numerator / denominator;

    // 检查是否超出储备金
    if amount_out >= reserve_out_128 {
        return Err(anyhow!("输出数量超过池子储备金"));
    }

    Ok(amount_out as u64)
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
    let token0_amount = token0_balance
        .amount
        .parse::<u64>()
        .map_err(|e| anyhow!("解析 token0 余额失败: {}", e))?;

    let token1_amount = token1_balance
        .amount
        .parse::<u64>()
        .map_err(|e| anyhow!("解析 token1 余额失败: {}", e))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey;

    #[test]
    fn test_get_pool_pda() {
        // 测试get_pool_pda函数
        let amm_config = constants::raydium_cpmm::accounts::AMM_CONFIG;
        let input_mint = pubkey!("So11111111111111111111111111111111111111112"); // WSOL
        let output_mint = pubkey!("BnwbwoqPm5ZNx7YTJ8g9jR2qCpYeHBC7xxpU8zEtbonk"); // USDC
        let pool_state = pubkey!("E9rRRpcdsKAseeLFbwC1Ewxd3aYG27meqwTTrMfCTbSG");
        let result = get_pool_pda(&amm_config, &input_mint, &output_mint);
        assert_eq!(result, Some(pool_state));
    }

    #[test]
    fn test_get_vault_pda() {
        // 测试get_vault_pda函数
        let pool_state = pubkey!("HBMkgQvt4NAFx6XzNav23bNcv6K3oiC5UfY3JsE22scY");
        let mint = pubkey!("DeESECsL3cLXno1LFquss98kNQSno1xpQC2ERCqSbonk"); // WSOL
        let vault_pda = pubkey!("7rkgNG3A8z636DuzhchKeqAJTaH3H5ZFWmBQeStydovA");
        let result = get_vault_pda(&pool_state, &mint);
        assert_eq!(result, Some(vault_pda));
    }

    #[test]
    fn test_get_observation_state_pda() {
        let pool_state = pubkey!("HBMkgQvt4NAFx6XzNav23bNcv6K3oiC5UfY3JsE22scY");
        let observation_state_pda = pubkey!("Gq8u9N18ASjq3AK2gCk6RtGSNyjXZf9EZDb6vTtB9JRs");
        let result = get_observation_state_pda(&pool_state);
        assert_eq!(result, Some(observation_state_pda));
    }
}
