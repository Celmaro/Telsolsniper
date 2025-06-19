use solana_hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;

use super::traits::ProtocolParams;
use crate::common::{PriorityFee, SolanaRpcClient};
use crate::swqos::FeeClient;
use crate::accounts::BondingCurveAccount;

/// 通用买入参数
#[derive(Clone)]
pub struct BuyParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub amount_sol: u64,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub data_size_limit: u32,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// 带MEV服务的买入参数
#[derive(Clone)]
pub struct BuyWithTipParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub fee_clients: Vec<Arc<FeeClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub amount_sol: u64,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub data_size_limit: u32,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// 通用卖出参数
#[derive(Clone)]
pub struct SellParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub amount_token: Option<u64>,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// 带MEV服务的卖出参数
#[derive(Clone)]
pub struct SellWithTipParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub fee_clients: Vec<Arc<FeeClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub amount_token: Option<u64>,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// PumpFun协议特定参数
#[derive(Clone)]
pub struct PumpFunParams {
    pub dev_buy_token: u64,
    pub dev_sol_cost: u64,
    pub trade_type: String,
    pub bonding_curve: Option<Arc<BondingCurveAccount>>,
}

impl ProtocolParams for PumpFunParams {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProtocolParams> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct PumpFunSellParams {}

impl ProtocolParams for PumpFunSellParams {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProtocolParams> {
        Box::new(self.clone())
    }
}

/// PumpSwap协议特定参数
#[derive(Clone)]
pub struct PumpSwapParams {
    pub pool: Option<Pubkey>,
    pub pool_base_token_account: Option<Pubkey>,
    pub pool_quote_token_account: Option<Pubkey>,
    pub user_base_token_account: Option<Pubkey>,
    pub user_quote_token_account: Option<Pubkey>,
}

impl ProtocolParams for PumpSwapParams {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProtocolParams> {
        Box::new(self.clone())
    }
}

impl BuyParams {
    /// 转换为BuyWithTipParams
    pub fn with_tip(self, fee_clients: Vec<Arc<FeeClient>>) -> BuyWithTipParams {
        BuyWithTipParams {
            rpc: self.rpc,
            fee_clients,
            payer: self.payer,
            mint: self.mint,
            creator: self.creator,
            amount_sol: self.amount_sol,
            slippage_basis_points: self.slippage_basis_points,
            priority_fee: self.priority_fee,
            lookup_table_key: self.lookup_table_key,
            recent_blockhash: self.recent_blockhash,
            data_size_limit: self.data_size_limit,
            protocol_params: self.protocol_params,
        }
    }
}

impl SellParams {
    /// 转换为SellWithTipParams
    pub fn with_tip(self, fee_clients: Vec<Arc<FeeClient>>) -> SellWithTipParams {
        SellWithTipParams {
            rpc: self.rpc,
            fee_clients,
            payer: self.payer,
            mint: self.mint,
            creator: self.creator,
            amount_token: self.amount_token,
            slippage_basis_points: self.slippage_basis_points,
            priority_fee: self.priority_fee,
            lookup_table_key: self.lookup_table_key,
            recent_blockhash: self.recent_blockhash,
            protocol_params: self.protocol_params,
        }
    }
}
