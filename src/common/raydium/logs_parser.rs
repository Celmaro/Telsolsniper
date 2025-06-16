use crate::common::raydium::{
    logs_data::{discriminators, RaydiumInstruction, V4SwapEvent},
    logs_events::RaydiumEvent,
};
use crate::error::ClientResult;

use solana_sdk::instruction::CompiledInstruction;
use solana_sdk::pubkey::Pubkey;
use std::time::{SystemTime, UNIX_EPOCH};

/// 获取当前时间戳
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

/// 从指令中解析Raydium指令
pub fn parse_raydium_instruction(
    instruction: &CompiledInstruction,
    _accounts: &[Pubkey],
    program_id: &Pubkey,
) -> Option<RaydiumInstruction> {
    if instruction.data.len() < 8 {
        return None;
    }

    if program_id == &crate::constants::raydium::accounts::CPMM_PROGRAM {
        let discriminator = &instruction.data[..8];
        let data = &instruction.data[8..];

        let accounts: Vec<Pubkey> = instruction
            .accounts
            .iter()
            .map(|&idx| _accounts[idx as usize])
            .collect();

        match discriminator {
            d if d == discriminators::SWAP_BASE_INPUT_IX => {
                if data.len() < 16 || accounts.len() < 12 {
                    return None;
                }
                let amount_in = u64::from_le_bytes(data[0..8].try_into().ok()?);
                let minimum_amount_out = u64::from_le_bytes(data[8..16].try_into().ok()?);

                Some(RaydiumInstruction::SwapBaseInput(
                    crate::common::raydium::SwapBaseInputEvent {
                        timestamp: current_timestamp(),
                        amount_in: amount_in,
                        minimum_amount_out: minimum_amount_out,

                        payer: accounts[0],
                        authority: accounts[1],
                        amm_config: accounts[2],
                        pool_state: accounts[3],
                        input_token_account: accounts[4],
                        output_token_account: accounts[5],
                        input_vault: accounts[6],
                        output_vault: accounts[7],
                        input_token_program: accounts[8],
                        output_token_program: accounts[9],
                        input_token_mint: accounts[10],
                        output_token_mint: accounts[11],
                        observation_state: accounts[12],
                        ..Default::default()
                    },
                ))
            }
            d if d == discriminators::SWAP_BASE_OUTPUT_IX => {
                println!("data.len(): {:?}", data.len());
                println!("accounts.len(): {:?}", accounts.len());
                if data.len() < 16 || accounts.len() < 12 {
                    return None;
                }
                let max_amount_in = u64::from_le_bytes(data[0..8].try_into().ok()?);
                let amount_out = u64::from_le_bytes(data[8..16].try_into().ok()?);

                Some(RaydiumInstruction::SwapBaseOutput(
                    crate::common::raydium::SwapBaseOutputEvent {
                        timestamp: current_timestamp(),
                        max_amount_in: max_amount_in,
                        amount_out: amount_out,

                        payer: accounts[0],
                        authority: accounts[1],
                        amm_config: accounts[2],
                        pool_state: accounts[3],
                        input_token_account: accounts[4],
                        output_token_account: accounts[5],
                        input_vault: accounts[6],
                        output_vault: accounts[7],
                        input_token_program: accounts[8],
                        output_token_program: accounts[9],
                        input_token_mint: accounts[10],
                        output_token_mint: accounts[11],
                        observation_state: accounts[12],
                        ..Default::default()
                    },
                ))
            }
            _ => Some(RaydiumInstruction::Other),
        }
    } else if program_id == &crate::constants::raydium::accounts::AMMV4_PROGRAM {
        let discriminator = &instruction.data[0];
        let data = &instruction.data[1..];

        let mut accounts: Vec<Pubkey> = instruction
            .accounts
            .iter()
            .map(|&idx| _accounts[idx as usize])
            .collect();

        match discriminator {
            d if d == discriminators::V4_SWAP_IX => {
                if data.len() < 16 || accounts.len() < 17 {
                    return None;
                }
                let amount_in = u64::from_le_bytes(data[0..8].try_into().ok()?);
                let minimum_amount_out = u64::from_le_bytes(data[8..16].try_into().ok()?);

                if accounts.len() == 17 {
                    accounts.insert(4, Pubkey::default());
                }

                Some(RaydiumInstruction::V4Swap(V4SwapEvent {
                    timestamp: current_timestamp(),
                    amount_in: amount_in,
                    minimum_amount_out: minimum_amount_out,
                    amm: accounts[1],
                    amm_authority: accounts[2],
                    amm_open_orders: accounts[3],
                    amm_target_orders: accounts[4],
                    pool_coin_token_account: accounts[5],
                    pool_pc_token_account: accounts[6],
                    serum_program: accounts[7],
                    serum_market: accounts[8],
                    serum_bids: accounts[9],
                    serum_asks: accounts[10],
                    serum_event_queue: accounts[11],
                    serum_coin_vault_account: accounts[12],
                    serum_pc_vault_account: accounts[13],
                    serum_vault_signer: accounts[14],
                    user_source_token_account: accounts[15],
                    user_destination_token_account: accounts[16],
                    user_source_owner: accounts[17],
                    ..Default::default()
                }))
            }
            _ => Some(RaydiumInstruction::Other),
        }
    } else {
        None
    }
}
