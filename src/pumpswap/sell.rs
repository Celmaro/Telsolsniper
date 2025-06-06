use std::sync::Arc;
use std::time::Instant;
use std::str::FromStr;
use anyhow::anyhow;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,

    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::VersionedTransaction,
};
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;

use crate::common::{address_lookup_cache::get_address_lookup_table_account, PriorityFee, SolanaRpcClient};
use crate::pumpswap::common::{calculate_with_slippage_sell, find_pool, get_sell_sol_amount, get_token_balance};
use crate::constants::{accounts, trade::DEFAULT_SLIPPAGE, SELL_DISCRIMINATOR};
use crate::swqos::FeeClient;

// Constants for compute budget
// Increased from 64KB to 256KB to handle larger transactions
const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 256 * 1024;

// Sell tokens to a Pumpswap pool
pub async fn sell(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    amount_token: Option<u64>,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    let instructions = build_sell_instructions(rpc.clone(), payer.clone(), mint.clone(), amount_token, slippage_basis_points).await?;
    println!(" Sell transaction instructions: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    let recent_blockhash = rpc.get_latest_blockhash().await?;
    let transaction = build_sell_transaction(
        rpc.clone(),
        payer.clone(),
        priority_fee,
        instructions,
        lookup_table_key,
        recent_blockhash
    ).await?;
    println!(" Sell transaction signature: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    rpc.send_and_confirm_transaction(&transaction).await?;
    println!(" Sell transaction confirmation: {:?}", start_time.elapsed());
    Ok(())
}

// Sell tokens by percentage
pub async fn sell_by_percent(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    percent: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>
) -> Result<(), anyhow::Error> {
    if percent == 0 || percent > 100 {
        return Err(anyhow!("Percentage must be between 1 and 100"));
    }

    let (balance_u64, _) = get_token_balance(rpc.as_ref(), payer.as_ref(), &mint).await?;
    let amount = balance_u64 * percent / 100;
    sell(rpc, payer, mint, Some(amount), slippage_basis_points, priority_fee, lookup_table_key).await
}

// Sell tokens using a MEV service
pub async fn sell_with_tip(
    rpc: Arc<SolanaRpcClient>,
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    amount_token: Option<u64>,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>
) -> Result<(), anyhow::Error> {
    let mut transactions = vec![];
    let instructions = build_sell_instructions(rpc.clone(), payer.clone(), mint.clone(), amount_token, slippage_basis_points).await?;
    let recent_blockhash = rpc.get_latest_blockhash().await?;

    for fee_client in fee_clients.clone() {
        let tip_account = fee_client.get_tip_account()?;
        let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);

        let transaction = build_sell_transaction_with_tip(
            rpc.clone(),
            tip_account,
            payer.clone(),
            priority_fee.clone(),
            instructions.clone(),
            lookup_table_key,
            recent_blockhash,
        ).await?;

        transactions.push(transaction);
    }

    let mut handles = vec![];
    for (i, fee_client) in fee_clients.iter().enumerate() {
        let transaction = transactions[i].clone();
        let fee_client = fee_client.clone();

        let handle = tokio::spawn(async move {
            fee_client.send_transaction(crate::swqos::TradeType::Sell, &transaction).await
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await?;
    }

    Ok(())
}

// Sell tokens by percentage using a MEV service
pub async fn sell_by_percent_with_tip(
    rpc: Arc<SolanaRpcClient>,
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    percent: u64,
    slippage_basis_points: Option<u64>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>
) -> Result<(), anyhow::Error> {
    if percent == 0 || percent > 100 {
        return Err(anyhow!("Percentage must be between 1 and 100"));
    }

    let (balance_u64, _) = get_token_balance(rpc.as_ref(), payer.as_ref(), &mint).await?;
    let amount = balance_u64 * percent / 100;
    sell_with_tip(rpc, fee_clients, payer, mint, Some(amount), slippage_basis_points, priority_fee, lookup_table_key).await
}

// Build a transaction for selling tokens
pub async fn build_sell_transaction(
    _rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,
    build_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: solana_sdk::hash::Hash,
) -> Result<VersionedTransaction, anyhow::Error> {
    let mut instructions = vec![
        ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT),
        ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price),
        ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit),
    ];

    instructions.extend(build_instructions);

    // 确保所有需要签名的账户都被正确标记
    for instruction in &instructions {
        for account_meta in &instruction.accounts {
            if account_meta.is_signer && account_meta.pubkey != payer.pubkey() {
                return Err(anyhow!("Transaction requires a signature from an account other than the payer: {}", account_meta.pubkey));
            }
        }
    }

    let mut address_lookup_table_accounts = vec![];
    if let Some(lookup_table_key) = lookup_table_key {
        let account = get_address_lookup_table_account(&lookup_table_key).await;
        address_lookup_table_accounts.push(account);
    }

    let v0_message = solana_sdk::message::v0::Message::try_compile(
        &payer.pubkey(),
        &instructions,
        &address_lookup_table_accounts,
        recent_blockhash,
    ).map_err(|e| anyhow!(e))?;

    let versioned_message = solana_sdk::message::VersionedMessage::V0(v0_message);
    let transaction = VersionedTransaction::try_new(versioned_message, &[&payer])?;

    Ok(transaction)
}

// Build a transaction with tip for selling tokens
pub async fn build_sell_transaction_with_tip(
    _rpc: Arc<SolanaRpcClient>,
    tip_account: Arc<Pubkey>,
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,
    build_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: solana_sdk::hash::Hash,
) -> Result<VersionedTransaction, anyhow::Error> {
    let mut instructions = vec![
        ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT),
        ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price),
        ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit),
        system_instruction::transfer(
            &payer.pubkey(),
            &tip_account,
            priority_fee.sell_tip_fee,
        ),
    ];

    instructions.extend(build_instructions);

    // 确保所有需要签名的账户都被正确标记
    for instruction in &instructions {
        for account_meta in &instruction.accounts {
            if account_meta.is_signer && account_meta.pubkey != payer.pubkey() {
                return Err(anyhow!("Transaction requires a signature from an account other than the payer: {}", account_meta.pubkey));
            }
        }
    }

    let mut address_lookup_table_accounts = vec![];
    if let Some(lookup_table_key) = lookup_table_key {
        let account = get_address_lookup_table_account(&lookup_table_key).await;
        address_lookup_table_accounts.push(account);
    }

    let v0_message = solana_sdk::message::v0::Message::try_compile(
        &payer.pubkey(),
        &instructions,
        &address_lookup_table_accounts,
        recent_blockhash,
    ).map_err(|e| anyhow!(e))?;

    let versioned_message = solana_sdk::message::VersionedMessage::V0(v0_message);
    let transaction = VersionedTransaction::try_new(versioned_message, &[&payer])?;

    Ok(transaction)
}

// Build instructions for selling tokens
pub async fn build_sell_instructions(
    rpc: Arc<SolanaRpcClient>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    amount_token: Option<u64>,
    slippage_basis_points: Option<u64>,
) -> Result<Vec<Instruction>, anyhow::Error> {
    let (balance_u64, _) = get_token_balance(rpc.as_ref(), payer.as_ref(), &mint).await?;
    let amount = amount_token.unwrap_or(balance_u64);

    if amount == 0 {
        return Err(anyhow!("Amount cannot be zero"));
    }

    // Find the pool for this mint
    let pool = find_pool(rpc.as_ref(), &mint).await?;

    // Calculate the expected SOL amount
    let sol_amount = get_sell_sol_amount(rpc.as_ref(), &pool, amount).await?;

    // Calculate the minimum SOL amount with slippage
    let min_sol_amount = calculate_with_slippage_sell(sol_amount, slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE));

    // Get token accounts
    let user_base_token_account = spl_associated_token_account::get_associated_token_address(&payer.pubkey(), &mint);
    let user_quote_token_account = spl_associated_token_account::get_associated_token_address(&payer.pubkey(), &accounts::WSOL_TOKEN_ACCOUNT);

    // Get pool token accounts
    let pool_base_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
        &pool,
        &mint,
        &accounts::TOKEN_PROGRAM,
    );

    let pool_quote_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
        &pool,
        &accounts::WSOL_TOKEN_ACCOUNT,
        &accounts::TOKEN_PROGRAM,
    );

    let mut instructions = vec![];

    // Create the user's token account if it doesn't exist
    instructions.push(
        create_associated_token_account_idempotent(
            &payer.pubkey(),
            &payer.pubkey(),
            &mint,
            &accounts::TOKEN_PROGRAM,
        )
    );

    // Create the sell instruction
    // 注意：账户顺序必须与JavaScript SDK匹配
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new_readonly(pool, false), // pool_id (readonly)
        solana_sdk::instruction::AccountMeta::new(payer.pubkey(), true), // user (signer)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_ACCOUNT, false), // global (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(mint, false), // mint (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // WSOL_TOKEN_ACCOUNT (readonly)
        solana_sdk::instruction::AccountMeta::new(user_base_token_account, false), // user_base_token_account
        solana_sdk::instruction::AccountMeta::new(user_quote_token_account, false), // user_quote_token_account
        solana_sdk::instruction::AccountMeta::new(pool_base_token_account, false), // pool_base_token_account
        solana_sdk::instruction::AccountMeta::new(pool_quote_token_account, false), // pool_quote_token_account
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::FEE_RECIPIENT, false), // fee_recipient (readonly)
        solana_sdk::instruction::AccountMeta::new(accounts::FEE_RECIPIENT_ATA, false), // fee_recipient_ata
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::ASSOCIATED_TOKEN_PROGRAM, false), // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // event_authority (readonly)
        solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_PROGRAM, false), // PUMP_AMM_PROGRAM_ID (readonly)
    ];

    // Create the instruction data
    let mut data = vec![];
    data.extend_from_slice(&SELL_DISCRIMINATOR);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&min_sol_amount.to_le_bytes());

    instructions.push(
        Instruction {
            program_id: accounts::AMM_PROGRAM,
            accounts,
            data,
        }
    );

    Ok(instructions)
}
