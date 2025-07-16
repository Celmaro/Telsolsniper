use anyhow::{anyhow, Result};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

use crate::streaming::event_parser::protocols::{
    bonk::parser::BONK_PROGRAM_ID, pumpfun::parser::PUMPFUN_PROGRAM_ID,
    pumpswap::parser::PUMPSWAP_PROGRAM_ID, raydium_cpmm::parser::RAYDIUM_CPMM_PROGRAM_ID,
    BonkEventParser, RaydiumCpmmEventParser,
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
    Bonk,
    RaydiumCpmm,
}

impl Protocol {
    pub fn get_program_id(&self) -> Vec<Pubkey> {
        match self {
            Protocol::PumpSwap => vec![PUMPSWAP_PROGRAM_ID],
            Protocol::PumpFun => vec![PUMPFUN_PROGRAM_ID],
            Protocol::Bonk => vec![BONK_PROGRAM_ID],
            Protocol::RaydiumCpmm => vec![RAYDIUM_CPMM_PROGRAM_ID],
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::PumpSwap => write!(f, "PumpSwap"),
            Protocol::PumpFun => write!(f, "PumpFun"),
            Protocol::Bonk => write!(f, "Bonk"),
            Protocol::RaydiumCpmm => write!(f, "RaydiumCpmm"),
        }
    }
}

impl std::str::FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pumpswap" => Ok(Protocol::PumpSwap),
            "pumpfun" => Ok(Protocol::PumpFun),
            "bonk" => Ok(Protocol::Bonk),
            "raydiumcpmm" => Ok(Protocol::RaydiumCpmm),
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
            Protocol::Bonk => Arc::new(BonkEventParser::new()),
            Protocol::RaydiumCpmm => Arc::new(RaydiumCpmmEventParser::new()),
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
