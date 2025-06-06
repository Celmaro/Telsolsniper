use base64::engine::general_purpose;
use base64::Engine;
use crate::common::pumpswap::logs_data::{
    BuyEvent, SellEvent, CreatePoolEvent, DepositEvent, WithdrawEvent,
    DisableEvent, UpdateAdminEvent, UpdateFeeConfigEvent, discriminators
};
use borsh::BorshDeserialize;

pub const PROGRAM_DATA: &str = "Program data: ";
pub const PROGRAM_LOG_PREFIX: &str = "Program log: PumpSwap: ";

/// PumpSwap事件枚举
#[derive(Debug)]
pub enum PumpSwapEvent {
    Buy(BuyEvent),
    Sell(SellEvent),
    CreatePool(CreatePoolEvent),
    Deposit(DepositEvent),
    Withdraw(WithdrawEvent),
    Disable(DisableEvent),
    UpdateAdmin(UpdateAdminEvent),
    UpdateFeeConfig(UpdateFeeConfigEvent),
    Error(String),
}

impl PumpSwapEvent {
    /// 解析日志并提取PumpSwap事件
    pub fn parse_logs(logs: &[String]) -> Vec<PumpSwapEvent> {
        let mut events = Vec::new();

        if logs.is_empty() {
            return events;
        }

        for log in logs {
            // 检查是否是事件日志
            if let Some(event_data) = log.strip_prefix(PROGRAM_DATA) {
                let borsh_bytes = match general_purpose::STANDARD.decode(event_data) {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };

                // 检查鉴别器
                if borsh_bytes.len() < 16 {
                    continue;
                }
                let prefix = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
                let discriminator = &[&prefix[..], &borsh_bytes[..8]].concat();
                let data = &borsh_bytes[8..];
                // 根据鉴别器解析不同类型的事件
                if discriminator == discriminators::BUY_EVENT {
                    if let Ok(mut event) = BuyEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::Buy(event));
                    }
                } else if discriminator == discriminators::SELL_EVENT {
                    if let Ok(mut event) = SellEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::Sell(event));
                    }
                } else if discriminator == discriminators::CREATE_POOL_EVENT {
                    if let Ok(mut event) = CreatePoolEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::CreatePool(event));
                    }
                } else if discriminator == discriminators::DEPOSIT_EVENT {
                    if let Ok(mut event) = DepositEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::Deposit(event));
                    }
                } else if discriminator == discriminators::WITHDRAW_EVENT {
                    if let Ok(mut event) = WithdrawEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::Withdraw(event));
                    }
                } else if discriminator == discriminators::DISABLE_EVENT {
                    if let Ok(mut event) = DisableEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::Disable(event));
                    }
                } else if discriminator == discriminators::UPDATE_ADMIN_EVENT {
                    if let Ok(mut event) = UpdateAdminEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::UpdateAdmin(event));
                    }
                } else if discriminator == discriminators::UPDATE_FEE_CONFIG_EVENT {
                    if let Ok(mut event) = UpdateFeeConfigEvent::deserialize(&mut &data[..]) {
                        event.signature = String::new(); // 在外部设置
                        events.push(PumpSwapEvent::UpdateFeeConfig(event));
                    }
                }
            } else if let Some(event_log) = log.strip_prefix(PROGRAM_LOG_PREFIX) {
                // 处理程序日志中的事件信息
                if event_log.contains("BuyEvent") {
                    // 这里可以添加从日志文本中解析事件的逻辑
                    // 例如使用正则表达式提取关键信息
                } else if event_log.contains("SellEvent") {
                    // 同上
                }
                // 其他事件类型...
            }
        }

        events
    }
}