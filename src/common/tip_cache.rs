use std::sync::{Arc, Mutex, OnceLock};

/// TipCache 单例，用于存储和管理 tip 金额
pub struct TipCache {
    /// tip 金额
    tip_amount: Mutex<f64>,
}

static TIP_CACHE: OnceLock<Arc<TipCache>> = OnceLock::new();

impl TipCache {
    /// 获取 TipCache 单例实例
    pub fn get_instance() -> Arc<TipCache> {
        TIP_CACHE
            .get_or_init(|| {
                Arc::new(TipCache {
                    tip_amount: Mutex::new(0.001),
                })
            })
            .clone()
    }

    /// 初始化 tip 金额
    pub fn init(&self, tip_amount: Option<f64>) {
        let amount = tip_amount.unwrap_or(0.001);
        self.update_tip(amount);
    }

    /// 获取 tip 金额
    pub fn get_tip(&self) -> f64 {
        *self.tip_amount.lock().unwrap()
    }

    /// 更新 tip 金额
    pub fn update_tip(&self, amount: f64) {
        *self.tip_amount.lock().unwrap() = amount;
    }
}