use base64::engine::general_purpose;
use base64::Engine;
use std::time::{SystemTime, UNIX_EPOCH};

/// 获取当前时间戳
pub fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

/// 从base64字符串解码数据
pub fn decode_base64(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
    general_purpose::STANDARD.decode(data)
}

/// 将数据编码为base64字符串
pub fn encode_base64(data: &[u8]) -> String {
    general_purpose::STANDARD.encode(data)
}

/// 从字节数组中提取鉴别器和剩余数据
pub fn extract_discriminator(length: usize, data: &[u8]) -> Option<(&[u8], &[u8])> {
    if data.len() < length {
        return None;
    }
    Some((&data[..length], &data[length..]))
}

/// 检查鉴别器是否匹配
pub fn discriminator_matches(data: &str, expected: &str) -> bool {
    if data.len() < expected.len() {
        return false;
    }
    &data[..expected.len()] == expected
}

/// 从日志中提取程序数据
pub fn extract_program_data(log: &str) -> Option<&str> {
    const PROGRAM_DATA_PREFIX: &str = "Program data: ";
    log.strip_prefix(PROGRAM_DATA_PREFIX)
}

/// 从日志中提取程序日志
pub fn extract_program_log<'a>(log: &'a str, prefix: &str) -> Option<&'a str> {
    log.strip_prefix(prefix)
}

/// 安全地从字节数组中读取u64
pub fn read_u64_le(data: &[u8], offset: usize) -> Option<u64> {
    if data.len() < offset + 8 {
        return None;
    }
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// 安全地从字节数组中读取u32
pub fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    if data.len() < offset + 4 {
        return None;
    }
    let bytes: [u8; 4] = data[offset..offset + 4].try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

/// 安全地从字节数组中读取u16
pub fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    if data.len() < offset + 2 {
        return None;
    }
    let bytes: [u8; 2] = data[offset..offset + 2].try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

/// 安全地从字节数组中读取u8
pub fn read_u8(data: &[u8], offset: usize) -> Option<u8> {
    data.get(offset).copied()
}

/// 验证账户索引的有效性
pub fn validate_account_indices(indices: &[u8], account_count: usize) -> bool {
    indices.iter().all(|&idx| (idx as usize) < account_count)
}

/// 格式化公钥为短字符串
pub fn format_pubkey_short(pubkey: &solana_sdk::pubkey::Pubkey) -> String {
    let s = pubkey.to_string();
    if s.len() <= 8 {
        s
    } else {
        format!("{}...{}", &s[..4], &s[s.len() - 4..])
    }
}
