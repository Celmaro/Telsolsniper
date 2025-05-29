use std::sync::Arc;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};
use serde::Deserialize;
use crate::{constants::trade::{DEFAULT_BUY_TIP_FEE, DEFAULT_COMPUTE_UNIT_LIMIT, DEFAULT_COMPUTE_UNIT_PRICE, DEFAULT_SELL_TIP_FEE}, swqos::FeeClient};

#[derive(Debug, Clone, PartialEq)]
pub enum FeeType {
    Jito,
    NextBlock,
}

#[derive(Debug, Clone)]
pub struct Cluster {
    pub rpc_url: String,
    pub block_engine_url: String,
    pub nextblock_url: String,
    pub nextblock_auth_token: String,
    pub zeroslot_url: String,
    pub zeroslot_auth_token: String,
    pub nozomi_url: String,
    pub nozomi_auth_token: String,
    pub use_jito: bool,
    pub use_nextblock: bool,
    pub use_zeroslot: bool,
    pub use_nozomi: bool,
    pub priority_fee: PriorityFee,
    pub commitment: CommitmentConfig,
    pub lookup_table_key: Option<Pubkey>,
    pub use_rpc: bool,
}

impl Cluster {
    pub fn new(
        rpc_url: String, 
        block_engine_url: 
        String, nextblock_url: 
        String, nextblock_auth_token: 
        String, zeroslot_url: String, 
        zeroslot_auth_token: String, 
        nozomi_url: String, 
        nozomi_auth_token: String, 
        priority_fee: PriorityFee, 
        commitment: CommitmentConfig, 
        use_jito: bool, 
        use_nextblock: bool, 
        use_zeroslot: bool, 
        use_nozomi: bool,
        lookup_table_key: Option<Pubkey>,
        use_rpc: bool,
    ) -> Self {
        Self { 
            rpc_url, 
            block_engine_url, 
            nextblock_url, 
            nextblock_auth_token, 
            zeroslot_url, 
            zeroslot_auth_token, 
            nozomi_url, 
            nozomi_auth_token, 
            priority_fee, 
            commitment, 
            use_jito, 
            use_nextblock, 
            use_zeroslot, 
            use_nozomi,
            lookup_table_key,
            use_rpc,
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PriorityFee {
    pub unit_limit: u32,
    pub unit_price: u64,
    pub rpc_unit_limit: u32,
    pub rpc_unit_price: u64,
    pub buy_tip_fee: f64,
    pub buy_tip_fees: Vec<f64>,
    pub sell_tip_fee: f64,
}

impl Default for PriorityFee {
    fn default() -> Self {
        Self { 
            unit_limit: DEFAULT_COMPUTE_UNIT_LIMIT, 
            unit_price: DEFAULT_COMPUTE_UNIT_PRICE, 
            rpc_unit_limit: 0,
            rpc_unit_price: 0,
            buy_tip_fee: DEFAULT_BUY_TIP_FEE, 
            buy_tip_fees: vec![],
            sell_tip_fee: DEFAULT_SELL_TIP_FEE 
        }
    }
}

pub type SolanaRpcClient = solana_client::nonblocking::rpc_client::RpcClient;

pub struct MethodArgs {
    pub payer: Arc<Keypair>,
    pub rpc: Arc<RpcClient>,
    pub nonblocking_rpc: Arc<SolanaRpcClient>,
    pub jito_client: Arc<FeeClient>,
}

impl MethodArgs {
    pub fn new(payer: Arc<Keypair>, rpc: Arc<RpcClient>, nonblocking_rpc: Arc<SolanaRpcClient>, jito_client: Arc<FeeClient>) -> Self {
        Self { payer, rpc, nonblocking_rpc, jito_client }
    }
}

pub type AnyResult<T> = anyhow::Result<T>;

