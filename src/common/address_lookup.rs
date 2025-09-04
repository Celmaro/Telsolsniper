use solana_program::{
    address_lookup_table::{
        instruction::{
            create_lookup_table as create_lookup_table_instruction,
            extend_lookup_table as extend_lookup_table_instruction,
            freeze_lookup_table as freeze_lookup_table_instruction,
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

/// Create address lookup table (if it doesn't exist)
pub async fn create_lookup_table_if_not_exists(
    client: Arc<SolanaRpcClient>,
    authority: &Keypair,
    payer: &Keypair,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    // 1. Calculate the expected lookup table address
    let recent_slot = client.get_slot().await?;
    let (create_ix, lookup_table_address) =
        create_lookup_table_instruction(authority.pubkey(), payer.pubkey(), recent_slot);

    // 2. Create new table
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

/// Add addresses to lookup table
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

/// Freeze lookup table to prevent further modifications
pub async fn freeze_lookup_table(
    client: Arc<SolanaRpcClient>,
    payer: &Keypair,
    authority: &Keypair,
    lookup_table_address: &Pubkey,
) -> Result<(), Box<dyn Error>> {
    let freeze_ix = freeze_lookup_table_instruction(*lookup_table_address, authority.pubkey());

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

/// Get lookup table information
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

    Ok(address_lookup_table_account)
}

/// Send transaction using lookup table
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

    println!("Transaction confirmed: {}", signature);
    Ok(())
}

/// Send transaction using a specific subset of addresses from lookup table
pub async fn send_transaction_with_filtered_lut(
    client: Arc<SolanaRpcClient>,
    instructions: Vec<Instruction>,
    payer: &Keypair,
    signers: Vec<&Keypair>,
    lookup_table: AddressLookupTableAccount,
    address_indices_to_use: &[usize], // List of address indices to use
) -> Result<(), Box<dyn Error>> {
    // Create a new lookup table account containing only selected addresses
    let filtered_addresses: Vec<Pubkey> = address_indices_to_use
        .iter()
        .filter_map(|&index| lookup_table.addresses.get(index).copied())
        .collect();

    println!("Selected {} addresses from lookup table for transaction", filtered_addresses.len());
    for (i, addr) in filtered_addresses.iter().enumerate() {
        println!("Using address {}: {}", i, addr);
    }

    let filtered_lookup_table =
        AddressLookupTableAccount { key: lookup_table.key, addresses: filtered_addresses };

    let blockhash = client.get_latest_blockhash().await?;

    let message = VersionedMessage::V0(MessageV0::try_compile(
        &payer.pubkey(),
        &instructions,
        &[filtered_lookup_table],
        blockhash,
    )?);

    let tx = VersionedTransaction::try_new(message, &signers)?;

    let signature = client.send_and_confirm_transaction(&tx).await?;

    println!("Transaction confirmed: {}", signature);
    Ok(())
}

/// Get recent block slot for creating lookup table
pub async fn get_recent_slot(client: Arc<SolanaRpcClient>) -> Result<u64, Box<dyn Error>> {
    let slot = client.get_slot().await?;
    Ok(slot)
}

/// Send transaction using specified address list
///
/// This method accepts a set of target addresses, automatically finds their indices in the lookup table,
/// then uses these addresses to create a filtered lookup table for sending transactions
///
/// # Arguments
/// * `instructions` - Transaction instructions
/// * `payer` - Account that pays transaction fees
/// * `signers` - Transaction signers
/// * `lookup_table` - Address lookup table
/// * `addresses_to_use` - List of addresses to use
///
/// # Returns
/// Returns transaction signature on success, error on failure
pub async fn send_transaction_with_addresses(
    client: Arc<SolanaRpcClient>,
    instructions: Vec<Instruction>,
    payer: &Keypair,
    signers: Vec<&Keypair>,
    lookup_table: AddressLookupTableAccount,
    addresses_to_use: &[Pubkey],
) -> Result<String, Box<dyn Error>> {
    // Build address to index mapping
    let mut address_to_index = std::collections::HashMap::new();
    for (i, addr) in lookup_table.addresses.iter().enumerate() {
        address_to_index.insert(*addr, i);
    }

    // Find indices of all existing addresses
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

    // Check if any addresses were not found
    if !missing_addresses.is_empty() {
        println!("Warning: {} addresses not found in lookup table", missing_addresses.len());
        for (i, addr) in missing_addresses.iter().enumerate() {
            println!("Address not found {}: {}", i, addr);
        }
    }

    // Return error if no addresses were found
    if indices_to_use.is_empty() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No specified addresses found in lookup table",
        )));
    }

    // Create a new lookup table account containing only selected addresses
    let filtered_addresses: Vec<Pubkey> = indices_to_use
        .iter()
        .filter_map(|&index| lookup_table.addresses.get(index).copied())
        .collect();

    println!("Selected {} addresses from lookup table for transaction", filtered_addresses.len());
    for (i, addr) in filtered_addresses.iter().enumerate() {
        println!("Using address {}: {}", i, addr);
    }

    let filtered_lookup_table =
        AddressLookupTableAccount { key: lookup_table.key, addresses: filtered_addresses };

    let blockhash = client.get_latest_blockhash().await?;

    let message = VersionedMessage::V0(MessageV0::try_compile(
        &payer.pubkey(),
        &instructions,
        &[filtered_lookup_table],
        blockhash,
    )?);

    let tx = VersionedTransaction::try_new(message, &signers)?;

    let signature = client.send_and_confirm_transaction(&tx).await?;

    println!("Transaction confirmed: {}", signature);
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
    extend_lookup_table(client, payer, authority, lookup_table_address, addresses).await?;

    Ok(())
}

pub async fn extend_pumpfun_address_to_lookup_table(
    client: Arc<SolanaRpcClient>,
    payer: &Keypair,
    authority: &Keypair,
    lookup_table_address: &Pubkey,
    addresses: Vec<Pubkey>,
) -> Result<(), Box<dyn Error>> {
    extend_lookup_table(client, payer, authority, lookup_table_address, addresses).await?;

    Ok(())
}

pub fn get_pumpfun_addresses(payer: Pubkey, include_addresses: Vec<Pubkey>) -> Vec<Pubkey> {
    let mut addresses = vec![
        payer,
        constants::pumpfun::accounts::PUMPFUN,
        constants::pumpfun::accounts::SYSTEM_PROGRAM,
        constants::pumpfun::accounts::TOKEN_PROGRAM,
        constants::pumpfun::accounts::RENT,
        constants::pumpfun::accounts::EVENT_AUTHORITY,
        constants::pumpfun::accounts::ASSOCIATED_TOKEN_PROGRAM,
        constants::pumpfun::global_constants::GLOBAL_ACCOUNT,
        constants::pumpfun::global_constants::FEE_RECIPIENT,
    ];

    addresses.extend(include_addresses);

    addresses
}

pub fn get_pumpfun_filtered_addresses(
    payer: Pubkey,
    include_addresses: Vec<Pubkey>,
) -> Vec<Pubkey> {
    let mut addresses = vec![
        payer,
        constants::pumpfun::accounts::PUMPFUN,
        constants::pumpfun::accounts::SYSTEM_PROGRAM,
        constants::pumpfun::accounts::TOKEN_PROGRAM,
        constants::pumpfun::accounts::RENT,
        constants::pumpfun::accounts::EVENT_AUTHORITY,
        constants::pumpfun::accounts::ASSOCIATED_TOKEN_PROGRAM,
        constants::pumpfun::global_constants::GLOBAL_ACCOUNT,
        constants::pumpfun::global_constants::FEE_RECIPIENT,
        constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_1,
        constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_2,
        constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_3,
        constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_4,
        constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_5,
        constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_6,
        constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_7,
        // constants::pumpfun::global_constants::PUMPFUN_AMM_FEE_8,
    ];

    addresses.extend(include_addresses);

    addresses
}
