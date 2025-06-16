use crate::common::raydium::logs_data::RaydiumInstruction;
use crate::common::raydium::logs_parser::parse_raydium_instruction;
use crate::constants::raydium::accounts;
use crate::error::ClientResult;
use solana_sdk::transaction::VersionedTransaction;

pub struct LogFilter;

impl LogFilter {
    /// 解析Raydium编译后的指令并返回指令类型和数据
    pub fn parse_raydium_compiled_instruction(
        versioned_tx: VersionedTransaction,
    ) -> ClientResult<Vec<RaydiumInstruction>> {
        let compiled_instructions = versioned_tx.message.instructions();
        let accounts = versioned_tx.message.static_account_keys();
        let ammv4_program_id = accounts::AMMV4_PROGRAM;
        let cpmm_program_id = accounts::CPMM_PROGRAM;
        let raydium_index = accounts.iter().position(|key| key == &ammv4_program_id);
        let cpmm_index = accounts.iter().position(|key| key == &cpmm_program_id);
        let mut instructions: Vec<RaydiumInstruction> = Vec::new();
        if let Some(index) = raydium_index {
            for instruction in compiled_instructions {
                if instruction.program_id_index as usize == index {
                    let all_accounts_valid = instruction
                        .accounts
                        .iter()
                        .all(|&acc_idx| (acc_idx as usize) < accounts.len());
                    if !all_accounts_valid {
                        continue;
                    }

                    if let Some(parsed_instruction) =
                        parse_raydium_instruction(instruction, accounts, &ammv4_program_id)
                    {
                        instructions.push(parsed_instruction);
                    }
                }
            }
        }
        if let Some(index) = cpmm_index {
            for instruction in compiled_instructions {
                if instruction.program_id_index as usize == index {
                    let all_accounts_valid = instruction
                        .accounts
                        .iter()
                        .all(|&acc_idx| (acc_idx as usize) < accounts.len());
                    if !all_accounts_valid {
                        continue;
                    }

                    if let Some(parsed_instruction) =
                        parse_raydium_instruction(instruction, accounts, &cpmm_program_id)
                    {
                        instructions.push(parsed_instruction);
                    }
                }
            }
        }

        Ok(instructions)
    }
}
