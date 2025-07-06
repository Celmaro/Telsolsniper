use anyhow::anyhow;
use solana_hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;

use crate::common::{PriorityFee, SolanaRpcClient};
use crate::pumpswap::common::get_token_balance;
use crate::swqos::SwqosClient;
use crate::trading::{core::params::PumpSwapParams, factory::Protocol, SellParams, TradeFactory};

// Sell tokens to a Pumpswap pool
pub async fn sell(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_token: Option<u64>,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    // 可选（必须全部传）
    pool: Option<Pubkey>,
    pool_base_token_account: Option<Pubkey>,
    pool_quote_token_account: Option<Pubkey>,
    user_base_token_account: Option<Pubkey>,
    user_quote_token_account: Option<Pubkey>,
) -> Result<(), anyhow::Error> {
    let executor = TradeFactory::create_executor(Protocol::PumpSwap);
    // 创建PumpFun协议参数
    let protocol_params = Box::new(PumpSwapParams {
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
        auto_handle_wsol: true,
    });
    // 创建卖出参数
    let sell_params = SellParams {
        rpc: Some(rpc.clone()),
        payer: payer.clone(),
        mint,
        creator,
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
    creator: Pubkey,
    percent: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    // 可选（必须全部传）
    pool: Option<Pubkey>,
    pool_base_token_account: Option<Pubkey>,
    pool_quote_token_account: Option<Pubkey>,
    user_base_token_account: Option<Pubkey>,
    user_quote_token_account: Option<Pubkey>,
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
        creator,
        Some(amount),
        slippage_basis_points,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
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
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    // 可选（必须全部传）
    pool: Option<Pubkey>,
    pool_base_token_account: Option<Pubkey>,
    pool_quote_token_account: Option<Pubkey>,
    user_base_token_account: Option<Pubkey>,
    user_quote_token_account: Option<Pubkey>,
) -> Result<(), anyhow::Error> {
    if amount == 0 {
        return Err(anyhow!("Amount must be greater than 0"));
    }

    sell(
        rpc,
        payer,
        mint,
        creator,
        Some(amount),
        slippage_basis_points,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
    )
    .await
}

// Sell tokens using a MEV service
pub async fn sell_with_tip(
    rpc: Arc<SolanaRpcClient>,
    swqos_clients: Vec<Arc<SwqosClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_token: Option<u64>,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    // 可选（必须全部传）
    pool: Option<Pubkey>,
    pool_base_token_account: Option<Pubkey>,
    pool_quote_token_account: Option<Pubkey>,
    user_base_token_account: Option<Pubkey>,
    user_quote_token_account: Option<Pubkey>,
) -> Result<(), anyhow::Error> {
    let executor = TradeFactory::create_executor(Protocol::PumpSwap);
    // 创建PumpFun协议参数
    let protocol_params = Box::new(PumpSwapParams {
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
        auto_handle_wsol: true,
    });
    // 创建卖出参数
    let sell_params = SellParams {
        rpc: Some(rpc.clone()),
        payer: payer.clone(),
        mint,
        creator,
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
    creator: Pubkey,
    percent: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    // 可选（必须全部传）
    pool: Option<Pubkey>,
    pool_base_token_account: Option<Pubkey>,
    pool_quote_token_account: Option<Pubkey>,
    user_base_token_account: Option<Pubkey>,
    user_quote_token_account: Option<Pubkey>,
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
        creator,
        Some(amount),
        slippage_basis_points,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
    )
    .await
}

// Sell tokens by amount using a MEV service
pub async fn sell_by_amount_with_tip(
    rpc: Arc<SolanaRpcClient>,
    swqos_clients: Vec<Arc<SwqosClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    // 可选（必须全部传）
    pool: Option<Pubkey>,
    pool_base_token_account: Option<Pubkey>,
    pool_quote_token_account: Option<Pubkey>,
    user_base_token_account: Option<Pubkey>,
    user_quote_token_account: Option<Pubkey>,
) -> Result<(), anyhow::Error> {
    if amount == 0 {
        return Err(anyhow!("Amount must be greater than 0"));
    }

    sell_with_tip(
        rpc,
        swqos_clients,
        payer,
        mint,
        creator,
        Some(amount),
        slippage_basis_points,
        priority_fee,
        lookup_table_key,
        recent_blockhash,
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
    )
    .await
}
