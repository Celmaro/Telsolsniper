use solana_sdk::{compute_budget::ComputeBudgetInstruction, instruction::Instruction};

use crate::common::PriorityFee;

/// 为交易添加计算预算指令
pub fn add_compute_budget_instructions(
    instructions: &mut Vec<Instruction>,
    priority_fee: &PriorityFee,
    data_size_limit: u32,
    is_rpc: bool,
    is_buy: bool,
) {
    if is_buy {
        instructions
            .push(ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(data_size_limit));
    }
    if is_rpc {
        instructions
            .push(ComputeBudgetInstruction::set_compute_unit_price(priority_fee.rpc_unit_price));
        instructions
            .push(ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.rpc_unit_limit));
    } else {
        instructions
            .push(ComputeBudgetInstruction::set_compute_unit_price(priority_fee.tip_unit_price));
        instructions
            .push(ComputeBudgetInstruction::set_compute_unit_limit(priority_fee.tip_unit_limit));
    }
}
