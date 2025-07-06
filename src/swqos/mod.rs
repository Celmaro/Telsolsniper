pub mod common;
pub mod solana_rpc;
pub mod jito;
pub mod nextblock;
pub mod zeroslot;
pub mod temporal;
pub mod bloxroute;

use solana_sdk::transaction::VersionedTransaction;
use tokio::sync::RwLock;

use anyhow::Result;

use crate::constants::swqos::{SWQOS_ENDPOINTS_BLOX, SWQOS_ENDPOINTS_JITO, SWQOS_ENDPOINTS_NEXTBLOCK, SWQOS_ENDPOINTS_TEMPORAL, SWQOS_ENDPOINTS_ZERO_SLOT};

lazy_static::lazy_static! {
    static ref TIP_ACCOUNT_CACHE: RwLock<Vec<String>> = RwLock::new(Vec::new());
}

#[derive(Debug, Clone, Copy)]
pub enum TradeType {
    Create,
    CreateAndBuy,
    Buy,
    Sell,
}

impl std::fmt::Display for TradeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TradeType::Create => "创建",
            TradeType::CreateAndBuy => "创建并买入",
            TradeType::Buy => "买入",
            TradeType::Sell => "卖出",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwqosType {
    Jito,
    NextBlock,
    ZeroSlot,
    Temporal,
    Bloxroute,
    Rpc,
}

pub type SwqosClient = dyn SwqosClientTrait + Send + Sync + 'static;

#[async_trait::async_trait]
pub trait SwqosClientTrait {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<()>;
    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<()>;
    fn get_tip_account(&self) -> Result<String>;
    fn get_swqos_type(&self) -> SwqosType;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SwqosRegion {
    NewYork,
    Frankfurt,
    Amsterdam,
    SLC,
    Tokyo,
    London,
    LosAngeles,
    Default,
}

#[derive(Debug, Clone)]
pub struct SwqosConfig {
    pub endpoint: String,
    pub auth_token: String,
    pub swqos_type: SwqosType,
}

impl SwqosConfig {
    pub fn new(endpoint: Option<String>, auth_token: Option<String>, swqos_type: SwqosType, region: SwqosRegion) -> Self {
        let auth_token = auth_token.unwrap_or_else(|| "".to_string());
        let endpoint = endpoint.unwrap_or_else(|| match swqos_type {
            SwqosType::Jito => SWQOS_ENDPOINTS_JITO[region as usize].to_string(),
            SwqosType::NextBlock => SWQOS_ENDPOINTS_NEXTBLOCK[region as usize].to_string(),
            SwqosType::ZeroSlot => SWQOS_ENDPOINTS_ZERO_SLOT[region as usize].to_string(),
            SwqosType::Temporal => SWQOS_ENDPOINTS_TEMPORAL[region as usize].to_string(),
            SwqosType::Bloxroute => SWQOS_ENDPOINTS_BLOX[region as usize].to_string(),
            SwqosType::Rpc => "".to_string(),
        });

        Self { endpoint, auth_token, swqos_type }
    }
}