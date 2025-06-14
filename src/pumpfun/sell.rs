use anyhow::anyhow;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, message::{v0, VersionedMessage}, native_token::sol_to_lamports, pubkey::Pubkey, signature::{Keypair}, signer::Signer, system_instruction, transaction::{VersionedTransaction}
};
use solana_hash::Hash;
use spl_associated_token_account::get_associated_token_address;
use spl_token::instruction::close_account;
use tokio::task::JoinHandle;
use std::{str::FromStr, sync::Arc, time::Instant};
use crate::PumpFun;
use crate::{common::{address_lookup_cache::get_address_lookup_table_account, PriorityFee, SolanaRpcClient}, constants::pumpfun::{global_constants::FEE_RECIPIENT}, instruction, swqos::{FeeClient, TradeType, ClientType}};

use super::common::get_creator_vault_pda;

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
    let start_time = Instant::now();
    let instructions = build_sell_instructions(payer.clone(), mint.clone(), creator, amount_token).await?;
    println!(" 卖出交易指令: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    let transaction = build_sell_transaction(payer.clone(), priority_fee, instructions, lookup_table_key, recent_blockhash).await?;
    println!(" 卖出交易签名: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    rpc.send_and_confirm_transaction(&transaction).await?;
    println!(" 卖出交易确认: {:?}", start_time.elapsed());
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
    sell(rpc, payer, mint, creator, amount, priority_fee, lookup_table_key, recent_blockhash).await
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

    sell(rpc, payer, mint, creator, amount, priority_fee, lookup_table_key, recent_blockhash).await
}

pub async fn sell_by_percent_with_tip(
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
    sell_with_tip(fee_clients, payer, mint, creator, amount, priority_fee, lookup_table_key, recent_blockhash).await
}

pub async fn sell_by_amount_with_tip(
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

    sell_with_tip(fee_clients, payer, mint, creator, amount, priority_fee, lookup_table_key, recent_blockhash).await
}

/// Sell tokens using Jito
pub async fn sell_with_tip(
    fee_clients: Vec<Arc<FeeClient>>,
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_token: u64,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    let mint = Arc::new(mint.clone());
    let instructions = build_sell_instructions(payer.clone(), *mint, creator, amount_token).await?;
    println!(" 卖出交易指令: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    let cores = core_affinity::get_core_ids().unwrap();
    let mut handles: Vec<JoinHandle<Result<(), anyhow::Error>>> = vec![];

    for i in 0..fee_clients.len() {
        let fee_client = fee_clients[i].clone();
        let payer = payer.clone();
        let instructions = instructions.clone();
        let priority_fee = priority_fee.clone();
        let core_id = cores[i % cores.len()];

        let handle = tokio::spawn(async move {
            core_affinity::set_for_current(core_id);

            let transaction = if fee_client.get_client_type() == ClientType::Rpc {
                build_sell_transaction(
                    payer.clone(),
                    priority_fee.clone(),
                    instructions.clone(),
                    lookup_table_key,
                    recent_blockhash
                ).await?
            } else {
                let tip_account = fee_client.get_tip_account()?;
                let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);
                
                build_sell_transaction_with_tip(
                    tip_account,
                    payer.clone(),
                    priority_fee.clone(),
                    instructions.clone(),
                    lookup_table_key,
                    recent_blockhash
                ).await?
            };

            fee_client.send_transaction(TradeType::Sell, &transaction).await?;
            Ok::<(), anyhow::Error>(())
        });

        handles.push(handle);
    }

    println!(" 卖出交易签名: {:?}", start_time.elapsed());

    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => (),
            Ok(Err(e)) => println!("Error in task: {}", e),
            Err(e) => println!("Task join error: {}", e),
        }
    }

    Ok(())
}

pub async fn build_sell_transaction(
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,
    build_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    blockhash: Hash
) -> Result<VersionedTransaction, anyhow::Error> {
    let mut instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price),
        ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit),
    ];

    instructions.extend(build_instructions);

    let mut address_lookup_table_accounts = vec![];
    if let Some(lookup_table_key) = lookup_table_key {
        let account = get_address_lookup_table_account(&lookup_table_key).await;
        address_lookup_table_accounts.push(account);
    }   

    let transaction = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &payer.pubkey(),
            &instructions,
            &address_lookup_table_accounts,
            blockhash,
        )?),
        &[payer],
    )?;

    Ok(transaction)
}

pub async fn build_sell_transaction_with_tip(
    tip_account: Arc<Pubkey>,
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,
    build_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    blockhash: Hash,
) -> Result<VersionedTransaction, anyhow::Error> {
    let mut instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_price(priority_fee.unit_price),
        ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.unit_limit),
    ];

    instructions.extend(build_instructions); 

    instructions.push(
        system_instruction::transfer(
            &payer.pubkey(),
            &tip_account,
            sol_to_lamports(priority_fee.sell_tip_fee),
        ),
    );

    let mut address_lookup_table_accounts = vec![];
    if let Some(lookup_table_key) = lookup_table_key {
        let account = get_address_lookup_table_account(&lookup_table_key).await;
        address_lookup_table_accounts.push(account);
    }   

    let transaction = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &payer.pubkey(),
            &instructions,
            &address_lookup_table_accounts,
            blockhash,
        )?),
        &[payer],
    )?;

    Ok(transaction)
}

pub async fn build_sell_instructions(
    payer: Arc<Keypair>,
    mint: Pubkey,
    creator: Pubkey,
    amount_token: u64,
) -> Result<Vec<Instruction>, anyhow::Error> {
    if amount_token == 0 {
        return Err(anyhow!("Amount cannot be zero"));
    }

    let creator_vault_pda = get_creator_vault_pda(&creator).unwrap();
    let ata = get_associated_token_address(&payer.pubkey(), &mint);

    // Get token balance using get_payer_token_balance
    let balance_u64 = PumpFun::get_instance().get_payer_token_balance(&mint).await?;

    let mut amount_token = amount_token;
    if amount_token > balance_u64 {
        amount_token = balance_u64;
    }

    let mut instructions = vec![
        instruction::sell(
            payer.as_ref(),
            &mint,
            &creator_vault_pda,
            &FEE_RECIPIENT,
            instruction::Sell {
                _amount: amount_token,
                _min_sol_output: 1,
            },
        ),
    ];

    // Only add close account instruction if amount is less than balance
    if amount_token >= balance_u64 {
        instructions.push(
            close_account(
                &spl_token::ID,
                &ata,
                &payer.pubkey(),
                &payer.pubkey(),
                &[&payer.pubkey()],
            )?
        );
    }

    Ok(instructions)
}
