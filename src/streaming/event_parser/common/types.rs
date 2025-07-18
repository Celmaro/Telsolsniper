use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub enum ProtocolType {
    #[default]
    PumpSwap,
    PumpFun,
    Bonk,
    RaydiumCpmm,
}

/// 事件类型枚举
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub enum EventType {
    // PumpSwap 事件
    #[default]
    PumpSwapBuy,
    PumpSwapSell,
    PumpSwapCreatePool,
    PumpSwapDeposit,
    PumpSwapWithdraw,

    // PumpFun 事件
    PumpFunCreateToken,
    PumpFunBuy,
    PumpFunSell,

    // Bonk 事件
    BonkBuyExactIn,
    BonkBuyExactOut,
    BonkSellExactIn,
    BonkSellExactOut,
    BonkInitialize,

    // Raydium CPMM 事件
    RaydiumCpmmSwapBaseInput,
    RaydiumCpmmSwapBaseOutput,

    // 通用事件
    Unknown,
}

impl EventType {
    pub fn to_string(&self) -> String {
        match self {
            EventType::PumpSwapBuy => "PumpSwapBuy".to_string(),
            EventType::PumpSwapSell => "PumpSwapSell".to_string(),
            EventType::PumpSwapCreatePool => "PumpSwapCreatePool".to_string(),
            EventType::PumpSwapDeposit => "PumpSwapDeposit".to_string(),
            EventType::PumpSwapWithdraw => "PumpSwapWithdraw".to_string(),
            EventType::PumpFunCreateToken => "PumpFunCreateToken".to_string(),
            EventType::PumpFunBuy => "PumpFunBuy".to_string(),
            EventType::PumpFunSell => "PumpFunSell".to_string(),
            EventType::BonkBuyExactIn => "BonkBuyExactIn".to_string(),
            EventType::BonkBuyExactOut => "BonkBuyExactOut".to_string(),
            EventType::BonkSellExactIn => "BonkSellExactIn".to_string(),
            EventType::BonkSellExactOut => "BonkSellExactOut".to_string(),
            EventType::BonkInitialize => "BonkInitialize".to_string(),
            EventType::RaydiumCpmmSwapBaseInput => "RaydiumCpmmSwapBaseInput".to_string(),
            EventType::RaydiumCpmmSwapBaseOutput => "RaydiumCpmmSwapBaseOutput".to_string(),
            EventType::Unknown => "Unknown".to_string(),
        }
    }
}

/// 解析结果
#[derive(Debug, Clone)]
pub struct ParseResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ParseResult<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn is_failure(&self) -> bool {
        !self.success
    }
}

/// 协议信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolInfo {
    pub name: String,
    pub program_ids: Vec<Pubkey>,
}

impl ProtocolInfo {
    pub fn new(name: String, program_ids: Vec<Pubkey>) -> Self {
        Self { name, program_ids }
    }

    pub fn supports_program(&self, program_id: &Pubkey) -> bool {
        self.program_ids.contains(program_id)
    }
}

/// 事件元数据
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct EventMetadata {
    pub id: String,
    pub signature: String,
    pub slot: u64,
    pub program_received_time_ms: i64,
    pub protocol: ProtocolType,
    pub event_type: EventType,
    pub program_id: Pubkey,
}

impl EventMetadata {
    pub fn new(
        id: String,
        signature: String,
        slot: u64,
        protocol: ProtocolType,
        event_type: EventType,
        program_id: Pubkey,
    ) -> Self {
        Self {
            id,
            signature,
            slot,
            program_received_time_ms: chrono::Utc::now().timestamp_millis(),
            protocol,
            event_type,
            program_id,
        }
    }
    pub fn set_id(&mut self, id: String) {
        let _id = format!("{}-{}-{}", self.signature, self.event_type.to_string(), id);
        // 对传入的 id 进行哈希处理
        let mut hasher = DefaultHasher::new();
        _id.hash(&mut hasher);
        let hash_value = hasher.finish();
        self.id = format!("{:x}", hash_value);
    }
}
