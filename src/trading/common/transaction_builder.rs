use solana_hash::Hash;
use solana_sdk::{
    instruction::Instruction,
    message::{v0, VersionedMessage},
    native_token::sol_str_to_lamports,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};
use solana_system_interface::instruction::transfer;
use std::sync::Arc;

use super::{
    address_lookup_manager::get_address_lookup_table_accounts,
    compute_budget_manager::add_compute_budget_instructions,
    nonce_manager::{add_nonce_instruction, get_transaction_blockhash},
};
use crate::{common::PriorityFee, trading::MiddlewareManager};

/// 构建标准的RPC交易
pub async fn build_transaction(
    payer: Arc<Keypair>,
    priority_fee: &PriorityFee,
    business_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    data_size_limit: u32,
    middleware_manager: Option<Arc<MiddlewareManager>>,
    protocol_name: &str,
    is_buy: bool,
    with_tip: bool,
    tip_account: &Pubkey,
    tip_amount: f64,
) -> Result<VersionedTransaction, anyhow::Error> {
    let mut instructions = Vec::with_capacity(business_instructions.len() + 5);

    // 添加nonce指令
    if is_buy {
        if let Err(e) = add_nonce_instruction(&mut instructions, payer.as_ref()) {
            return Err(e);
        }
    }

    // 添加计算预算指令
    add_compute_budget_instructions(&mut instructions, priority_fee, data_size_limit, true, is_buy);

    // 添加业务指令
    instructions.extend(business_instructions);

    // 添加小费转账指令
    if with_tip {
        instructions.push(transfer(
            &payer.pubkey(),
            tip_account,
            sol_str_to_lamports(tip_amount.to_string().as_str()).unwrap_or(0),
        ));
    }

    // 获取交易使用的blockhash
    let blockhash =
        if is_buy { get_transaction_blockhash(recent_blockhash) } else { recent_blockhash };

    // 获取地址查找表账户
    let address_lookup_table_accounts = get_address_lookup_table_accounts(lookup_table_key).await;

    // 构建交易
    build_versioned_transaction(
        payer,
        instructions,
        address_lookup_table_accounts,
        blockhash,
        middleware_manager,
        protocol_name,
        is_buy,
    )
    .await
}

/// 构建版本化交易的底层函数
async fn build_versioned_transaction(
    payer: Arc<Keypair>,
    instructions: Vec<Instruction>,
    address_lookup_table_accounts: Vec<solana_sdk::message::AddressLookupTableAccount>,
    blockhash: Hash,
    middleware_manager: Option<Arc<MiddlewareManager>>,
    protocol_name: &str,
    is_buy: bool,
) -> Result<VersionedTransaction, anyhow::Error> {
    let full_instructions = match middleware_manager {
        Some(middleware_manager) => middleware_manager
            .apply_middlewares_process_full_instructions(
                instructions,
                protocol_name.to_string(),
                is_buy,
            )?,
        None => instructions,
    };
    let v0_message: v0::Message = v0::Message::try_compile(
        &payer.pubkey(),
        &full_instructions,
        &address_lookup_table_accounts,
        blockhash,
    )?;

    let versioned_message: VersionedMessage = VersionedMessage::V0(v0_message.clone());
    let transaction = VersionedTransaction::try_new(versioned_message, &[payer.as_ref()])?;

    Ok(transaction)
}
