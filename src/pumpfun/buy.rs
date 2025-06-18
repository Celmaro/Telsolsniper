use solana_hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;
use crate::{
    common::{PriorityFee, SolanaRpcClient},
    swqos::FeeClient,
    trading::{core::params::PumpFunParams, factory::Protocol, BuyParams, TradeFactory},
};
use crate::accounts::BondingCurveAccount;
const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 250000;

pub async fn buy(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    dev_buy_token: u64,
    dev_sol_cost: u64,
    buy_sol_cost: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    bonding_curve: Option<Arc<BondingCurveAccount>>,
    trade_type: String,
) -> Result<(), anyhow::Error> {
    // 创建执行器
    let executor = TradeFactory::create_executor(Protocol::PumpFun);
    // 创建协议特定参数
    let protocol_params = Box::new(PumpFunParams {
        dev_buy_token: dev_buy_token,
        dev_sol_cost: dev_sol_cost,
        trade_type: trade_type,
        bonding_curve: bonding_curve,
    });
    // 创建买入参数
    let buy_params = BuyParams {
        rpc: Some(rpc),
        payer,
        mint,
        creator,
        amount_sol: buy_sol_cost,
        slippage_basis_points: slippage_basis_points,
        priority_fee: priority_fee,
        lookup_table_key: lookup_table_key,
        recent_blockhash,
        data_size_limit: MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT,
        protocol_params,
    };
    // 执行买入
    executor.buy(buy_params).await?;
    Ok(())
}

pub async fn buy_with_tip(
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    dev_buy_token: u64,
    dev_sol_cost: u64,
    buy_sol_cost: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    bonding_curve: Option<Arc<BondingCurveAccount>>,
    trade_type: String,
) -> Result<(), anyhow::Error> {
    // 创建执行器
    let executor = TradeFactory::create_executor(Protocol::PumpFun);
    // 创建协议特定参数
    let protocol_params = Box::new(PumpFunParams {
        dev_buy_token: dev_buy_token,
        dev_sol_cost: dev_sol_cost,
        trade_type: trade_type,
        bonding_curve: bonding_curve,
    });
    // 创建买入参数
    let buy_params = BuyParams {
        rpc: None,
        payer,
        mint,
        creator,
        amount_sol: buy_sol_cost,
        slippage_basis_points: slippage_basis_points,
        priority_fee: priority_fee,
        lookup_table_key: lookup_table_key,
        recent_blockhash,
        data_size_limit: MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT,
        protocol_params,
    };
    let buy_with_tip_params = buy_params.with_tip(fee_clients);
    // 执行买入
    executor.buy_with_tip(buy_with_tip_params).await?;
    Ok(())
}
