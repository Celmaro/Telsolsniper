use dashmap::DashMap;
use solana_sdk::{message::AddressLookupTableAccount, pubkey::Pubkey};
use std::sync::{Arc, OnceLock};

/// AddressLookupTableInfo struct, stores address lookup table related information
#[derive(Clone)]
pub struct AddressLookupTableInfo {
    /// Address lookup table account address
    pub lookup_table_address: Option<Pubkey>,
    /// Address lookup table content
    pub address_lookup_table: Option<AddressLookupTableAccount>,
}

/// AddressLookupTableCache singleton for storing and managing address lookup tables
pub struct AddressLookupTableCache {
    /// Lock-free hash map supporting high concurrent access
    tables: DashMap<Pubkey, AddressLookupTableInfo>,
}

// Use static OnceLock to ensure thread safety of singleton pattern
static ADDRESS_LOOKUP_TABLE_CACHE: OnceLock<Arc<AddressLookupTableCache>> = OnceLock::new();

impl AddressLookupTableCache {
    /// Get AddressLookupTableCache singleton instance
    pub fn get_instance() -> Arc<AddressLookupTableCache> {
        ADDRESS_LOOKUP_TABLE_CACHE
            .get_or_init(|| Arc::new(AddressLookupTableCache { tables: DashMap::new() }))
            .clone()
    }

    /// Add or update address lookup table information - lock-free implementation
    pub fn add_or_update_table(
        &self,
        lookup_table_address: Pubkey,
        address_lookup_table: Option<AddressLookupTableAccount>,
    ) {
        if let Some(mut entry) = self.tables.get_mut(&lookup_table_address) {
            // Update existing table
            if let Some(table) = address_lookup_table {
                entry.address_lookup_table = Some(table);
            }
        } else {
            // Add new table
            self.tables.insert(
                lookup_table_address,
                AddressLookupTableInfo {
                    lookup_table_address: Some(lookup_table_address),
                    address_lookup_table,
                },
            );
        }
    }

    /// Remove address lookup table - lock-free implementation
    pub fn remove_table(&self, lookup_table_address: &Pubkey) -> bool {
        self.tables.remove(lookup_table_address).is_some()
    }

    /// Get address lookup table information - lock-free implementation
    pub fn get_table(&self, lookup_table_address: &Pubkey) -> Option<AddressLookupTableInfo> {
        self.tables.get(lookup_table_address).map(|entry| entry.value().clone())
    }

    /// Get all table addresses - lock-free implementation
    pub fn get_all_table_addresses(&self) -> Vec<Pubkey> {
        self.tables.iter().map(|entry| *entry.key()).collect()
    }

    /// Check if table exists - lock-free implementation
    pub fn table_exists(&self, lookup_table_address: &Pubkey) -> bool {
        self.tables.contains_key(lookup_table_address)
    }

    /// Update address lookup table content - lock-free implementation
    pub fn update_table_content(
        &self,
        lookup_table_address: &Pubkey,
        address_lookup_table: AddressLookupTableAccount,
    ) -> bool {
        if let Some(mut entry) = self.tables.get_mut(lookup_table_address) {
            entry.address_lookup_table = Some(address_lookup_table);
            true
        } else {
            false
        }
    }

    /// Get table content - high-performance lock-free implementation
    pub fn get_table_content(&self, lookup_table_address: &Pubkey) -> AddressLookupTableAccount {
        let result = self
            .tables
            .get(lookup_table_address)
            .and_then(|entry| entry.address_lookup_table.clone())
            .unwrap_or_else(|| AddressLookupTableAccount {
                key: *lookup_table_address,
                addresses: Vec::new(),
            });

        if result.addresses.len() == 0 {
            eprintln!(" ❌ Address lookup table account {} not setup", lookup_table_address);
            eprintln!(" ❌ Please update the address table account information using 【AddressLookupTableCache】 first");
            eprintln!(
                " ❌ The current transaction will not include this address lookup table account"
            );
        }

        return result;
    }
}

/// Get address lookup table account
pub async fn get_address_lookup_table_account(
    lookup_table_address: &Pubkey,
) -> AddressLookupTableAccount {
    let cache = AddressLookupTableCache::get_instance();
    return cache.get_table_content(&lookup_table_address);
}
