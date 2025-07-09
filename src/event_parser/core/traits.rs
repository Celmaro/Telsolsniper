use anyhow::Result;
use solana_sdk::{
    instruction::CompiledInstruction, pubkey::Pubkey, transaction::VersionedTransaction,
};
use solana_transaction_status::{
    EncodedTransactionWithStatusMeta, UiCompiledInstruction, UiInstruction,
};
use std::collections::HashMap;
use std::fmt::Debug;

use crate::{
    error,
    event_parser::{common::{utils::*, EventMetadata, EventType, ProtocolType}, protocols::{pumpfun::{PumpFunCreateTokenEvent, PumpFunTradeEvent}, bonk::{BonkPoolCreateEvent, BonkTradeEvent}}},
};

/// 统一事件接口 - 所有协议的事件都需要实现此trait
pub trait UnifiedEvent: Debug + Send + Sync {
    /// 获取事件ID
    fn id(&self) -> &str;

    /// 获取事件类型
    fn event_type(&self) -> EventType;

    /// 获取交易签名
    fn signature(&self) -> &str;

    /// 获取槽位号
    fn slot(&self) -> u64;

    /// 获取程序接收的时间戳(毫秒)
    fn program_received_time_ms(&self) -> i64;

    /// 将事件转换为Any以便向下转型
    fn as_any(&self) -> &dyn std::any::Any;

    /// 将事件转换为可变Any以便向下转型
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// 克隆事件
    fn clone_boxed(&self) -> Box<dyn UnifiedEvent>;

    /// 合并事件（可选实现）
    fn merge(&mut self, _other: Box<dyn UnifiedEvent>) {
        // 默认实现：不进行任何合并操作
    }
}

/// 事件解析器trait - 定义了事件解析的核心方法
#[async_trait::async_trait]
pub trait EventParser: Send + Sync {
    /// 从内联指令中解析事件数据
    fn parse_events_from_inner_instruction(
        &self,
        instruction: &UiCompiledInstruction,
        signature: &str,
        slot: u64,
    ) -> Vec<Box<dyn UnifiedEvent>>;

    /// 从指令中解析事件数据
    fn parse_events_from_instruction(
        &self,
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
        signature: &str,
        slot: u64,
    ) -> Vec<Box<dyn UnifiedEvent>>;

    /// 从VersionedTransaction中解析指令事件的通用方法
    async fn parse_instruction_events_from_versioned_transaction(
        &self,
        versioned_tx: &VersionedTransaction,
        signature: &str,
        slot: Option<u64>,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let mut instruction_events = Vec::new();
        // 获取交易的指令和账户
        let compiled_instructions = versioned_tx.message.instructions();
        let accounts = versioned_tx.message.static_account_keys();
        // 检查交易中是否包含程序
        let has_program = accounts.iter().any(|account| self.should_handle(account));
        if has_program {
            // 解析每个指令
            for instruction in compiled_instructions {
                if let Some(program_id) = accounts.get(instruction.program_id_index as usize) {
                    if self.should_handle(program_id) {
                        // 验证账户索引的有效性
                        let all_accounts_valid = instruction
                            .accounts
                            .iter()
                            .all(|&acc_idx| (acc_idx as usize) < accounts.len());

                        if all_accounts_valid {
                            if let Ok(events) = self
                                .parse_instruction(instruction, accounts, signature, slot)
                                .await
                            {
                                instruction_events.extend(events);
                            }
                        }
                    }
                }
            }
        }
        Ok(instruction_events)
    }

    async fn parse_versioned_transaction(
        &self,
        versioned_tx: &VersionedTransaction,
        signature: &str,
        slot: Option<u64>,
        bot_wallet: Option<Pubkey>,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let events = self.parse_instruction_events_from_versioned_transaction(versioned_tx, signature, slot)
            .await.unwrap_or_else(|_e| vec![]);
        Ok(self.process_events(events, bot_wallet))
    }

    async fn parse_transaction(
        &self,
        tx: EncodedTransactionWithStatusMeta,
        signature: &str,
        slot: Option<u64>,
        bot_wallet: Option<Pubkey>,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let transaction = tx.transaction;
        let mut instruction_events = Vec::new();

        // 解析指令事件
        if let Some(versioned_tx) = transaction.decode() {
            instruction_events = self
                .parse_instruction_events_from_versioned_transaction(&versioned_tx, signature, slot)
                .await
                .unwrap_or_else(|_e| vec![]);
        }

        // 解析内联指令事件
        let mut inner_instruction_events = Vec::new();
        // 检查交易元数据
        let meta = tx
            .meta
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing transaction metadata"))?;
        // 检查交易是否成功
        if meta.err.is_none() {
            let inner_instructions = meta.inner_instructions.as_ref().unwrap();
            for inner_instruction in inner_instructions {
                for instruction in &inner_instruction.instructions {
                    match instruction {
                        UiInstruction::Compiled(compiled) => {
                            if let Ok(events) = self
                                .parse_inner_instruction(compiled, signature, slot)
                                .await
                            {
                                inner_instruction_events.extend(events);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if instruction_events.len() > 0 && inner_instruction_events.len() > 0 {
            for instruction_event in &mut instruction_events {
                for inner_instruction_event in &inner_instruction_events {
                    if instruction_event.id() == inner_instruction_event.id()
                        && instruction_event.event_type() == inner_instruction_event.event_type()
                    {
                        instruction_event.merge(inner_instruction_event.clone_boxed());
                        break;
                    }
                }
            }
        }
        Ok(self.process_events(instruction_events, bot_wallet))
    }

    fn process_events(&self, mut events: Vec<Box<dyn UnifiedEvent>>, bot_wallet: Option<Pubkey>) -> Vec<Box<dyn UnifiedEvent>> {
        let mut dev_address = None;
        let mut bonk_dev_address = None;
        for event in &mut events {
            if let Some(token_info) = event.as_any().downcast_ref::<PumpFunCreateTokenEvent>() {
                dev_address = Some(token_info.user);
            } else if let Some(trade_info) =
                event.as_any_mut().downcast_mut::<PumpFunTradeEvent>()
            {
                if Some(trade_info.user) == dev_address {
                    trade_info.is_dev_create_token_trade = true;
                } else if Some(trade_info.user) == bot_wallet {
                    trade_info.is_bot = true;
                } else {
                    trade_info.is_dev_create_token_trade = false;
                }
            }
            if let Some(pool_info) = event
                .as_any()
                .downcast_ref::<BonkPoolCreateEvent>()
            {
                bonk_dev_address = Some(pool_info.creator);
            } else if let Some(trade_info) = event
                .as_any_mut()
                .downcast_mut::<BonkTradeEvent>()
            {
                if Some(trade_info.payer) == bonk_dev_address {
                    trade_info.is_dev_create_token_trade = true;
                } else if Some(trade_info.payer) == bot_wallet {
                    trade_info.is_bot = true;
                } else {
                    trade_info.is_dev_create_token_trade = false;
                }
            }
        }
        events
    }

    async fn parse_inner_instruction(
        &self,
        instruction: &UiCompiledInstruction,
        signature: &str,
        slot: Option<u64>,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let slot = slot.unwrap_or(0);
        let events = self.parse_events_from_inner_instruction(instruction, signature, slot);
        Ok(events)
    }

    async fn parse_instruction(
        &self,
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
        signature: &str,
        slot: Option<u64>,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let slot = slot.unwrap_or(0);
        let events = self.parse_events_from_instruction(instruction, accounts, signature, slot);
        Ok(events)
    }

    /// 检查是否应该处理此程序ID
    fn should_handle(&self, program_id: &Pubkey) -> bool;

    /// 获取支持的程序ID列表
    fn supported_program_ids(&self) -> Vec<Pubkey>;
}

// 为Box<dyn UnifiedEvent>实现Clone
impl Clone for Box<dyn UnifiedEvent> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// 通用事件解析器配置
#[derive(Debug, Clone)]
pub struct GenericEventParseConfig {
    pub inner_instruction_discriminator: &'static str,
    pub instruction_discriminator: &'static [u8],
    pub event_type: EventType,
    pub inner_instruction_parser: InnerInstructionEventParser,
    pub instruction_parser: InstructionEventParser,
}

/// 内联指令事件解析器
pub type InnerInstructionEventParser =
    fn(data: &[u8], metadata: EventMetadata) -> Option<Box<dyn UnifiedEvent>>;

/// 指令事件解析器
pub type InstructionEventParser =
    fn(data: &[u8], accounts: &[Pubkey], metadata: EventMetadata) -> Option<Box<dyn UnifiedEvent>>;

/// 通用事件解析器基类
pub struct GenericEventParser {
    program_id: Pubkey,
    protocol_type: ProtocolType,
    inner_instruction_configs: HashMap<&'static str, Vec<GenericEventParseConfig>>,
    instruction_configs: HashMap<Vec<u8>, Vec<GenericEventParseConfig>>,
}

impl GenericEventParser {
    /// 创建新的通用事件解析器
    pub fn new(
        program_id: Pubkey,
        protocol_type: ProtocolType,
        configs: Vec<GenericEventParseConfig>,
    ) -> Self {
        let mut inner_instruction_configs = HashMap::new();
        let mut instruction_configs = HashMap::new();

        for config in configs {
            inner_instruction_configs
                .entry(config.inner_instruction_discriminator)
                .or_insert(vec![])
                .push(config.clone());
            instruction_configs
                .entry(config.instruction_discriminator.to_vec())
                .or_insert(vec![])
                .push(config);
        }

        Self {
            program_id,
            protocol_type,
            inner_instruction_configs,
            instruction_configs,
        }
    }

    /// 通用的内联指令解析方法
    fn parse_inner_instruction_event(
        &self,
        config: &GenericEventParseConfig,
        data: &[u8],
        signature: &str,
        slot: u64,
    ) -> Option<Box<dyn UnifiedEvent>> {
        let metadata = EventMetadata::new(
            signature.to_string(),
            signature.to_string(),
            slot,
            self.protocol_type.clone(),
            config.event_type.clone(),
            self.program_id,
        );
        (config.inner_instruction_parser)(data, metadata)
    }

    /// 通用的指令解析方法
    fn parse_instruction_event(
        &self,
        config: &GenericEventParseConfig,
        data: &[u8],
        account_pubkeys: &[Pubkey],
        signature: &str,
        slot: u64,
    ) -> Option<Box<dyn UnifiedEvent>> {
        let metadata = EventMetadata::new(
            signature.to_string(),
            signature.to_string(),
            slot,
            self.protocol_type.clone(),
            config.event_type.clone(),
            self.program_id,
        );
        (config.instruction_parser)(data, account_pubkeys, metadata)
    }
}

#[async_trait::async_trait]
impl EventParser for GenericEventParser {
    /// 从内联指令中解析事件数据
    fn parse_events_from_inner_instruction(
        &self,
        inner_instruction: &UiCompiledInstruction,
        signature: &str,
        slot: u64,
    ) -> Vec<Box<dyn UnifiedEvent>> {
        let inner_instruction_data = inner_instruction.data.clone();
        let inner_instruction_data_decoded =
            bs58::decode(inner_instruction_data).into_vec().unwrap();
        if inner_instruction_data_decoded.len() < 16 {
            return Vec::new();
        }
        let inner_instruction_data_decoded_str =
            format!("0x{}", hex::encode(&inner_instruction_data_decoded));
        let data = &inner_instruction_data_decoded[16..];
        let mut events = Vec::new();
        for (disc, configs) in &self.inner_instruction_configs {
            if discriminator_matches(&inner_instruction_data_decoded_str, disc) {
                for config in configs {
                    if let Some(event) =
                        self.parse_inner_instruction_event(config, data, signature, slot)
                    {
                        events.push(event);
                    }
                }
            }
        }
        events
    }

    /// 从指令中解析事件
    fn parse_events_from_instruction(
        &self,
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
        signature: &str,
        slot: u64,
    ) -> Vec<Box<dyn UnifiedEvent>> {
        let mut events = Vec::new();
        for (disc, configs) in &self.instruction_configs {
            if instruction.data.len() < disc.len() {
                continue;
            }
            let discriminator = &instruction.data[..disc.len()];
            let data = &instruction.data[disc.len()..];
            if discriminator == disc {
                // 验证账户索引
                if !validate_account_indices(&instruction.accounts, accounts.len()) {
                    continue;
                }

                let account_pubkeys: Vec<Pubkey> = instruction
                    .accounts
                    .iter()
                    .map(|&idx| accounts[idx as usize])
                    .collect();
                for config in configs {
                    if let Some(event) = self.parse_instruction_event(
                        config,
                        data,
                        &account_pubkeys,
                        signature,
                        slot,
                    ) {
                        events.push(event);
                    }
                }
            }
        }

        events
    }

    fn should_handle(&self, program_id: &Pubkey) -> bool {
        *program_id == self.program_id
    }

    fn supported_program_ids(&self) -> Vec<Pubkey> {
        vec![self.program_id]
    }
}
