use std::sync::Arc;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};
use serde::Deserialize;
use crate::{constants::pumpfun::trade::{DEFAULT_BUY_TIP_FEE, DEFAULT_COMPUTE_UNIT_LIMIT, DEFAULT_COMPUTE_UNIT_PRICE, DEFAULT_RPC_UNIT_LIMIT, DEFAULT_RPC_UNIT_PRICE, DEFAULT_SELL_TIP_FEE}, swqos::{SwqosClient, SwqosConfig, SwqosRegion}};

#[derive(Debug, Clone)]
pub struct TradeConfig {
    pub rpc_url: String,
    pub swqos_configs: Vec<SwqosConfig>,
    pub priority_fee: PriorityFee,
    pub commitment: CommitmentConfig,
    pub lookup_table_key: Option<Pubkey>,
    pub use_rpc: bool,
}

impl TradeConfig {
    pub fn new(
        rpc_url: String, 
        swqos_configs: Vec<SwqosConfig>,
        priority_fee: PriorityFee, 
        commitment: CommitmentConfig, 
        lookup_table_key: Option<Pubkey>,
        use_rpc: bool,
    ) -> Self {
        Self { 
            rpc_url, 
            swqos_configs,
            priority_fee, 
            commitment, 
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
    pub smart_buy_tip_fee: f64,
    pub sell_tip_fee: f64,
}

impl Default for PriorityFee {
    fn default() -> Self {
        Self { 
            unit_limit: DEFAULT_COMPUTE_UNIT_LIMIT, 
            unit_price: DEFAULT_COMPUTE_UNIT_PRICE, 
            rpc_unit_limit: DEFAULT_RPC_UNIT_LIMIT,
            rpc_unit_price: DEFAULT_RPC_UNIT_PRICE,
            buy_tip_fee: DEFAULT_BUY_TIP_FEE, 
            buy_tip_fees: vec![],
            smart_buy_tip_fee: 0.0,
            sell_tip_fee: DEFAULT_SELL_TIP_FEE 
        }
    }
}

pub type SolanaRpcClient = solana_client::nonblocking::rpc_client::RpcClient;

pub struct MethodArgs {
    pub payer: Arc<Keypair>,
    pub rpc: Arc<RpcClient>,
    pub nonblocking_rpc: Arc<SolanaRpcClient>,
    pub jito_client: Arc<SwqosClient>,
}

impl MethodArgs {
    pub fn new(payer: Arc<Keypair>, rpc: Arc<RpcClient>, nonblocking_rpc: Arc<SolanaRpcClient>, jito_client: Arc<SwqosClient>) -> Self {
        Self { payer, rpc, nonblocking_rpc, jito_client }
    }
}

pub type AnyResult<T> = anyhow::Result<T>;
