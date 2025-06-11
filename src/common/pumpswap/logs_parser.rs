use crate::error::ClientResult;
use crate::common::pumpswap::{
    logs_data::{
        PumpSwapInstruction,
        BuyEvent, SellEvent, CreatePoolEvent, DepositEvent, WithdrawEvent,
        DisableEvent, UpdateAdminEvent, UpdateFeeConfigEvent, discriminators
    },
    logs_events::PumpSwapEvent
};

use solana_sdk::pubkey::Pubkey;
use solana_sdk::instruction::CompiledInstruction;
use std::time::{SystemTime, UNIX_EPOCH};

/// 处理PumpSwap日志并调用回调函数
pub async fn process_pumpswap_logs<F>(
    signature: &str,
    logs: Vec<String>,
    slot: Option<u64>,
    callback: F,
) -> ClientResult<()>
where
    F: Fn(&str, PumpSwapEvent) + Send + Sync,
{
    let events = PumpSwapEvent::parse_logs(&logs);
    for mut event in events {
        // 设置签名和slot
        match &mut event {
            PumpSwapEvent::Buy(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            PumpSwapEvent::Sell(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            PumpSwapEvent::CreatePool(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            PumpSwapEvent::Deposit(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            PumpSwapEvent::Withdraw(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            PumpSwapEvent::Disable(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            PumpSwapEvent::UpdateAdmin(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            PumpSwapEvent::UpdateFeeConfig(e) => {
                e.signature = signature.to_string();
                if let Some(s) = slot {
                    e.slot = s;
                }
            },
            _ => {}
        }
        callback(signature, event);
    }
    Ok(())
}

/// 获取当前时间戳
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

/// 从指令中解析PumpSwap指令
pub fn parse_pumpswap_instruction(instruction: &CompiledInstruction, _accounts: &[Pubkey]) -> Option<PumpSwapInstruction> {
    if instruction.data.len() < 8 {
        return None;
    }

    let discriminator = &instruction.data[..8];
    let data = &instruction.data[8..];

    let accounts: Vec<Pubkey> = instruction.accounts.iter()
        .map(|&idx| _accounts[idx as usize])
        .collect();

    match discriminator {
        d if d == discriminators::BUY_IX => {
            // buy指令参数: base_amount_out: u64, max_quote_amount_in: u64
            // 账户顺序：pool, user, global_config, base_mint, quote_mint, user_base_token_account, 
            // user_quote_token_account, pool_base_token_account, pool_quote_token_account, 
            // protocol_fee_recipient, protocol_fee_recipient_token_account, ...
            if data.len() < 16 || accounts.len() < 11 {
                return None;
            }
            let base_amount_out = u64::from_le_bytes(data[0..8].try_into().ok()?);
            let max_quote_amount_in = u64::from_le_bytes(data[8..16].try_into().ok()?);

            Some(PumpSwapInstruction::Buy(BuyEvent {
                base_amount_out,
                max_quote_amount_in,
                pool: accounts[0],
                user: accounts[1],
                user_base_token_account: accounts[5],
                user_quote_token_account: accounts[6],
                protocol_fee_recipient: accounts[9],
                protocol_fee_recipient_token_account: accounts[10],
                timestamp: current_timestamp(),

                base_mint: accounts[3],
                quote_mint: accounts[4],
                pool_base_token_account: accounts[7],
                pool_quote_token_account: accounts[8],
                coin_creator_vault_ata: if accounts.len() > 17 { accounts[17] } else { Pubkey::default() },
                coin_creator_vault_authority: if accounts.len() > 18 { accounts[18] } else { Pubkey::default() }, 
                ..Default::default()
            }))
        },
        d if d == discriminators::SELL_IX => {
            // sell指令参数: base_amount_in: u64, min_quote_amount_out: u64
            // 账户顺序：pool, user, global_config, base_mint, quote_mint, user_base_token_account,
            // user_quote_token_account, pool_base_token_account, pool_quote_token_account,
            // protocol_fee_recipient, protocol_fee_recipient_token_account, ...
            if data.len() < 16 || accounts.len() < 11 {
                return None;
            }
            let base_amount_in = u64::from_le_bytes(data[0..8].try_into().ok()?);
            let min_quote_amount_out = u64::from_le_bytes(data[8..16].try_into().ok()?);
            
            Some(PumpSwapInstruction::Sell(SellEvent {
                base_amount_in,
                min_quote_amount_out,
                pool: accounts[0],
                user: accounts[1],
                user_base_token_account: accounts[5],
                user_quote_token_account: accounts[6],
                protocol_fee_recipient: accounts[9],
                protocol_fee_recipient_token_account: accounts[10],
                timestamp: current_timestamp(),
                
                base_mint: accounts[3],
                quote_mint: accounts[4],
                pool_base_token_account: accounts[7],
                pool_quote_token_account: accounts[8],
                coin_creator_vault_ata: if accounts.len() > 17 { accounts[17] } else { Pubkey::default() },
                coin_creator_vault_authority: if accounts.len() > 18 { accounts[18] } else { Pubkey::default() }, 
                ..Default::default()
            }))
        },
        d if d == discriminators::CREATE_POOL_IX => {
            // create_pool指令参数: index: u16, base_amount_in: u64, quote_amount_in: u64
            // 账户顺序：pool, global_config, creator, base_mint, quote_mint, lp_mint,
            // user_base_token_account, user_quote_token_account, user_pool_token_account,
            // pool_base_token_account, pool_quote_token_account, ...
            if data.len() < 18 || accounts.len() < 11 {
                return None;
            }
            let index = u16::from_le_bytes(data[0..2].try_into().ok()?);
            let base_amount_in = u64::from_le_bytes(data[2..10].try_into().ok()?);
            let quote_amount_in = u64::from_le_bytes(data[10..18].try_into().ok()?);
            
            Some(PumpSwapInstruction::CreatePool(CreatePoolEvent {
                index,
                base_amount_in,
                quote_amount_in,
                pool: accounts[0],
                creator: accounts[2],
                base_mint: accounts[3],
                quote_mint: accounts[4],
                lp_mint: accounts[5],
                user_base_token_account: accounts[6],
                user_quote_token_account: accounts[7],
                timestamp: current_timestamp(),
                ..Default::default()
            }))
        },
        d if d == discriminators::DEPOSIT_IX => {
            // deposit指令参数: lp_token_amount_out: u64, max_base_amount_in: u64, max_quote_amount_in: u64
            // 账户顺序：pool, global_config, user, base_mint, quote_mint, lp_mint,
            // user_base_token_account, user_quote_token_account, user_pool_token_account,
            // pool_base_token_account, pool_quote_token_account, ...
            if data.len() < 24 || accounts.len() < 11 {
                return None;
            }
            let lp_token_amount_out = u64::from_le_bytes(data[0..8].try_into().ok()?);
            let max_base_amount_in = u64::from_le_bytes(data[8..16].try_into().ok()?);
            let max_quote_amount_in = u64::from_le_bytes(data[16..24].try_into().ok()?);
            
            Some(PumpSwapInstruction::Deposit(DepositEvent {
                lp_token_amount_out,
                max_base_amount_in,
                max_quote_amount_in,
                pool: accounts[0],
                user: accounts[2],
                user_base_token_account: accounts[6],
                user_quote_token_account: accounts[7],
                user_pool_token_account: accounts[8],
                timestamp: current_timestamp(),
                ..Default::default()
            }))
        },
        d if d == discriminators::WITHDRAW_IX => {
            // withdraw指令参数: lp_token_amount_in: u64, min_base_amount_out: u64, min_quote_amount_out: u64
            // 账户顺序：pool, global_config, user, base_mint, quote_mint, lp_mint,
            // user_base_token_account, user_quote_token_account, user_pool_token_account,
            // pool_base_token_account, pool_quote_token_account, ...
            if data.len() < 24 || accounts.len() < 11 {
                return None;
            }
            let lp_token_amount_in = u64::from_le_bytes(data[0..8].try_into().ok()?);
            let min_base_amount_out = u64::from_le_bytes(data[8..16].try_into().ok()?);
            let min_quote_amount_out = u64::from_le_bytes(data[16..24].try_into().ok()?);
            
            Some(PumpSwapInstruction::Withdraw(WithdrawEvent {
                lp_token_amount_in,
                min_base_amount_out,
                min_quote_amount_out,
                pool: accounts[0],
                user: accounts[2],
                user_base_token_account: accounts[6],
                user_quote_token_account: accounts[7],
                user_pool_token_account: accounts[8],
                timestamp: current_timestamp(),
                ..Default::default()
            }))
        },
        d if d == discriminators::DISABLE_IX => {
            // disable指令参数: disable_create_pool: bool, disable_deposit: bool, disable_withdraw: bool, disable_buy: bool, disable_sell: bool
            // 账户顺序：admin, global_config, event_authority, program
            if data.len() < 5 || accounts.len() < 2 {
                return None;
            }
            let disable_create_pool = data[0] != 0;
            let disable_deposit = data[1] != 0;
            let disable_withdraw = data[2] != 0;
            let disable_buy = data[3] != 0;
            let disable_sell = data[4] != 0;
            
            Some(PumpSwapInstruction::Disable(DisableEvent {
                disable_create_pool,
                disable_deposit,
                disable_withdraw,
                disable_buy,
                disable_sell,
                admin: accounts[0],
                timestamp: current_timestamp(),
                ..Default::default()
            }))
        },
        d if d == discriminators::UPDATE_ADMIN_IX => {
            // update_admin指令参数: 无
            // 账户顺序：admin, global_config, new_admin, event_authority, program
            if accounts.len() < 3 {
                return None;
            }
            Some(PumpSwapInstruction::UpdateAdmin(UpdateAdminEvent {
                old_admin: accounts[0],
                new_admin: accounts[2],
                timestamp: current_timestamp(),
                ..Default::default()
            }))
        },
        d if d == discriminators::UPDATE_FEE_CONFIG_IX => {
            // update_fee_config指令参数: lp_fee_basis_points: u64, protocol_fee_basis_points: u64, protocol_fee_recipients: [pubkey; 8]
            // 账户顺序：admin, global_config, event_authority, program
            if data.len() < 272 || accounts.len() < 2 { // 8 + 8 + 32*8 = 272 bytes
                return None;
            }
            let lp_fee_basis_points = u64::from_le_bytes(data[0..8].try_into().ok()?);
            let protocol_fee_basis_points = u64::from_le_bytes(data[8..16].try_into().ok()?);
            
            let mut protocol_fee_recipients = [Pubkey::default(); 8];
            for i in 0..8 {
                let start = 16 + i * 32;
                let end = start + 32;
                if let Ok(pubkey_bytes) = data[start..end].try_into() {
                    protocol_fee_recipients[i] = Pubkey::new_from_array(pubkey_bytes);
                }
            }
            
            Some(PumpSwapInstruction::UpdateFeeConfig(UpdateFeeConfigEvent {
                admin: accounts[0],
                new_lp_fee_basis_points: lp_fee_basis_points,
                new_protocol_fee_basis_points: protocol_fee_basis_points,
                new_protocol_fee_recipients: protocol_fee_recipients,
                timestamp: current_timestamp(),
                ..Default::default()
            }))
        },
        _ => Some(PumpSwapInstruction::Other),
    }
}