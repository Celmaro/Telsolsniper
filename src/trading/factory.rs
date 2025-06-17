use anyhow::{anyhow, Result};
use std::sync::Arc;

use super::{
    core::{executor::GenericTradeExecutor, traits::TradeExecutor},
    protocols::{pumpfun::PumpFunInstructionBuilder, pumpswap::PumpSwapInstructionBuilder},
};

/// 支持的交易协议
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    PumpFun,
    PumpSwap,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::PumpFun => write!(f, "PumpFun"),
            Protocol::PumpSwap => write!(f, "PumpSwap"),
        }
    }
}

impl std::str::FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pumpfun" => Ok(Protocol::PumpFun),
            "pumpswap" => Ok(Protocol::PumpSwap),
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
        }
    }

    /// 获取所有支持的协议
    pub fn supported_protocols() -> Vec<Protocol> {
        vec![Protocol::PumpFun, Protocol::PumpSwap]
    }

    /// 检查协议是否支持
    pub fn is_supported(protocol: &Protocol) -> bool {
        Self::supported_protocols().contains(protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_from_str() {
        assert_eq!("pumpfun".parse::<Protocol>().unwrap(), Protocol::PumpFun);
        assert_eq!("pumpswap".parse::<Protocol>().unwrap(), Protocol::PumpSwap);
        assert_eq!("PUMPFUN".parse::<Protocol>().unwrap(), Protocol::PumpFun);
        assert!("unknown".parse::<Protocol>().is_err());
    }

    #[test]
    fn test_create_executor() {
        let pumpfun_executor = TradeFactory::create_executor(Protocol::PumpFun);
        assert_eq!(pumpfun_executor.protocol_name(), "PumpFun");

        let pumpswap_executor = TradeFactory::create_executor(Protocol::PumpSwap);
        assert_eq!(pumpswap_executor.protocol_name(), "PumpSwap");
    }

    #[test]
    fn test_supported_protocols() {
        let protocols = TradeFactory::supported_protocols();
        assert!(protocols.contains(&Protocol::PumpFun));
        assert!(protocols.contains(&Protocol::PumpSwap));
    }
} 