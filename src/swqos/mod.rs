
pub mod common;
pub mod solana_rpc;
pub mod jito;
pub mod nextblock;
pub mod zeroslot;
pub mod nozomi;
pub mod types;

use solana_sdk::signature::Signature;
use solana_sdk::transaction::VersionedTransaction;
use tokio::sync::RwLock;

use anyhow::Result;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClientType {
    Jito,
    NextBlock,
    ZeroSlot,
    Nozomi,
    Rpc,
}

pub type SwqosClient = dyn SwqosClientTrait + Send + Sync + 'static;

#[async_trait::async_trait]
pub trait SwqosClientTrait {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature>;
    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>>;
    fn get_tip_account(&self) -> Result<String>;
    fn get_client_type(&self) -> ClientType;
}