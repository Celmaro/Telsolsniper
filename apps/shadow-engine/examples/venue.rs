use sol_trade_sdk::instruction::utils::pumpfun::{fetch_bonding_curve_account, get_bonding_curve_pda};
use sol_trade_sdk::instruction::utils::pumpswap::find_pool;
use sol_trade_sdk::trading::core::params::PumpSwapParams;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Venue coverage probe: for each mint, report which venue holds it —
/// active Pump.fun bonding curve, graduated PumpSwap pool, or neither.
#[tokio::main]
async fn main() {
    let rpc = sol_trade_sdk::common::SolanaRpcClient::new(
        "https://api.mainnet-beta.solana.com".to_string(),
    );
    for m in std::env::args().skip(1) {
        let mint = match Pubkey::from_str(&m) {
            Ok(p) => p,
            Err(_) => {
                println!("{m} bad-pubkey");
                continue;
            }
        };
        if get_bonding_curve_pda(&mint).is_some() {
            if let Ok((c, _)) = fetch_bonding_curve_account(&rpc, &mint).await {
                if !c.complete {
                    println!(
                        "{m} PUMPFUN_CURVE price {} mcap {}",
                        c.get_token_price(),
                        c.get_market_cap_sol() as f64 / 1e9
                    );
                    continue;
                }
            }
        }
        match find_pool(&rpc, &mint).await {
            Ok(pool) => match PumpSwapParams::from_pool_address_by_rpc(&rpc, &pool).await {
                Ok(p) => {
                    let q = p.effective_quote_reserves().unwrap_or(0);
                    println!("{m} PUMPSWAP pool {pool} quote_reserves_sol {}", q as f64 / 1e9);
                }
                Err(e) => println!("{m} PUMPSWAP pool {pool} params-err {e}"),
            },
            Err(_) => println!("{m} NO_VENUE (neither curve nor pumpswap pool)"),
        }
    }
}
