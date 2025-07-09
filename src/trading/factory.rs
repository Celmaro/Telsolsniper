use anyhow::{anyhow, Result};
use std::sync::Arc;

use crate::trading::protocols::bonk::BonkInstructionBuilder;

use super::{
    core::{executor::GenericTradeExecutor, traits::TradeExecutor},
    protocols::{pumpfun::PumpFunInstructionBuilder, pumpswap::PumpSwapInstructionBuilder},
};

/// 支持的交易协议
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    PumpFun,
    PumpSwap,
    Bonk,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::PumpFun => write!(f, "PumpFun"),
            Protocol::PumpSwap => write!(f, "PumpSwap"),
            Protocol::Bonk => write!(f, "Bonk"),
        }
    }
}

impl std::str::FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pumpfun" => Ok(Protocol::PumpFun),
            "pumpswap" => Ok(Protocol::PumpSwap),
            "bonk" => Ok(Protocol::Bonk),
            _ => Err(anyhow!("Unsupported protocol: {}", s)),
        }
    }
}

/// 交易工厂 - 用于创建不同协议的交易执行器
pub struct TradeFactory;

impl TradeFactory {
    /// 创建指定协议的交易执行器
    pub fn create_executor(protocol: Protocol) -> Arc<dyn TradeExecutor> {
        match protocol {
            Protocol::PumpFun => {
                let instruction_builder = Arc::new(PumpFunInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(instruction_builder, "PumpFun"))
            }
            Protocol::PumpSwap => {
                let instruction_builder = Arc::new(PumpSwapInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(instruction_builder, "PumpSwap"))
            }
            Protocol::Bonk => {
                let instruction_builder = Arc::new(BonkInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(
                    instruction_builder,
                    "Bonk",
                ))
            }
        }
    }

    /// 获取所有支持的协议
    pub fn supported_protocols() -> Vec<Protocol> {
        vec![
            Protocol::PumpFun,
            Protocol::PumpSwap,
            Protocol::Bonk,
        ]
    }

    /// 检查协议是否支持
    pub fn is_supported(protocol: &Protocol) -> bool {
        Self::supported_protocols().contains(protocol)
    }
}
