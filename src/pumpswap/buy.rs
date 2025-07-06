use solana_hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;

use crate::swqos::SwqosClient;
use crate::trading::{core::params::PumpSwapParams, factory::Protocol, BuyParams, TradeFactory};
use crate::{common::PriorityFee, SolanaRpcClient};

// Constants for compute budget
// Increased from 64KB to 256KB to handle larger transactions
const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 256 * 1024;

// Buy tokens from a Pumpswap pool
pub async fn buy(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_sol: u64,
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
    auto_handle_wsol: bool,
) -> Result<(), anyhow::Error> {
    // 创建执行器
    let executor = TradeFactory::create_executor(Protocol::PumpSwap);
    // 创建协议特定参数
    let protocol_params = Box::new(PumpSwapParams {
        pool: pool,
        pool_base_token_account: pool_base_token_account,
        pool_quote_token_account: pool_quote_token_account,
        user_base_token_account: user_base_token_account,
        user_quote_token_account: user_quote_token_account,
        auto_handle_wsol: auto_handle_wsol,
    });
    // 创建买入参数
    let buy_params = BuyParams {
        rpc: Some(rpc.clone()),
        payer: payer,
        mint: mint,
        creator: creator,
        amount_sol: amount_sol,
        slippage_basis_points: slippage_basis_points,
        priority_fee: priority_fee,
        lookup_table_key: lookup_table_key,
        recent_blockhash: recent_blockhash,
        data_size_limit: MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT,
        protocol_params,
    };
    // 执行买入
    executor.buy(buy_params).await?;
    Ok(())
}

// Buy tokens using a MEV service
pub async fn buy_with_tip(
    rpc: Arc<SolanaRpcClient>,
    swqos_clients: Vec<Arc<SwqosClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_sol: u64,
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
    auto_handle_wsol: bool,
) -> Result<(), anyhow::Error> {
    // 创建执行器
    let executor = TradeFactory::create_executor(Protocol::PumpSwap);
    // 创建协议特定参数
    let protocol_params = Box::new(PumpSwapParams {
        pool: pool,
        pool_base_token_account: pool_base_token_account,
        pool_quote_token_account: pool_quote_token_account,
        user_base_token_account: user_base_token_account,
        user_quote_token_account: user_quote_token_account,
        auto_handle_wsol: auto_handle_wsol,
    });
    // 创建买入参数
    let buy_params = BuyParams {
        rpc: Some(rpc.clone()),
        payer: payer,
        mint: mint,
        creator: creator,
        amount_sol: amount_sol,
        slippage_basis_points: slippage_basis_points,
        priority_fee: priority_fee,
        lookup_table_key: lookup_table_key,
        recent_blockhash: recent_blockhash,
        data_size_limit: MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT,
        protocol_params,
    };
    let buy_with_tip_params = buy_params.with_tip(swqos_clients);
    // 执行买入
    executor.buy_with_tip(buy_with_tip_params).await?;
    Ok(())
}
