use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use solana_hash::Hash;

/// NonceInfo 结构体，存储 nonce 相关信息
pub struct NonceInfo {
    /// nonce 账户地址
    pub nonce_account: Option<Pubkey>,
    /// 当前 nonce 值
    pub current_nonce: Hash,
    /// 下次可用时间（Unix 时间戳，秒）
    pub next_buy_time: i64,
    /// 锁定状态
    pub lock: bool,
    /// 是否已使用
    pub used: bool,
}

/// NonceInfoStore 单例，用于存储和管理 NonceInfo
pub struct NonceCache {
    /// 内部存储的 NonceInfo 数据
    nonce_info: Mutex<NonceInfo>,
}

// 使用静态 OnceLock 确保单例模式的线程安全性
static NONCE_CACHE: OnceLock<Arc<NonceCache>> = OnceLock::new();

impl NonceCache {
    /// 获取 NonceInfoStore 单例实例
    pub fn get_instance() -> Arc<NonceCache> {
        NONCE_CACHE
            .get_or_init(|| {
                Arc::new(NonceCache {
                    nonce_info: Mutex::new(NonceInfo {
                        nonce_account: None,
                        current_nonce: Hash::default(),
                        next_buy_time: 0,
                        lock: false,
                        used: false,
                    }),
                })
            })
            .clone()
    }

    /// 初始化 nonce 信息
    pub fn init(&self, nonce_account_str: Option<String>) {
        let nonce_account = nonce_account_str
            .and_then(|s| Pubkey::from_str(&s).ok());

        self.update_nonce_info_partial(
            nonce_account,
            None,
            None,
            Some(false),
            Some(false),
        );
    }

     /// 获取 NonceInfo 的副本
     pub fn get_nonce_info(&self) -> NonceInfo {
        let nonce_info = self.nonce_info.lock().unwrap();
        NonceInfo {
            nonce_account: nonce_info.nonce_account,
            current_nonce: nonce_info.current_nonce,
            next_buy_time: nonce_info.next_buy_time,
            lock: nonce_info.lock,
            used: nonce_info.used,
        }
    }

    /// 部分更新 NonceInfo，只更新传入的字段
    pub fn update_nonce_info_partial(
        &self,
        nonce_account: Option<Pubkey>,
        current_nonce: Option<Hash>,
        next_buy_time: Option<i64>,
        lock: Option<bool>,
        used: Option<bool>,
    ) {
        let mut current = self.nonce_info.lock().unwrap();

        // 只更新传入的字段
        if let Some(account) = nonce_account {
            current.nonce_account = Some(account);
        }
        
        if let Some(nonce) = current_nonce {
            current.current_nonce = nonce;
        }
        
        if let Some(time) = next_buy_time {
            current.next_buy_time = time;
        }
        
        if let Some(l) = lock {
            current.lock = l;
        }
        
        if let Some(u) = used {
            current.used = u;
        }
    }

    /// 标记 nonce 已使用
    pub fn mark_used(&self) {
        self.update_nonce_info_partial(
            None,
            None,
            None,
            None,
            Some(true),
        );
    }

    /// 锁定 nonce
    pub fn lock(&self) {
        self.update_nonce_info_partial(
            None,
            None,
            None,
            Some(true),
            None,
        );
    }

    /// 解锁 nonce
    pub fn unlock(&self) {
        self.update_nonce_info_partial(
            None,
            None,
            None,
            Some(false),
            None,
        );
    }
}
