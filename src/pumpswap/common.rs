use anyhow::anyhow;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use crate::common::SolanaRpcClient;

// Calculate slippage for buy operations
pub fn calculate_with_slippage_buy(amount: u64, basis_points: u64) -> u64 {
    amount + (amount * basis_points / 10000)
}

// Calculate slippage for sell operations
pub fn calculate_with_slippage_sell(amount: u64, basis_points: u64) -> u64 {
    if amount <= basis_points / 10000 {
        1
    } else {
        amount - (amount * basis_points / 10000)
    }
}

// Get token balance for a specific mint and owner
pub async fn get_token_balance(
    rpc: &SolanaRpcClient,
    owner: &Keypair,
    mint: &Pubkey,
) -> Result<(u64, Pubkey), anyhow::Error> {
    let ata = spl_associated_token_account::get_associated_token_address(&owner.pubkey(), mint);

    match rpc.get_token_account_balance(&ata).await {
        Ok(balance) => {
            let amount = balance.amount.parse::<u64>().map_err(|e| anyhow!(e))?;
            Ok((amount, ata))
        }
        Err(_) => Ok((0, ata)),
    }
}

// Find a pool for a specific mint
pub async fn find_pool(
    rpc: &SolanaRpcClient,
    mint: &Pubkey,
) -> Result<Pubkey, anyhow::Error> {
    let (pool_address, _) = crate::pumpswap::pool::Pool::find_by_mint(rpc, mint).await?;
    Ok(pool_address)
}

// Calculate the amount of tokens to receive for a given SOL amount
pub async fn get_buy_token_amount(
    rpc: &SolanaRpcClient,
    pool: &Pubkey,
    sol_amount: u64,
) -> Result<u64, anyhow::Error> {
    let pool_data = crate::pumpswap::pool::Pool::fetch(rpc, pool).await?;
    pool_data.calculate_buy_amount(rpc, sol_amount).await
}

// Calculate the amount of SOL to receive for a given token amount
pub async fn get_sell_sol_amount(
    rpc: &SolanaRpcClient,
    pool: &Pubkey,
    token_amount: u64,
) -> Result<u64, anyhow::Error> {
    let pool_data = crate::pumpswap::pool::Pool::fetch(rpc, pool).await?;
    pool_data.calculate_sell_amount(rpc, token_amount).await
}
