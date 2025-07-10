use crate::common::SolanaRpcClient;
use crate::trading::pumpswap;
use solana_sdk::pubkey::Pubkey;

// Find a pool for a specific mint
pub async fn find_pool(rpc: &SolanaRpcClient, mint: &Pubkey) -> Result<Pubkey, anyhow::Error> {
    let (pool_address, _) = pumpswap::pool::Pool::find_by_mint(rpc, mint).await?;
    Ok(pool_address)
}

// Calculate the amount of tokens to receive for a given SOL amount
pub async fn get_buy_token_amount(
    rpc: &SolanaRpcClient,
    pool: &Pubkey,
    sol_amount: u64,
) -> Result<u64, anyhow::Error> {
    let pool_data = pumpswap::pool::Pool::fetch(rpc, pool).await?;
    pool_data.calculate_buy_amount(rpc, sol_amount).await
}

// Calculate the amount of SOL to receive for a given token amount
pub async fn get_sell_sol_amount(
    rpc: &SolanaRpcClient,
    pool: &Pubkey,
    token_amount: u64,
) -> Result<u64, anyhow::Error> {
    let pool_data = pumpswap::pool::Pool::fetch(rpc, pool).await?;
    pool_data.calculate_sell_amount(rpc, token_amount).await
}

pub(crate) fn coin_creator_vault_authority(coin_creator: Pubkey) -> Pubkey {
    let (pump_pool_authority, _) = Pubkey::find_program_address(
        &[b"creator_vault", &coin_creator.to_bytes()],
        &crate::constants::pumpswap::accounts::AMM_PROGRAM,
    );
    pump_pool_authority
}

pub(crate) fn coin_creator_vault_ata(coin_creator: Pubkey) -> Pubkey {
    let creator_vault_authority = coin_creator_vault_authority(coin_creator);
    let associated_token_creator_vault_authority =
        spl_associated_token_account::get_associated_token_address_with_program_id(
            &creator_vault_authority,
            &crate::constants::pumpswap::accounts::WSOL_TOKEN_ACCOUNT,
            &crate::constants::pumpswap::accounts::TOKEN_PROGRAM,
        );
    associated_token_creator_vault_authority
}
