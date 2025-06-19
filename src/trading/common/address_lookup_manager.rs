use solana_sdk::{
    message::AddressLookupTableAccount,
    pubkey::Pubkey,
};

use crate::common::address_lookup_cache::get_address_lookup_table_account;

/// 获取地址查找表账户列表
/// 如果提供了lookup_table_key，则获取对应的账户，否则返回空列表
pub async fn get_address_lookup_table_accounts(
    lookup_table_key: Option<Pubkey>,
) -> Vec<AddressLookupTableAccount> {
    let mut address_lookup_table_accounts = vec![];
    
    if let Some(lookup_table_key) = lookup_table_key {
        let account = get_address_lookup_table_account(&lookup_table_key).await;
        address_lookup_table_accounts.push(account);
    }
    
    address_lookup_table_accounts
}