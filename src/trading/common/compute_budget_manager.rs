use crate::common::PriorityFee;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use smallvec::SmallVec;
use solana_sdk::{compute_budget::ComputeBudgetInstruction, instruction::Instruction};

/// 缓存键，包含计算预算指令的所有参数
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ComputeBudgetCacheKey {
    data_size_limit: u32,
    unit_price: u64,
    unit_limit: u32,
    is_buy: bool,
}

/// 全局缓存，存储计算预算指令
/// 使用 DashMap 提供高性能的无锁并发访问
static COMPUTE_BUDGET_CACHE: Lazy<DashMap<ComputeBudgetCacheKey, SmallVec<[Instruction; 3]>>> =
    Lazy::new(|| DashMap::new());

#[inline(always)]
pub fn compute_budget_instructions(
    priority_fee: &PriorityFee,
    data_size_limit: u32,
    is_rpc: bool,
    is_buy: bool,
) -> SmallVec<[Instruction; 3]> {
    let (unit_price, unit_limit) = if is_rpc {
        (priority_fee.rpc_unit_price, priority_fee.rpc_unit_limit)
    } else {
        (priority_fee.tip_unit_price, priority_fee.tip_unit_limit)
    };

    // 创建缓存键
    let cache_key = ComputeBudgetCacheKey { data_size_limit, unit_price, unit_limit, is_buy };

    // 先尝试从缓存中获取
    if let Some(cached_insts) = COMPUTE_BUDGET_CACHE.get(&cache_key) {
        return cached_insts.clone();
    }

    // 缓存未命中，生成新的指令
    let mut insts = SmallVec::<[Instruction; 3]>::new();

    if is_buy {
        insts.push(ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(data_size_limit));
    }

    insts.extend([
        ComputeBudgetInstruction::set_compute_unit_price(unit_price),
        ComputeBudgetInstruction::set_compute_unit_limit(unit_limit),
    ]);

    // 将结果存入缓存
    let insts_clone = insts.clone();
    COMPUTE_BUDGET_CACHE.insert(cache_key, insts_clone);

    insts
}
