use anyhow::{anyhow, Result};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

use crate::event_parser::protocols::{
    pumpfun::parser::PUMPFUN_PROGRAM_ID, pumpswap::parser::PUMPSWAP_PROGRAM_ID, raydium_launchpad::parser::RAYDIUM_LAUNCHPAD_PROGRAM_ID, RaydiumLaunchpadEventParser,
};

use super::{
    core::traits::EventParser,
    protocols::{pumpfun::PumpFunEventParser, pumpswap::PumpSwapEventParser},
};

/// 支持的协议
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Protocol {
    PumpSwap,
    PumpFun,
    RaydiumLaunchpad,
}

impl Protocol {
    pub fn get_program_id(&self) -> Vec<Pubkey> {
        match self {
            Protocol::PumpSwap => vec![PUMPSWAP_PROGRAM_ID],
            Protocol::PumpFun => vec![PUMPFUN_PROGRAM_ID],
            Protocol::RaydiumLaunchpad => vec![RAYDIUM_LAUNCHPAD_PROGRAM_ID],
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::PumpSwap => write!(f, "PumpSwap"),
            Protocol::PumpFun => write!(f, "PumpFun"),
            Protocol::RaydiumLaunchpad => write!(f, "RaydiumLaunchpad"),
        }
    }
}

impl std::str::FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pumpswap" => Ok(Protocol::PumpSwap),
            "pumpfun" => Ok(Protocol::PumpFun),
            "raydiumlaunchpad" => Ok(Protocol::RaydiumLaunchpad),
            _ => Err(anyhow!("Unsupported protocol: {}", s)),
        }
    }
}

/// 事件解析器工厂 - 用于创建不同协议的事件解析器
pub struct EventParserFactory;

impl EventParserFactory {
    /// 创建指定协议的事件解析器
    pub fn create_parser(protocol: Protocol) -> Arc<dyn EventParser> {
        match protocol {
            Protocol::PumpSwap => Arc::new(PumpSwapEventParser::new()),
            Protocol::PumpFun => Arc::new(PumpFunEventParser::new()),
            Protocol::RaydiumLaunchpad => Arc::new(RaydiumLaunchpadEventParser::new()),
        }
    }

    /// 创建所有协议的事件解析器
    pub fn create_all_parsers() -> Vec<Arc<dyn EventParser>> {
        Self::supported_protocols()
            .into_iter()
            .map(Self::create_parser)
            .collect()
    }

    /// 获取所有支持的协议
    pub fn supported_protocols() -> Vec<Protocol> {
        vec![Protocol::PumpSwap]
    }

    /// 检查协议是否支持
    pub fn is_supported(protocol: &Protocol) -> bool {
        Self::supported_protocols().contains(protocol)
    }
}
