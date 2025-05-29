use solana_sdk::{message::AddressLookupTableAccount, pubkey::Pubkey};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// AddressLookupTableInfo 结构体，存储地址表相关信息
pub struct AddressLookupTableInfo {
    /// 地址表账户地址
    pub lookup_table_address: Option<Pubkey>,
    /// 地址表内容
    pub address_lookup_table: Option<AddressLookupTableAccount>,
    /// 锁定状态
    pub lock: bool,
}

/// AddressLookupTableCache 单例，用于存储和管理地址表
pub struct AddressLookupTableCache {
    /// 内部存储的地址表数据，键为地址表地址
    tables: Mutex<HashMap<Pubkey, AddressLookupTableInfo>>,
}

// 使用静态 OnceLock 确保单例模式的线程安全性
static ADDRESS_LOOKUP_TABLE_CACHE: OnceLock<Arc<AddressLookupTableCache>> = OnceLock::new();

impl AddressLookupTableCache {
    /// 获取 AddressLookupTableCache 单例实例
    pub fn get_instance() -> Arc<AddressLookupTableCache> {
        ADDRESS_LOOKUP_TABLE_CACHE
            .get_or_init(|| {
                Arc::new(AddressLookupTableCache {
                    tables: Mutex::new(HashMap::new()),
                })
            })
            .clone()
    }

    /// 添加或更新地址表信息
    pub fn add_or_update_table(
        &self,
        lookup_table_address: Pubkey,
        address_lookup_table: Option<AddressLookupTableAccount>,
        lock: Option<bool>,
    ) {
        let mut tables = self.tables.lock().unwrap();

        if let Some(table_info) = tables.get_mut(&lookup_table_address) {
            // 更新已存在的表
            if let Some(table) = address_lookup_table {
                table_info.address_lookup_table = Some(table);
            }

            if let Some(l) = lock {
                table_info.lock = l;
            }
        } else {
            // 添加新表
            tables.insert(
                lookup_table_address,
                AddressLookupTableInfo {
                    lookup_table_address: Some(lookup_table_address),
                    address_lookup_table,
                    lock: lock.unwrap_or(false),
                },
            );
        }
    }

    /// 移除地址表
    pub fn remove_table(&self, lookup_table_address: &Pubkey) -> bool {
        let mut tables = self.tables.lock().unwrap();
        tables.remove(lookup_table_address).is_some()
    }

    /// 获取地址表信息
    pub fn get_table(&self, lookup_table_address: &Pubkey) -> Option<AddressLookupTableInfo> {
        let tables = self.tables.lock().unwrap();

        tables.get(lookup_table_address).map(|info| AddressLookupTableInfo {
            lookup_table_address: info.lookup_table_address,
            address_lookup_table: info.address_lookup_table.clone(),
            lock: info.lock,
        })
    }

    /// 获取所有表地址
    pub fn get_all_table_addresses(&self) -> Vec<Pubkey> {
        let tables = self.tables.lock().unwrap();
        tables.keys().cloned().collect()
    }

    /// 检查表是否存在
    pub fn table_exists(&self, lookup_table_address: &Pubkey) -> bool {
        let tables = self.tables.lock().unwrap();
        tables.contains_key(lookup_table_address)
    }

    /// 锁定地址表
    pub fn lock_table(&self, lookup_table_address: &Pubkey) -> bool {
        let mut tables = self.tables.lock().unwrap();

        if let Some(table_info) = tables.get_mut(lookup_table_address) {
            table_info.lock = true;
            true
        } else {
            false
        }
    }

    /// 解锁地址表
    pub fn unlock_table(&self, lookup_table_address: &Pubkey) -> bool {
        let mut tables = self.tables.lock().unwrap();

        if let Some(table_info) = tables.get_mut(lookup_table_address) {
            table_info.lock = false;
            true
        } else {
            false
        }
    }

    /// 更新地址表内容
    pub fn update_table_content(
        &self,
        lookup_table_address: &Pubkey,
        address_lookup_table: AddressLookupTableAccount,
    ) -> bool {
        let mut tables = self.tables.lock().unwrap();

        if let Some(table_info) = tables.get_mut(lookup_table_address) {
            table_info.address_lookup_table = Some(address_lookup_table);
            true
        } else {
            false
        }
    }

    /// 获取表的内容
    pub fn get_table_content(&self, lookup_table_address: &Pubkey) -> AddressLookupTableAccount {
        let tables = self.tables.lock().unwrap();

        tables
            .get(lookup_table_address)
            .and_then(|info| info.address_lookup_table.clone())
            .unwrap_or_else(|| AddressLookupTableAccount {
                key: *lookup_table_address,
                addresses: Vec::new(),
            })
    }
}

/// 获取地址表账户
pub async fn get_address_lookup_table_account(lookup_table_address: &Pubkey) -> AddressLookupTableAccount {
    let cache = AddressLookupTableCache::get_instance();
    return cache.get_table_content(&lookup_table_address);
}