use anyhow::{anyhow, Result};
use std::sync::Arc;

use crate::instruction::{bonk::BonkInstructionBuilder, pumpfun::PumpFunInstructionBuilder, pumpswap::PumpSwapInstructionBuilder};

use super::{
    core::{executor::GenericTradeExecutor, traits::TradeExecutor},
};

/// 支持的交易协议
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DexType {
    PumpFun,
    PumpSwap,
    Bonk,
}

impl std::fmt::Display for DexType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DexType::PumpFun => write!(f, "PumpFun"),
            DexType::PumpSwap => write!(f, "PumpSwap"),
            DexType::Bonk => write!(f, "Bonk"),
        }
    }
}

impl std::str::FromStr for DexType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pumpfun" => Ok(DexType::PumpFun),
            "pumpswap" => Ok(DexType::PumpSwap),
            "bonk" => Ok(DexType::Bonk),
            _ => Err(anyhow!("Unsupported protocol: {}", s)),
        }
    }
}

/// 交易工厂 - 用于创建不同协议的交易执行器
pub struct TradeFactory;

impl TradeFactory {
    /// 创建指定协议的交易执行器
    pub fn create_executor(protocol: DexType) -> Arc<dyn TradeExecutor> {
        match protocol {
            DexType::PumpFun => {
                let instruction_builder = Arc::new(PumpFunInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(instruction_builder, "PumpFun"))
            }
            DexType::PumpSwap => {
                let instruction_builder = Arc::new(PumpSwapInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(instruction_builder, "PumpSwap"))
            }
            DexType::Bonk => {
                let instruction_builder = Arc::new(BonkInstructionBuilder);
                Arc::new(GenericTradeExecutor::new(
                    instruction_builder,
                    "Bonk",
                ))
            }
        }
    }

    /// 获取所有支持的协议
    pub fn supported_protocols() -> Vec<DexType> {
        vec![DexType::PumpFun, DexType::PumpSwap, DexType::Bonk]
    }

    /// 检查协议是否支持
    pub fn is_supported(protocol: &DexType) -> bool {
        Self::supported_protocols().contains(protocol)
    }
}
