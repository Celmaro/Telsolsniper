use crate::{common::SolanaRpcClient, constants::raydium_launchpad::accounts};
use anyhow::anyhow;
use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, BorshDeserialize)]
pub struct VestingSchedule {
    pub total_locked_amount: u64,
    pub cliff_period: u64,
    pub unlock_period: u64,
    pub start_time: u64,
    pub allocated_share_amount: u64,
}

#[derive(Debug, Clone, BorshDeserialize)]
pub struct Pool {
    pub epoch: u64,
    pub auth_bump: u8,
    pub status: u8,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub migrate_type: u8,
    pub supply: u64,
    pub total_base_sell: u64,
    pub virtual_base: u64,
    pub virtual_quote: u64,
    pub real_base: u64,
    pub real_quote: u64,
    pub total_quote_fund_raising: u64,
    pub quote_protocol_fee: u64,
    pub platform_fee: u64,
    pub migrate_fee: u64,
    pub vesting_schedule: VestingSchedule,
    pub global_config: Pubkey,
    pub platform_config: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub creator: Pubkey,
    pub padding: [u64; 8],
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

        if account.owner != accounts::LAUNCHPAD_PROGRAM {
            return Err(anyhow!("Account is not owned by RaydiumLaunchpad program"));
        }

        Self::from_bytes(&account.data)
    }
}
