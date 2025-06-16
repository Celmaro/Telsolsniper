use crate::common::raydium::logs_data::{discriminators, SwapBaseInputEvent, V4SwapEvent};
use crate::common::raydium::SwapBaseOutputEvent;
use base64::engine::general_purpose;
use base64::Engine;
use borsh::BorshDeserialize;

pub const PROGRAM_DATA: &str = "Program data: ";
pub const PROGRAM_LOG_PREFIX: &str = "Program log: ray_log: ";

/// Raydium事件枚举
#[derive(Debug)]
pub enum RaydiumEvent {
    V4Swap(V4SwapEvent),
    SwapBaseInput(SwapBaseInputEvent),
    SwapBaseOutput(SwapBaseOutputEvent),
    Error(String),
}
