use anyhow::anyhow;
use solana_hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;

use crate::common::{PriorityFee, SolanaRpcClient};
use crate::pumpswap::common::get_token_balance;
use crate::swqos::SwqosClient;
use crate::trading::{
    core::params::BonkParams, factory::Protocol, SellParams, TradeFactory,
};

// Sell tokens to a Pumpswap pool
pub async fn sell(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    virtual_base: u128,
    virtual_quote: u128,
    real_base_before: u128,
    real_quote_before: u128,
    amount_token: Option<u64>,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    let executor = TradeFactory::create_executor(Protocol::Bonk);
    // 创建PumpFun协议参数
    let protocol_params = Box::new(BonkParams {
        virtual_base: Some(virtual_base),
        virtual_quote: Some(virtual_quote),
        real_base_before: Some(real_base_before),
        real_quote_before: Some(real_quote_before),
        auto_handle_wsol: true,
    });
    // 创建卖出参数
    let sell_params = SellParams {
        rpc: Some(rpc.clone()),
        payer: payer.clone(),
        mint,
        creator: Pubkey::default(),
        amount_token: amount_token,
        slippage_basis_points: slippage_basis_points,
        priority_fee: priority_fee.clone(),
        lookup_table_key,
        recent_blockhash,
        protocol_params,
    };
    // 执行卖出交易
    executor.sell(sell_params).await?;
    Ok(())
}

// Sell tokens by percentage
pub async fn sell_by_percent(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    virtual_base: u128,
    virtual_quote: u128,
    real_base_before: u128,
    real_quote_before: u128,
    percent: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    if percent == 0 || percent > 100 {
        return Err(anyhow!("Percentage must be between 1 and 100"));
    }
    let (balance_u64, _) = get_token_balance(rpc.as_ref(), payer.as_ref(), &mint).await?;
    let amount = balance_u64 * percent / 100;
    sell(
        rpc,
        payer,
        mint,
        virtual_base,
        virtual_quote,
        real_base_before,
        real_quote_before,
        Some(amount),
        slippage_basis_points,
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
    virtual_base: u128,
    virtual_quote: u128,
    real_base_before: u128,
    real_quote_before: u128,
    amount: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    sell(
        rpc,
        payer,
        mint,
        virtual_base,
        virtual_quote,
        real_base_before,
        real_quote_before,
        Some(amount),
        slippage_basis_points,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
    )
    .await
}

// Sell tokens using a MEV service
pub async fn sell_with_tip(
    rpc: Arc<SolanaRpcClient>,
    swqos_clients: Vec<Arc<SwqosClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    virtual_base: u128,
    virtual_quote: u128,
    real_base_before: u128,
    real_quote_before: u128,
    amount_token: Option<u64>,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    let executor = TradeFactory::create_executor(Protocol::Bonk);
    // 创建PumpFun协议参数
    let protocol_params = Box::new(BonkParams {
        virtual_base: Some(virtual_base),
        virtual_quote: Some(virtual_quote),
        real_base_before: Some(real_base_before),
        real_quote_before: Some(real_quote_before),
        auto_handle_wsol: true,
    });
    // 创建卖出参数
    let sell_params = SellParams {
        rpc: Some(rpc.clone()),
        payer: payer.clone(),
        mint,
        creator: Pubkey::default(),
        amount_token: amount_token,
        slippage_basis_points: slippage_basis_points,
        priority_fee: priority_fee.clone(),
        lookup_table_key,
        recent_blockhash,
        protocol_params,
    };
    let sell_with_tip_params = sell_params.with_tip(swqos_clients);
    // 执行卖出交易
    executor.sell_with_tip(sell_with_tip_params).await?;
    Ok(())
}

// Sell tokens by percentage using a MEV service
pub async fn sell_by_percent_with_tip(
    rpc: Arc<SolanaRpcClient>,
    swqos_clients: Vec<Arc<SwqosClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    virtual_base: u128,
    virtual_quote: u128,
    real_base_before: u128,
    real_quote_before: u128,
    percent: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    if percent == 0 || percent > 100 {
        return Err(anyhow!("Percentage must be between 1 and 100"));
    }

    let (balance_u64, _) = get_token_balance(rpc.as_ref(), payer.as_ref(), &mint).await?;
    let amount = balance_u64 * percent / 100;
    sell_with_tip(
        rpc,
        swqos_clients,
        payer,
        mint,
        virtual_base,
        virtual_quote,
        real_base_before,
        real_quote_before,
        Some(amount),
        slippage_basis_points,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
    )
    .await
}

// Sell tokens by amount using a MEV service
pub async fn sell_by_amount_with_tip(
    rpc: Arc<SolanaRpcClient>,
    swqos_clients: Vec<Arc<SwqosClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    virtual_base: u128,
    virtual_quote: u128,
    real_base_before: u128,
    real_quote_before: u128,
    amount: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    sell_with_tip(
        rpc,
        swqos_clients,
        payer,
        mint,
        virtual_base,
        virtual_quote,
        real_base_before,
        real_quote_before,
        Some(amount),
        slippage_basis_points,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
    )
    .await
}
