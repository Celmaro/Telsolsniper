use crate::{common::SolanaRpcClient, constants::raydium_cpmm::accounts};
use anyhow::anyhow;
use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, BorshDeserialize)]
pub struct Pool {
    pub amm_config: Pubkey,
    pub pool_creator: Pubkey,
    pub token0_vault: Pubkey,
    pub token1_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub token0_mint: Pubkey,
    pub token1_mint: Pubkey,
    pub token0_program: Pubkey,
    pub token1_program: Pubkey,
    pub observation_key: Pubkey,
    pub auth_bump: u8,
    pub status: u8,
    pub lp_mint_decimals: u8,
    pub mint0_decimals: u8,
    pub mint1_decimals: u8,
    pub lp_supply: u64,
    pub protocol_fees_token0: u64,
    pub protocol_fees_token1: u64,
    pub fund_fees_token0: u64,
    pub fund_fees_token1: u64,
    pub open_time: u64,
    pub recent_epoch: u64,
    pub padding: [u64; 31],
}

impl Pool {
    pub fn from_bytes(data: &[u8]) -> Result<Self, anyhow::Error> {
        let pool = Pool::try_from_slice(&data[8..])?;
        Ok(pool)
    }

    pub async fn fetch(
        rpc: &SolanaRpcClient,
        pool_address: &Pubkey,
    ) -> Result<Self, anyhow::Error> {
        let account = rpc.get_account(pool_address).await?;

        if account.owner != accounts::RAYDIUM_CPMM {
            return Err(anyhow!("Account is not owned by Raydium Cpmm program"));
        }

        Self::from_bytes(&account.data)
    }
}
