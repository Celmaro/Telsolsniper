use anyhow::anyhow;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, message::{v0, AddressLookupTableAccount, VersionedMessage}, native_token::sol_to_lamports, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction, transaction::{Transaction, VersionedTransaction}
};
use solana_hash::Hash;
use spl_associated_token_account::instruction::create_associated_token_account;
use tokio::task::JoinHandle;
use std::{str::FromStr, time::Instant, sync::Arc};

use crate::{
    common::{
        address_lookup_cache::get_address_lookup_table_account, 
        nonce_cache:: NonceCache, 
        tip_cache::TipCache, 
        PriorityFee, 
        SolanaRpcClient
    }, 
    constants::{self, pumpfun::global_constants::FEE_RECIPIENT}, 
    instruction, 
    swqos::{ClientType, FeeClient, TradeType}
};

const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 250000;

use super::common::{calculate_with_slippage_buy, get_buy_token_amount_from_sol_amount, init_bonding_curve_account, get_bonding_curve_account_v2, get_bonding_curve_pda};
use crate::constants::trade_type::{SNIPER_BUY};
use crate::PumpFun;

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
    trade_type: String,
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    let mint = Arc::new(mint.clone());
    let instructions = build_buy_instructions(payer.clone(), mint.clone(), creator, dev_buy_token, dev_sol_cost, buy_sol_cost, slippage_basis_points, trade_type).await?;
    println!(" 买入交易指令: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    let transaction = build_buy_transaction(
        payer.clone(), 
        priority_fee.clone(),
        instructions,
        lookup_table_key,
        recent_blockhash,
    ).await?;
    println!(" 买入交易签名: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    rpc.send_and_confirm_transaction(&transaction).await?;
    println!(" 买入交易确认: {:?}", start_time.elapsed());

    Ok(())
}

/// Buy tokens using Jito
// pub async fn buy_with_tip(
//     fee_clients: Vec<Arc<FeeClient>>,
//     payer: Arc<Keypair>,
//     mint: Pubkey,
//     creator: Pubkey,
//     dev_buy_token: u64,
//     dev_sol_cost: u64,
//     buy_sol_cost: u64,
//     slippage_basis_points: Option<u64>,
//     priority_fee: PriorityFee,
//     lookup_table_key: Option<Pubkey>,
//     recent_blockhash: Hash,
// ) -> Result<(), anyhow::Error> {
//     let start_time = Instant::now();
//     let mint = Arc::new(mint.clone());
//     let instructions = build_buy_instructions(payer.clone(), mint.clone(), creator, dev_buy_token, dev_sol_cost, buy_sol_cost, slippage_basis_points).await?;
//     println!(" 买入交易指令: {:?}", start_time.elapsed());

//     let start_time = Instant::now();
//     let mut transactions = vec![];
    
//     for fee_client in fee_clients.clone() {
//         if fee_client.get_client_type() == ClientType::Rpc {
//             let transaction = build_buy_transaction(
//                 payer.clone(),
//                 priority_fee.clone(),
//                 instructions.clone(),
//                 lookup_table_key,
//                 recent_blockhash,
//             ).await?;

//             transactions.push(transaction);
//         } else {
//             let tip_account = fee_client.get_tip_account()?;
//             let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);
            
//             let transaction = build_buy_transaction_with_tip(
//                 tip_account, 
//                 payer.clone(),
//                 priority_fee.clone(), 
//                 instructions.clone(), 
//                 lookup_table_key,
//                 recent_blockhash,
//             ).await?;
            
//             transactions.push(transaction);
//         }
//     }

//     println!(" 买入交易签名: {:?}", start_time.elapsed());

//     let cores = core_affinity::get_core_ids().unwrap();
//     let mut handles: Vec<JoinHandle<Result<(), anyhow::Error>>> = vec![];
//     for i in 0..fee_clients.len() {
//         let fee_client = fee_clients[i].clone();
//         let transactions = transactions.clone();
//         let transaction = transactions[i].clone();
        
//         let core_id = cores[i % cores.len()];
//         let handle = tokio::spawn(async move {
//             core_affinity::set_for_current(core_id);
//            fee_client.send_transaction(TradeType::Buy, &transaction).await?;
//             Ok::<(), anyhow::Error>(())
//         });

//         handles.push(handle);        
//     }

//     for handle in handles {
//         match handle.await {
//             Ok(Ok(_)) => (),
//             Ok(Err(e)) => println!("Error in task: {}", e),
//             Err(e) => println!("Task join error: {}", e),
//         }
//     }

//     Ok(())
// }

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
    trade_type: String,
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    let mint = Arc::new(mint.clone());
    let instructions = build_buy_instructions(payer.clone(), mint.clone(), creator, dev_buy_token, dev_sol_cost, buy_sol_cost, slippage_basis_points, trade_type).await?;
    println!(" 买入交易指令: {:?}", start_time.elapsed());

    let start_time = Instant::now();
    let cores = core_affinity::get_core_ids().unwrap();
    let mut handles: Vec<JoinHandle<Result<(), anyhow::Error>>> = vec![];

    for i in 0..fee_clients.len() {
        let fee_client = fee_clients[i].clone();
        let payer = payer.clone();
        let instructions = instructions.clone();
        let mut priority_fee = priority_fee.clone();
        let core_id = cores[i % cores.len()];

        let handle = tokio::spawn(async move {
            core_affinity::set_for_current(core_id);

            let transaction = if fee_client.get_client_type() == ClientType::Rpc {
                build_buy_transaction(
                    payer.clone(),
                    priority_fee.clone(),
                    instructions.clone(),
                    lookup_table_key,
                    recent_blockhash,
                ).await?
            } else {
                let tip_account = fee_client.get_tip_account()?;
                let tip_account = Arc::new(Pubkey::from_str(&tip_account).map_err(|e| anyhow!(e))?);
                priority_fee.buy_tip_fee = priority_fee.buy_tip_fees[i];
                // println!(" 买入交易小费: {:?}", priority_fee.buy_tip_fee);
                build_buy_transaction_with_tip(
                    tip_account,
                    payer.clone(),
                    priority_fee.clone(),
                    instructions.clone(),
                    lookup_table_key,
                    recent_blockhash,
                ).await?
            };

            fee_client.send_transaction(TradeType::Buy, &transaction).await?;
            Ok::<(), anyhow::Error>(())
        });

        handles.push(handle);
    }

    println!(" 买入交易签名: {:?}", start_time.elapsed());

    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => (),
            Ok(Err(e)) => println!("Error in task: {}", e),
            Err(e) => println!("Task join error: {}", e),
        }
    }

    Ok(())
}

pub async fn build_buy_transaction(
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

pub async fn build_buy_transaction_with_tip(
    tip_account: Arc<Pubkey>,
    payer: Arc<Keypair>,
    priority_fee: PriorityFee,  
    build_instructions: Vec<Instruction>,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
) -> Result<VersionedTransaction, anyhow::Error> {
    // 从TipCache获取tip金额
    // let tip_cache = TipCache::get_instance();
    // let tip_amount = tip_cache.get_tip();
    // let tip_amount = priority_fee.buy_tip_fee;

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

pub async fn build_buy_instructions(
    payer: Arc<Keypair>,
    mint: Arc<Pubkey>,
    creator: Pubkey,
    dev_buy_token: u64,
    dev_sol_cost: u64,
    buy_sol_cost: u64,
    slippage_basis_points: Option<u64>,
    trade_type: String,
) -> Result<Vec<Instruction>, anyhow::Error> {
    if buy_sol_cost == 0 {
        return Err(anyhow!("Amount cannot be zero"));
    }

    let bonding_curve = if trade_type == SNIPER_BUY {
        init_bonding_curve_account(&mint, dev_buy_token, dev_sol_cost, creator).await?
    } else {
        let (bonding_curve, _) = get_bonding_curve_account_v2(&PumpFun::get_instance().get_rpc(), &mint).await?;
        Arc::new(crate::accounts::BondingCurveAccount {
            discriminator: bonding_curve.discriminator,
            account: get_bonding_curve_pda(&mint).unwrap(),
            virtual_token_reserves: bonding_curve.virtual_token_reserves,
            virtual_sol_reserves: bonding_curve.virtual_sol_reserves,
            real_token_reserves: bonding_curve.real_token_reserves,
            real_sol_reserves: bonding_curve.real_sol_reserves,
            token_total_supply: bonding_curve.token_total_supply,
            complete: bonding_curve.complete,
            creator: creator,
        })
    };

    let max_sol_cost = calculate_with_slippage_buy(buy_sol_cost, slippage_basis_points.unwrap_or(100));
    let creator_vault_pda = bonding_curve.get_creator_vault_pda();

    let mut buy_token_amount = get_buy_token_amount_from_sol_amount(&bonding_curve, buy_sol_cost);
    if buy_token_amount <= 100 * 1_000_000_u64 {
        buy_token_amount = if max_sol_cost > sol_to_lamports(0.01) {
            25547619 * 1_000_000_u64
        } else {
            255476 * 1_000_000_u64
        };
    }

    let mut instructions = vec![];
    instructions.push(create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &mint,
        &constants::pumpfun::accounts::TOKEN_PROGRAM,
    ));

    instructions.push(instruction::buy(
        payer.as_ref(),
        &mint,
        &bonding_curve.account,
        &creator_vault_pda,
        &FEE_RECIPIENT,
        instruction::Buy {
            _amount: buy_token_amount,
            _max_sol_cost: max_sol_cost,
        },
    ));

    Ok(instructions)
}

/// 验证地址表是否被成功用于编译后的消息中
fn verify_lookup_table_usage(
    v0_message: &v0::Message,
    address_lookup_table_accounts: &[AddressLookupTableAccount],
) {
    if !address_lookup_table_accounts.is_empty() {
        println!("消息已编译，使用了地址表引用");
        // 如果地址表有地址，但没有被使用，给出警告
        if v0_message.address_table_lookups.is_empty() {
            // println!("警告：编译后的消息没有使用地址表引用！");
        } else {
            for (i, lookup) in v0_message.address_table_lookups.iter().enumerate() {
                println!(
                    "使用地址表 {}: 可写索引 {} 个, 只读索引 {} 个",
                    i,
                    lookup.writable_indexes.len(),
                    lookup.readonly_indexes.len()
                );
            }
        }
    }
}