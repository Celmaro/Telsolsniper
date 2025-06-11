use crate::common::pumpswap::logs_data::PumpSwapInstruction;
use crate::common::pumpswap::logs_parser::parse_pumpswap_instruction;
use crate::common::pumpswap::logs_events::PumpSwapEvent;
use crate::constants::pumpswap::accounts;
use crate::error::ClientResult;
use solana_sdk::transaction::VersionedTransaction;

pub struct LogFilter;

impl LogFilter {
    /// 解析PumpSwap编译后的指令并返回指令类型和数据
    pub fn parse_pumpswap_compiled_instruction(
        versioned_tx: VersionedTransaction) -> ClientResult<Vec<PumpSwapInstruction>> {
        let compiled_instructions = versioned_tx.message.instructions();
        let accounts = versioned_tx.message.static_account_keys();
        let program_id = accounts::AMM_PROGRAM;
        let pump_index = accounts.iter().position(|key| key == &program_id);
        let mut instructions: Vec<PumpSwapInstruction> = Vec::new();

        if let Some(index) = pump_index {
            for instruction in compiled_instructions {
                if instruction.program_id_index as usize == index {
                    let all_accounts_valid = instruction.accounts.iter()
                        .all(|&acc_idx| (acc_idx as usize) < accounts.len());
                    if !all_accounts_valid {
                        continue;
                    }

                    if let Some(parsed_instruction) = parse_pumpswap_instruction(instruction, accounts) {
                        instructions.push(parsed_instruction);
                    }
                }
            }
        }

        Ok(instructions)
    }

    /// 解析PumpSwap交易日志并返回事件
    pub fn parse_pumpswap_logs(logs: &[String]) -> Vec<PumpSwapEvent> {
        PumpSwapEvent::parse_logs(logs)
    }
}