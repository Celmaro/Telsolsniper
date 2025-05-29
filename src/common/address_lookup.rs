use solana_program::{
    address_lookup_table::{
        instruction::{
            create_lookup_table as create_lookup_table_instruction, 
            extend_lookup_table as extend_lookup_table_instruction, 
            freeze_lookup_table as freeze_lookup_table_instruction
        },
        state::AddressLookupTable,
    },
    instruction::Instruction,
    pubkey::Pubkey,
};
use solana_sdk::{
    message::{v0::Message as MessageV0, AddressLookupTableAccount, VersionedMessage}, 
    signature::{Keypair, Signer}, 
    transaction::{Transaction, VersionedTransaction},
};
use std::{error::Error, sync::Arc};

use crate::{common::SolanaRpcClient, constants};

/// 创建地址查找表（如果不存在）
pub async fn create_lookup_table_if_not_exists(
    client: Arc<SolanaRpcClient>,
    authority: &Keypair,
    payer: &Keypair,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    // 1. 计算预期的查找表地址
    let recent_slot = client.get_slot().await?;
    let (create_ix, lookup_table_address) = create_lookup_table_instruction(
        authority.pubkey(), 
        payer.pubkey(),
        recent_slot
    );

    // 2. 创建新表
    let blockhash = client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    client.send_and_confirm_transaction(&transaction).await?;

    Ok(lookup_table_address)
}

/// 向查找表添加地址
pub async fn extend_lookup_table(
    client: Arc<SolanaRpcClient>,
    payer: &Keypair,
    authority: &Keypair,
    lookup_table_address: &Pubkey,
    addresses: Vec<Pubkey>,
) -> Result<(), Box<dyn Error>> {
    let extend_ix = extend_lookup_table_instruction(
        *lookup_table_address,
        authority.pubkey(),
        Some(payer.pubkey()),
        addresses.clone(),
    );

    let blockhash = client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[extend_ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    client.send_and_confirm_transaction(&transaction).await?;

    Ok(())
}

/// 冻结查找表，防止进一步修改
pub async fn freeze_lookup_table(
    client: Arc<SolanaRpcClient>,
    payer: &Keypair,
    authority: &Keypair,
    lookup_table_address: &Pubkey,
) -> Result<(), Box<dyn Error>> {
    let freeze_ix = freeze_lookup_table_instruction(
        *lookup_table_address,
        authority.pubkey(),
    );

    let blockhash = client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[freeze_ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    client.send_and_confirm_transaction(&transaction).await?;

    Ok(())
}

/// 获取查找表信息
pub async fn get_address_lookup_table(
    client: Arc<SolanaRpcClient>,
    lookup_table_address: &Pubkey,
) -> Result<AddressLookupTableAccount, Box<dyn Error>> {
    let account = client.get_account(lookup_table_address).await?;
    let lookup_table = AddressLookupTable::deserialize(&account.data)?;

    let address_lookup_table_account = AddressLookupTableAccount {
        key: *lookup_table_address,
        addresses: lookup_table.addresses.to_vec(),
    };

    for (i, addr) in address_lookup_table_account.addresses.iter().enumerate() {
        println!("地址 {}: {}", i, addr);
    }

    Ok(address_lookup_table_account)
}

/// 使用查找表发送交易
pub async fn send_transaction_with_lut(
    client: Arc<SolanaRpcClient>,
    instructions: Vec<Instruction>,
    payer: &Keypair,
    signers: Vec<&Keypair>,
    address_lookup_tables: Vec<AddressLookupTableAccount>,
) -> Result<(), Box<dyn Error>> {
    let blockhash = client.get_latest_blockhash().await?;

    let message = VersionedMessage::V0(MessageV0::try_compile(
        &payer.pubkey(),
        &instructions,
        &address_lookup_tables,
        blockhash,
    )?);

    let tx = VersionedTransaction::try_new(message, &signers)?;

    let signature = client.send_and_confirm_transaction(&tx).await?;

    println!("交易已确认: {}", signature);
    Ok(())
}

/// 使用查找表的特定地址子集发送交易
pub async fn send_transaction_with_filtered_lut(
    client: Arc<SolanaRpcClient>,
    instructions: Vec<Instruction>,
    payer: &Keypair,
    signers: Vec<&Keypair>,
    lookup_table: AddressLookupTableAccount,
    address_indices_to_use: &[usize], // 要使用的地址索引列表
) -> Result<(), Box<dyn Error>> {
    // 创建只包含选定地址的新查找表账户
    let filtered_addresses: Vec<Pubkey> = address_indices_to_use
        .iter()
        .filter_map(|&index| lookup_table.addresses.get(index).copied())
        .collect();

    println!(
        "从查找表中选择了 {} 个地址用于交易",
        filtered_addresses.len()
    );
    for (i, addr) in filtered_addresses.iter().enumerate() {
        println!("使用地址 {}: {}", i, addr);
    }

    let filtered_lookup_table = AddressLookupTableAccount {
        key: lookup_table.key,
        addresses: filtered_addresses,
    };

    let blockhash = client.get_latest_blockhash().await?;

    let message = VersionedMessage::V0(MessageV0::try_compile(
        &payer.pubkey(),
        &instructions,
        &[filtered_lookup_table],
        blockhash,
    )?);

    let tx = VersionedTransaction::try_new(message, &signers)?;

    let signature = client.send_and_confirm_transaction(&tx).await?;

    println!("交易已确认: {}", signature);
    Ok(())
}

/// 获取最近的区块槽位，用于创建查找表
pub async fn get_recent_slot(client: Arc<SolanaRpcClient>) -> Result<u64, Box<dyn Error>> {
    let slot = client.get_slot().await?;
    Ok(slot)
}

/// 使用指定的地址列表发送交易
///
/// 这个方法接受一组目标地址，自动查找它们在查找表中的索引，
/// 然后使用这些地址创建一个过滤后的查找表来发送交易
///
/// # 参数
/// * `instructions` - 交易指令
/// * `payer` - 支付交易费用的账户
/// * `signers` - 交易签名者
/// * `lookup_table` - 地址查找表
/// * `addresses_to_use` - 要使用的地址列表
///
/// # 返回值
/// 成功返回交易签名，失败返回错误
pub async fn send_transaction_with_addresses(
    client: Arc<SolanaRpcClient>,
    instructions: Vec<Instruction>,
    payer: &Keypair,
    signers: Vec<&Keypair>,
    lookup_table: AddressLookupTableAccount,
    addresses_to_use: &[Pubkey],
) -> Result<String, Box<dyn Error>> {
    // 构建地址到索引的映射
    let mut address_to_index = std::collections::HashMap::new();
    for (i, addr) in lookup_table.addresses.iter().enumerate() {
        address_to_index.insert(*addr, i);
    }

    // 查找所有存在的地址的索引
    let mut indices_to_use = Vec::new();
    let mut found_addresses = Vec::new();
    let mut missing_addresses = Vec::new();

    for addr in addresses_to_use {
        if let Some(&index) = address_to_index.get(addr) {
            indices_to_use.push(index);
            found_addresses.push(*addr);
        } else {
            missing_addresses.push(*addr);
        }
    }

    // 检查是否有地址未找到
    if !missing_addresses.is_empty() {
        println!("警告: {} 个地址未在查找表中找到", missing_addresses.len());
        for (i, addr) in missing_addresses.iter().enumerate() {
            println!("未找到的地址 {}: {}", i, addr);
        }
    }

    // 如果没有找到任何地址，返回错误
    if indices_to_use.is_empty() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "没有在查找表中找到任何指定的地址",
        )));
    }

    // 创建只包含选定地址的新查找表账户
    let filtered_addresses: Vec<Pubkey> = indices_to_use
        .iter()
        .filter_map(|&index| lookup_table.addresses.get(index).copied())
        .collect();

    println!(
        "从查找表中选择了 {} 个地址用于交易",
        filtered_addresses.len()
    );
    for (i, addr) in filtered_addresses.iter().enumerate() {
        println!("使用地址 {}: {}", i, addr);
    }

    let filtered_lookup_table = AddressLookupTableAccount {
        key: lookup_table.key,
        addresses: filtered_addresses,
    };

    let blockhash = client.get_latest_blockhash().await?;

    let message = VersionedMessage::V0(MessageV0::try_compile(
        &payer.pubkey(),
        &instructions,
        &[filtered_lookup_table],
        blockhash,
    )?);

    let tx = VersionedTransaction::try_new(message, &signers)?;

    let signature = client.send_and_confirm_transaction(&tx).await?;

    println!("交易已确认: {}", signature);
    Ok(signature.to_string())
}

pub async fn create_pumpfun_lookup_table(
    client: Arc<SolanaRpcClient>,
    payer: &Keypair,
    authority: &Keypair,
) -> Result<Pubkey, Box<dyn Error>> {
    let recent_slot = client.get_slot().await?;
    let (create_ix, lookup_table_address) =
        create_lookup_table_instruction(authority.pubkey(), payer.pubkey(), recent_slot);

    let blockhash = client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    client.send_and_confirm_transaction(&transaction).await?;

    Ok(lookup_table_address)
}   

pub async fn add_pumpfun_address_to_lookup_table(
    client: Arc<SolanaRpcClient>,
    payer: &Keypair,
    authority: &Keypair,
    lookup_table_address: &Pubkey,
) -> Result<(), Box<dyn Error>> {
    let addresses = get_pumpfun_addresses(payer.pubkey(), vec![]);
    extend_lookup_table(
        client,
        payer, 
        authority, 
        lookup_table_address, 
        addresses
    ).await?;

    Ok(())
}

pub async fn extend_pumpfun_address_to_lookup_table(
    client: Arc<SolanaRpcClient>,
    payer: &Keypair,
    authority: &Keypair,
    lookup_table_address: &Pubkey,
    addresses: Vec<Pubkey>,
) -> Result<(), Box<dyn Error>> {
    extend_lookup_table(
        client,
        payer, 
        authority, 
        lookup_table_address, 
        addresses
    ).await?;

    Ok(())
}

pub fn get_pumpfun_addresses(payer: Pubkey, include_addresses: Vec<Pubkey>) -> Vec<Pubkey> {
    let mut addresses = vec![
        payer,
        constants::accounts::PUMPFUN,
        constants::accounts::SYSTEM_PROGRAM,
        constants::accounts::TOKEN_PROGRAM,
        constants::accounts::RENT,
        constants::accounts::EVENT_AUTHORITY,
        constants::accounts::ASSOCIATED_TOKEN_PROGRAM,
        constants::global_constants::GLOBAL_ACCOUNT,
        constants::global_constants::FEE_RECIPIENT,
    ];

    addresses.extend(include_addresses);
    
    addresses
}

pub fn get_pumpfun_filtered_addresses(payer: Pubkey, include_addresses: Vec<Pubkey>) -> Vec<Pubkey> {
    let mut addresses = vec![
        payer,
        constants::accounts::PUMPFUN, 
        constants::accounts::SYSTEM_PROGRAM,
        constants::accounts::TOKEN_PROGRAM,
        constants::accounts::RENT,
        constants::accounts::EVENT_AUTHORITY,
        constants::accounts::ASSOCIATED_TOKEN_PROGRAM,
        constants::global_constants::GLOBAL_ACCOUNT,
        constants::global_constants::FEE_RECIPIENT,
        constants::global_constants::PUMPFUN_AMM_FEE_1,
        constants::global_constants::PUMPFUN_AMM_FEE_2,
        constants::global_constants::PUMPFUN_AMM_FEE_3,
        constants::global_constants::PUMPFUN_AMM_FEE_4,
        constants::global_constants::PUMPFUN_AMM_FEE_5,
        constants::global_constants::PUMPFUN_AMM_FEE_6,
        constants::global_constants::PUMPFUN_AMM_FEE_7,
        // constants::global_constants::PUMPFUN_AMM_FEE_8,
    ];

    addresses.extend(include_addresses);
    
    addresses
}
