use crate::common::SolanaRpcClient;
use crate::trading::common::get_multi_token_balances;
use solana_sdk::pubkey::Pubkey;

/// RaydiumCpmm protocol specific parameters
/// Configuration parameters specific to Raydium CPMM trading protocol
#[derive(Clone)]
pub struct RaydiumAmmV4Params {
    /// AMM pool address
    pub amm: Pubkey,
    /// Base token (coin) mint address
    pub coin_mint: Pubkey,
    /// Quote token (pc) mint address  
    pub pc_mint: Pubkey,
    /// Pool's coin token account address
    pub token_coin: Pubkey,
    /// Pool's pc token account address
    pub token_pc: Pubkey,
    /// Current coin reserve amount in the pool
    pub coin_reserve: u64,
    /// Current pc reserve amount in the pool
    pub pc_reserve: u64,
}

impl RaydiumAmmV4Params {
    pub fn new(
        amm: Pubkey,
        coin_mint: Pubkey,
        pc_mint: Pubkey,
        token_coin: Pubkey,
        token_pc: Pubkey,
        coin_reserve: u64,
        pc_reserve: u64,
    ) -> Self {
        Self { amm, coin_mint, pc_mint, token_coin, token_pc, coin_reserve, pc_reserve }
    }
    pub async fn from_amm_address_by_rpc(
        rpc: &SolanaRpcClient,
        amm: Pubkey,
    ) -> Result<Self, anyhow::Error> {
        let amm_info = crate::instruction::utils::raydium_amm_v4::fetch_amm_info(rpc, amm).await?;
        let (coin_reserve, pc_reserve) =
            get_multi_token_balances(rpc, &amm_info.token_coin, &amm_info.token_pc).await?;
        Ok(Self {
            amm,
            coin_mint: amm_info.coin_mint,
            pc_mint: amm_info.pc_mint,
            token_coin: amm_info.token_coin,
            token_pc: amm_info.token_pc,
            coin_reserve,
            pc_reserve,
        })
    }
}
