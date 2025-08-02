use crate::common::SolanaRpcClient;
use crate::trading::pumpswap;
use solana_sdk::pubkey::Pubkey;

// Find a pool for a specific mint
pub async fn find_pool(rpc: &SolanaRpcClient, mint: &Pubkey) -> Result<Pubkey, anyhow::Error> {
    let (pool_address, _) = pumpswap::pool::Pool::find_by_mint(rpc, mint).await?;
    Ok(pool_address)
}

pub async fn get_token_amount(
    quote_mint_is_wsol: bool,
    pool_base_token_reserves: u64,
    pool_quote_token_reserves: u64,
    sol_amount: u64,
    lp_fee_basis_points: u64,
    protocol_fee_basis_points: u64,
    coin_creator_fee_basis_points: u64,
) -> Result<u64, anyhow::Error> {
    let product = pool_base_token_reserves as u128 * pool_quote_token_reserves as u128;
    if quote_mint_is_wsol {
        // base_amount_out
        let mut sol_amount = sol_amount as u128;
        sol_amount = sol_amount
            .checked_mul(10000)
            .unwrap()
            .checked_div(
                (10000
                    + lp_fee_basis_points
                    + protocol_fee_basis_points
                    + coin_creator_fee_basis_points) as u128,
            )
            .unwrap()
            .checked_sub(1)
            .unwrap();
        let new_quote_amount = pool_quote_token_reserves as u128 + sol_amount as u128;
        let new_base_amount = product / new_quote_amount;
        let token_amount = pool_base_token_reserves as u128 - new_base_amount;
        return Ok(token_amount as u64);
    } else {
        // min_quote_amount_out
        let new_base_amount = pool_base_token_reserves as u128 + sol_amount as u128;
        let new_quote_amount = product / new_base_amount;
        let token_amount = pool_quote_token_reserves as u128 - new_quote_amount;
        let lp_fee = token_amount
            .checked_mul(lp_fee_basis_points as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        let protocol_fee = token_amount
            .checked_mul(protocol_fee_basis_points as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        let coin_creator_fee = token_amount
            .checked_mul(coin_creator_fee_basis_points as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        let token_amount = token_amount.checked_sub(lp_fee).unwrap();
        let token_amount = token_amount.checked_sub(protocol_fee).unwrap();
        let token_amount = token_amount.checked_sub(coin_creator_fee).unwrap();
        return Ok(token_amount as u64);
    }
}

pub async fn get_wsol_amount(
    quote_mint_is_wsol: bool,
    pool_base_token_reserves: u64,
    pool_quote_token_reserves: u64,
    token_amount: u64,
    lp_fee_basis_points: u64,
    protocol_fee_basis_points: u64,
    coin_creator_fee_basis_points: u64,
) -> Result<u64, anyhow::Error> {
    let product = pool_base_token_reserves as u128 * pool_quote_token_reserves as u128;
    if !quote_mint_is_wsol {
        // base_amount_out
        let mut token_amount = token_amount as u128;
        token_amount = token_amount
            .checked_mul(10000)
            .unwrap()
            .checked_div(
                (10000
                    + lp_fee_basis_points
                    + protocol_fee_basis_points
                    + coin_creator_fee_basis_points) as u128,
            )
            .unwrap()
            .checked_sub(1)
            .unwrap();
        let new_quote_amount = pool_quote_token_reserves as u128 + token_amount as u128;
        let new_base_amount = product / new_quote_amount;
        let wsol_amount = pool_base_token_reserves as u128 - new_base_amount;
        Ok(wsol_amount as u64)
    } else {
        // min_quote_amount_out
        let new_base_amount = pool_base_token_reserves as u128 + token_amount as u128;
        let new_quote_amount = product / new_base_amount;
        let token_amount = pool_quote_token_reserves as u128 - new_quote_amount;
        let lp_fee = token_amount
            .checked_mul(lp_fee_basis_points as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        let protocol_fee = token_amount
            .checked_mul(protocol_fee_basis_points as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        let coin_creator_fee = token_amount
            .checked_mul(coin_creator_fee_basis_points as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        let wsol_amount = token_amount.checked_sub(lp_fee).unwrap();
        let wsol_amount = wsol_amount.checked_sub(protocol_fee).unwrap();
        let wsol_amount = wsol_amount.checked_sub(coin_creator_fee).unwrap();
        Ok(wsol_amount as u64)
    }
}


pub(crate) fn coin_creator_vault_authority(coin_creator: Pubkey) -> Pubkey {
    let (pump_pool_authority, _) = Pubkey::find_program_address(
        &[b"creator_vault", &coin_creator.to_bytes()],
        &crate::constants::pumpswap::accounts::AMM_PROGRAM,
    );
    pump_pool_authority
}

pub(crate) fn coin_creator_vault_ata(coin_creator: Pubkey, quote_mint: Pubkey) -> Pubkey {
    let creator_vault_authority = coin_creator_vault_authority(coin_creator);
    let associated_token_creator_vault_authority =
        spl_associated_token_account::get_associated_token_address_with_program_id(
            &creator_vault_authority,
            &quote_mint,
            &crate::constants::pumpswap::accounts::TOKEN_PROGRAM,
        );
    associated_token_creator_vault_authority
}

pub(crate) fn fee_recipient_ata(fee_recipient: Pubkey, quote_mint: Pubkey) -> Pubkey {
    let associated_token_fee_recipient =
        spl_associated_token_account::get_associated_token_address_with_program_id(
            &fee_recipient,
            &quote_mint,
            &crate::constants::pumpswap::accounts::TOKEN_PROGRAM,
        );
    associated_token_fee_recipient
}

pub fn get_user_volume_accumulator_pda(user: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[
        &crate::constants::pumpswap::seeds::USER_VOLUME_ACCUMULATOR_SEED,
        user.as_ref(),
    ];
    let program_id: &Pubkey = &&crate::constants::pumpswap::accounts::AMM_PROGRAM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

pub fn get_global_volume_accumulator_pda() -> Option<Pubkey> {
    let seeds: &[&[u8]; 1] = &[&crate::constants::pumpswap::seeds::GLOBAL_VOLUME_ACCUMULATOR_SEED];
    let program_id: &Pubkey = &&crate::constants::pumpswap::accounts::AMM_PROGRAM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}
