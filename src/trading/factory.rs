use anyhow::{anyhow, Result};
use std::sync::Arc;

use crate::instruction::{bonk::BonkInstructionBuilder, pumpfun::PumpFunInstructionBuilder, pumpswap::PumpSwapInstructionBuilder};

use super::{
    core::{executor::GenericTradeExecutor, traits::TradeExecutor},
};

/// 支持的交易协议
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradingProtocol {
    PumpFun,
    PumpSwap,
    Bonk,
}

impl std::fmt::Display for TradingProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradingProtocol::PumpFun => write!(f, "PumpFun"),
            TradingProtocol::PumpSwap => write!(f, "PumpSwap"),
            TradingProtocol::Bonk => write!(f, "Bonk"),
        }
    }
}

impl std::str::FromStr for TradingProtocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pumpfun" => Ok(TradingProtocol::PumpFun),
            "pumpswap" => Ok(TradingProtocol::PumpSwap),
            "bonk" => Ok(TradingProtocol::Bonk),
            _ => Err(anyhow!("Unsupported protocol: {}", s)),
        }
    }
}

/// 交易工厂 - 用于创建不同协议的交易执行器
pub struct TradeFactory;

impl TradeFactory {
    /// 创建指定协议的交易执行器
    pub fn create_executor(protocol: TradingProtocol) -> Arc<dyn TradeExecutor> {
        match protocol {
            TradingProtocol::PumpFun => {
                let instruction_builder = Arc::new(PumpFunInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(instruction_builder, "PumpFun"))
            }
            TradingProtocol::PumpSwap => {
                let instruction_builder = Arc::new(PumpSwapInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(instruction_builder, "PumpSwap"))
            }
            TradingProtocol::Bonk => {
                let instruction_builder = Arc::new(BonkInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(
                    instruction_builder,
                    "Bonk",
                ))
            }
        }
    }

    /// 获取所有支持的协议
    pub fn supported_protocols() -> Vec<TradingProtocol> {
        vec![
            TradingProtocol::PumpFun,
            TradingProtocol::PumpSwap,
            TradingProtocol::Bonk,
        ]
    }

    /// 检查协议是否支持
    pub fn is_supported(protocol: &TradingProtocol) -> bool {
        Self::supported_protocols().contains(protocol)
    }
}
