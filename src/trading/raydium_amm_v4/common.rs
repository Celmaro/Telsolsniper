use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::event_parser::protocols::raydium_amm_v4::types::{
    amm_info_decode, AmmInfo,
};

use crate::common::SolanaRpcClient;

pub async fn fetch_amm_info(rpc: &SolanaRpcClient, amm: Pubkey) -> Result<AmmInfo, anyhow::Error> {
    let amm_info = rpc.get_account_data(&amm).await?;
    let amm_info =
        amm_info_decode(&amm_info).ok_or_else(|| anyhow!("Failed to decode amm info"))?;
    Ok(amm_info)
}
