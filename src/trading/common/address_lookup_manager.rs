use solana_sdk::{message::AddressLookupTableAccount, pubkey::Pubkey};

use crate::common::address_lookup_cache::get_address_lookup_table_account;

/// Get address lookup table account list
/// If lookup_table_key is provided, get the corresponding account, otherwise return empty list
pub async fn get_address_lookup_table_accounts(
    lookup_table_key: Option<Pubkey>,
) -> Vec<AddressLookupTableAccount> {
    match lookup_table_key {
        Some(key) => {
            let account = get_address_lookup_table_account(&key).await;
            vec![account]
        }
        None => Vec::new(),
    }
}
