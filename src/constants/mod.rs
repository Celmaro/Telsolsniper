use solana_sdk::{pubkey, pubkey::Pubkey};

pub mod decimals;
pub mod swqos;
pub mod trade;
pub mod trade_platform;

pub const SYSTEM_PROGRAM: Pubkey = solana_sdk::system_program::ID;
pub const SYSTEM_PROGRAM_META: once_cell::sync::Lazy<solana_sdk::instruction::AccountMeta> =
    once_cell::sync::Lazy::new(|| {
        solana_sdk::instruction::AccountMeta::new_readonly(SYSTEM_PROGRAM, false)
    });

pub const TOKEN_PROGRAM: Pubkey = spl_token::ID;
pub const TOKEN_PROGRAM_META: once_cell::sync::Lazy<solana_sdk::instruction::AccountMeta> =
    once_cell::sync::Lazy::new(|| {
        solana_sdk::instruction::AccountMeta::new_readonly(TOKEN_PROGRAM, false)
    });

pub const TOKEN_PROGRAM_2022: Pubkey = spl_token_2022::ID;

pub const WSOL_TOKEN_ACCOUNT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
pub const WSOL_TOKEN_ACCOUNT_META: once_cell::sync::Lazy<solana_sdk::instruction::AccountMeta> =
    once_cell::sync::Lazy::new(|| {
        solana_sdk::instruction::AccountMeta::new_readonly(WSOL_TOKEN_ACCOUNT, false)
    });
