use crate::trading::{
    core::params::PumpFunSellParams, factory::Protocol, SellParams, TradeFactory,
};
use crate::{
    common::{PriorityFee, SolanaRpcClient},
    swqos::FeeClient,
};
use anyhow::anyhow;
use solana_hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;

pub async fn sell(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_token: u64,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    let executor = TradeFactory::create_executor(Protocol::PumpFun);
    // 创建PumpFun协议参数
    let protocol_params = Box::new(PumpFunSellParams {});
    // 创建卖出参数
    let sell_params = SellParams {
        rpc: Some(rpc.clone()),
        payer: payer.clone(),
        mint,
        creator,
        amount_token: Some(amount_token),
        slippage_basis_points: None,
        priority_fee: priority_fee.clone(),
        lookup_table_key,
        recent_blockhash,
        protocol_params,
    };
    // 执行卖出交易
    executor.sell(sell_params).await?;
    Ok(())
}

/// Sell tokens by percentage
pub async fn sell_by_percent(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    percent: u64,
    amount_token: u64,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    if percent == 0 || percent > 100 {
        return Err(anyhow!("Percentage must be between 1 and 100"));
    }
    let amount = amount_token * percent / 100;
    sell(
        rpc,
        payer,
        mint,
        creator,
        amount,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
    )
    .await
}

/// Sell tokens by amount
pub async fn sell_by_amount(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount: u64,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    if amount == 0 {
        return Err(anyhow!("Amount must be greater than 0"));
    }
    sell(
        rpc,
        payer,
        mint,
        creator,
        amount,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
    )
    .await
}

pub async fn sell_by_percent_with_tip(
    rpc: Arc<SolanaRpcClient>,
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    percent: u64,
    amount_token: u64,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    if percent == 0 || percent > 100 {
        return Err(anyhow!("Percentage must be between 1 and 100"));
    }
    let amount = amount_token * percent / 100;
    sell_with_tip(
        rpc,
        fee_clients,
        payer,
        mint,
        creator,
        amount,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
    )
    .await
}

pub async fn sell_by_amount_with_tip(
    rpc: Arc<SolanaRpcClient>,
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount: u64,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    if amount == 0 {
        return Err(anyhow!("Amount must be greater than 0"));
    }
    sell_with_tip(
        rpc,
        fee_clients,
        payer,
        mint,
        creator,
        amount,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
    )
    .await
}

/// Sell tokens using Jito
pub async fn sell_with_tip(
    rpc: Arc<SolanaRpcClient>,
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_token: u64,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    let executor = TradeFactory::create_executor(Protocol::PumpFun);
    // 创建PumpFun协议参数
    let protocol_params = Box::new(PumpFunSellParams {});
    // 创建卖出参数
    let sell_params = SellParams {
        rpc: Some(rpc.clone()),
        payer: payer.clone(),
        mint,
        creator,
        amount_token: Some(amount_token),
        slippage_basis_points: None,
        priority_fee: priority_fee.clone(),
        lookup_table_key,
        recent_blockhash,
        protocol_params,
    };
    let sell_with_tip_params = sell_params.with_tip(fee_clients);
    // 执行卖出交易
    executor.sell_with_tip(sell_with_tip_params).await?;
    Ok(())
}
