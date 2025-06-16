use anyhow::anyhow;
use chrono;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0, AddressLookupTableAccount, VersionedMessage},
    native_token::sol_to_lamports,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::VersionedTransaction,
};
use solana_hash::Hash;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use crate::constants::pumpswap::{accounts, trade::DEFAULT_SLIPPAGE, BUY_DISCRIMINATOR};
use crate::pumpswap::common::{calculate_with_slippage_buy, find_pool, get_buy_token_amount};
use crate::swqos::FeeClient;
use crate::{
    common::{
        address_lookup_cache::get_address_lookup_table_account,
        nonce_cache::{self, NonceCache},
        PriorityFee, SolanaRpcClient,
    },
    pumpswap::common::{coin_creator_vault_ata, coin_creator_vault_authority},
};

// Constants for compute budget
// Increased from 64KB to 256KB to handle larger transactions
const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 256 * 1024;

/// 添加nonce消费指令到指令集合中
///
/// 只有提供了nonce_pubkey时才使用nonce功能
/// 如果nonce被锁定、已使用或未准备好，将返回错误
/// 成功时会锁定并标记nonce为已使用
fn add_nonce_instruction(instructions: &mut Vec<Instruction>, payer: &Keypair) -> Result<(), anyhow::Error> {
    let nonce_cache = NonceCache::get_instance();
    let nonce_info = nonce_cache.get_nonce_info();

    // 只检查nonce_account是否存在
    if let Some(nonce_pubkey) = nonce_info.nonce_account {
        // 暂不加锁
        // if nonce_info.lock {
        //     return Err(anyhow!("Nonce is locked"));
        // }
        if nonce_info.used {
            return Err(anyhow!("Nonce is used"));
        }
        if nonce_info.current_nonce == Hash::default() {
            return Err(anyhow!("Nonce is not ready"));
        }
        // if nonce_info.next_buy_time == 0 || chrono::Utc::now().timestamp() < nonce_info.next_buy_time {
        //     return Err(anyhow!("Nonce is not ready"));
        // }
        // 加锁 - 暂不加锁
        // nonce_cache.lock();

        // 创建Solana系统nonce推进指令 - 使用系统程序ID
        let nonce_advance_ix = system_instruction::advance_nonce_account(
            &nonce_pubkey,
            &payer.pubkey(),
        );


        instructions.push(nonce_advance_ix);
    }

    Ok(())
}


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
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    let mint = Arc::new(mint.clone());
    let creator = Arc::new(creator.clone());
    let instructions = match (
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
    ) {
        (
            Some(pool),
            Some(pool_base_token_account),
            Some(pool_quote_token_account),
            Some(user_base_token_account),
            Some(user_quote_token_account),
        ) => {
            build_buy_instructions_with_accounts(
                rpc.clone(),
                payer.clone(),
                Arc::new(pool),
                Arc::new(pool_base_token_account),
                Arc::new(pool_quote_token_account),
                Arc::new(user_base_token_account),
                Arc::new(user_quote_token_account),
                mint.clone(),
                creator.clone(),
                amount_sol,
                slippage_basis_points,
            )
            .await?
        }
        _ => {
            build_buy_instructions(
                rpc.clone(),
                payer.clone(),
                mint.clone(),
                creator.clone(),
                amount_sol,
                slippage_basis_points,
            )
            .await?
        }
    };
    println!(" Buy transaction instructions: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    let transaction = build_buy_transaction(
        rpc.clone(),
        payer.clone(),
        priority_fee.clone(),
        instructions,
        lookup_table_key,
        recent_blockhash,
    )
    .await?;
    println!(" Buy transaction signature: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    rpc.send_and_confirm_transaction(&transaction).await?;
    println!(" Buy transaction confirmation: {:?}", start_time.elapsed());

    Ok(())
}

// Buy tokens using a MEV service
pub async fn buy_with_tip(
    rpc: Arc<SolanaRpcClient>,
    fee_clients: Vec<Arc<FeeClient>>,
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
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    let mint = Arc::new(mint.clone());
    let creator = Arc::new(creator.clone());
    let instructions = match (
        pool,
        pool_base_token_account,
        pool_quote_token_account,
        user_base_token_account,
        user_quote_token_account,
    ) {
        (
            Some(pool),
            Some(pool_base_token_account),
            Some(pool_quote_token_account),
            Some(user_base_token_account),
            Some(user_quote_token_account),
        ) => {
            build_buy_instructions_with_accounts(
                rpc.clone(),
                payer.clone(),
                Arc::new(pool),
                Arc::new(pool_base_token_account),
                Arc::new(pool_quote_token_account),
                Arc::new(user_base_token_account),
                Arc::new(user_quote_token_account),
                mint.clone(),
                creator.clone(),
                amount_sol,
                slippage_basis_points,
            )
            .await?
        }
        _ => {
            build_buy_instructions(
                rpc.clone(),
                payer.clone(),
                mint.clone(),
                creator.clone(),
                amount_sol,
                slippage_basis_points,
            )
            .await?
        }
    };
    println!(" Buy transaction instructions: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    let mut transactions = vec![];

    for fee_client in fee_clients.clone() {
        let tip_account = fee_client.get_tip_account()?;
        let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);

        let transaction = build_buy_transaction_with_tip(
            rpc.clone(),
            tip_account,
            payer.clone(),
            priority_fee.clone(),
            instructions.clone(),
            lookup_table_key,
            recent_blockhash,
        )
        .await?;

        transactions.push(transaction);
    }

    println!(" Buy transaction signature: {:?}", start_time.elapsed());

    let mut handles = vec![];
    for (i, fee_client) in fee_clients.iter().enumerate() {
        let transaction = transactions[i].clone();
        let fee_client = fee_client.clone();

        let handle = tokio::spawn(async move {
            fee_client
                .send_transaction(crate::swqos::TradeType::Buy, &transaction)
                .await
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await?;
    }

    println!(" Buy transaction confirmation: {:?}", start_time.elapsed());

    Ok(())
}

// Build a transaction for buying tokens
pub async fn build_buy_transaction(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,
    build_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<VersionedTransaction, anyhow::Error> {
    let mut instructions = vec![];
    if let Err(e) = add_nonce_instruction(&mut instructions, payer.as_ref()) {
        return Err(e);
    }

     // 添加计算预算指令
     instructions.push(ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT));
     instructions.push(ComputeBudgetInstruction::set_compute_unit_price( priority_fee.rpc_unit_price ));
     instructions.push(ComputeBudgetInstruction::set_compute_unit_limit( priority_fee.rpc_unit_limit ));
     instructions.extend(build_instructions);
 
     let nonce_cache = NonceCache::get_instance();
     let nonce_info = nonce_cache.get_nonce_info();
 
     let blockhash = if nonce_info.nonce_account.is_some() && instructions.len() > 0 {
         nonce_info.current_nonce
     } else {
         recent_blockhash
     };

     let mut address_lookup_table_accounts = vec![];
     if let Some(lookup_table_key) = lookup_table_key {
         let account = get_address_lookup_table_account(&lookup_table_key).await;
         address_lookup_table_accounts.push(account);
     }
 
     let v0_message: v0::Message =
         v0::Message::try_compile(&payer.pubkey(), &instructions, &address_lookup_table_accounts, blockhash)?;
     let versioned_message: VersionedMessage = VersionedMessage::V0(v0_message.clone());
     let transaction = VersionedTransaction::try_new(versioned_message, &[payer.as_ref()])?;
 
     // verify_lookup_table_usage(&v0_message, &address_lookup_table_accounts);
 
     Ok(transaction)
}

// Build a transaction with tip for buying tokens
pub async fn build_buy_transaction_with_tip(
    rpc: Arc<SolanaRpcClient>,
    tip_account: Arc<Pubkey>,
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,
    build_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<VersionedTransaction, anyhow::Error> {
    let mut instructions = vec![];

    // 添加nonce消费指令
    if let Err(e) = add_nonce_instruction(&mut instructions, payer.as_ref()) {
        return Err(e);
    }

    // 添加计算预算指令和小费转账指令
    instructions.push(ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT));
    instructions.push(ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price));
    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit));
    instructions.extend(build_instructions);
    instructions.push(system_instruction::transfer(
        &payer.pubkey(),
        &tip_account,
        sol_to_lamports(priority_fee.buy_tip_fee),
    ));

    let nonce_cache = NonceCache::get_instance();
    let nonce_info = nonce_cache.get_nonce_info();

    // 如果使用了nonce账户，则使用nonce账户中的blockhash
    let blockhash_to_use = if nonce_info.nonce_account.is_some() && instructions.len() > 0 {
        nonce_info.current_nonce
    } else {
        recent_blockhash
    };

    let mut address_lookup_table_accounts = vec![];
    if let Some(lookup_table_key) = lookup_table_key {
        let account = get_address_lookup_table_account(&lookup_table_key).await;
        address_lookup_table_accounts.push(account);
    }

    let v0_message: v0::Message =
        v0::Message::try_compile(&payer.pubkey(), &instructions,  &address_lookup_table_accounts, blockhash_to_use)?;
    let versioned_message: VersionedMessage = VersionedMessage::V0(v0_message.clone());
    let transaction = VersionedTransaction::try_new(versioned_message, &[payer.as_ref()])?;

    // nonce_cache.mark_used();
    // verify_lookup_table_usage(&v0_message, &address_lookup_table_accounts);

    Ok(transaction)
}

// Build instructions for buying tokens
pub async fn build_buy_instructions(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Arc<Pubkey>,
    creator: Arc<Pubkey>,
    amount_sol: u64,
    slippage_basis_points: Option<u64>,
) -> Result<Vec<Instruction>, anyhow::Error> {
    if amount_sol == 0 {
        return Err(anyhow!("Amount cannot be zero"));
    }

    // Find the pool for this mint
    let pool = find_pool(rpc.as_ref(), mint.as_ref()).await?;

    // Create the user's token account if it doesn't exist
    let user_base_token_account =
        spl_associated_token_account::get_associated_token_address(&payer.pubkey(), mint.as_ref());
    let user_quote_token_account = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &accounts::WSOL_TOKEN_ACCOUNT,
    );

    // Get pool token accounts
    let pool_base_token_account =
        spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool,
            mint.as_ref(),
            &accounts::TOKEN_PROGRAM,
        );

    let pool_quote_token_account =
        spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool,
            &accounts::WSOL_TOKEN_ACCOUNT,
            &accounts::TOKEN_PROGRAM,
        );

    let instructions = build_buy_instructions_with_accounts(
        rpc,
        payer,
        Arc::new(pool),
        Arc::new(pool_base_token_account),
        Arc::new(pool_quote_token_account),
        Arc::new(user_base_token_account),
        Arc::new(user_quote_token_account),
        mint,
        creator,
        amount_sol,
        slippage_basis_points,
    )
    .await?;

    Ok(instructions)
}

pub async fn build_buy_instructions_with_accounts(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    pool: Arc<Pubkey>,
    pool_base_token_account: Arc<Pubkey>,
    pool_quote_token_account: Arc<Pubkey>,
    user_base_token_account: Arc<Pubkey>,
    user_quote_token_account: Arc<Pubkey>,
    mint: Arc<Pubkey>,
    creator: Arc<Pubkey>,
    amount_sol: u64,
    slippage_basis_points: Option<u64>,
) -> Result<Vec<Instruction>, anyhow::Error> {
    if amount_sol == 0 {
        return Err(anyhow!("Amount cannot be zero"));
    }

    // Calculate the expected token amount
    let token_amount = get_buy_token_amount(rpc.as_ref(), &pool, amount_sol).await?;

    // Calculate the maximum SOL amount with slippage
    let max_sol_amount = calculate_with_slippage_buy(
        amount_sol,
        slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
    );

    let mut instructions = vec![];

    // Create the user's base token account if it doesn't exist
    instructions.push(create_associated_token_account_idempotent(
        &payer.pubkey(),
        &payer.pubkey(),
        mint.as_ref(),
        &accounts::TOKEN_PROGRAM,
    ));

    let coin_creator_vault_ata = coin_creator_vault_ata(*creator.as_ref());
    let coin_creator_vault_authority = coin_creator_vault_authority(*creator.as_ref());

    // Create the buy instruction
    // 注意：账户顺序必须与JavaScript SDK匹配
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new_readonly(*pool, false), // pool_id (readonly)
        solana_sdk::instruction::AccountMeta::new(payer.pubkey(), true),  // user (signer)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_ACCOUNT, false), // global (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(*mint, false), // mint (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // WSOL_TOKEN_ACCOUNT (readonly)
        solana_sdk::instruction::AccountMeta::new(*user_base_token_account, false), // user_base_token_account
        solana_sdk::instruction::AccountMeta::new(*user_quote_token_account, false), // user_quote_token_account
        solana_sdk::instruction::AccountMeta::new(*pool_base_token_account, false), // pool_base_token_account
        solana_sdk::instruction::AccountMeta::new(*pool_quote_token_account, false), // pool_quote_token_account
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::FEE_RECIPIENT, false), // fee_recipient (readonly)
        solana_sdk::instruction::AccountMeta::new(accounts::FEE_RECIPIENT_ATA, false), // fee_recipient_ata
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(
            accounts::ASSOCIATED_TOKEN_PROGRAM,
            false,
        ), // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // event_authority (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_PROGRAM, false), // PUMP_AMM_PROGRAM_ID (readonly)
        solana_sdk::instruction::AccountMeta::new(coin_creator_vault_ata, false), // coin_creator_vault_ata
        solana_sdk::instruction::AccountMeta::new_readonly(coin_creator_vault_authority, false), // coin_creator_vault_authority (readonly)
    ];

    // Create the instruction data
    let mut data = vec![];
    data.extend_from_slice(&BUY_DISCRIMINATOR);
    data.extend_from_slice(&token_amount.to_le_bytes());
    data.extend_from_slice(&max_sol_amount.to_le_bytes());

    instructions.push(Instruction {
        program_id: accounts::AMM_PROGRAM,
        accounts,
        data,
    });

    Ok(instructions)
}
