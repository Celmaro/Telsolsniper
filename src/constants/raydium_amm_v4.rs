//! Constants used by the crate.
//!
//! This module contains various constants used throughout the crate, including:
//!
//! - Seeds for deriving Program Derived Addresses (PDAs)
//! - Program account addresses and public keys
//!
//! The constants are organized into submodules for better organization:
//!
//! - `seeds`: Contains seed values used for PDA derivation
//! - `accounts`: Contains important program account addresses

/// Constants used as seeds for deriving PDAs (Program Derived Addresses)
pub mod seeds {
    pub const POOL_SEED: &[u8] = b"pool";
}

/// Constants related to program accounts and authorities
pub mod accounts {
    use solana_sdk::{pubkey, pubkey::Pubkey};
    pub const AUTHORITY: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
    pub const TOKEN_PROGRAM: Pubkey = spl_token::ID;
    pub const WSOL_TOKEN_ACCOUNT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
    pub const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

    pub const TRADE_FEE_NUMERATOR: u64 = 25;
    pub const TRADE_FEE_DENOMINATOR: u64 = 10000;
    pub const SWAP_FEE_NUMERATOR: u64 = 25;
    pub const SWAP_FEE_DENOMINATOR: u64 = 10000;
}

pub const SWAP_BASE_IN_DISCRIMINATOR: &[u8] = &[9];
pub const SWAP_BASE_OUT_DISCRIMINATOR: &[u8] = &[11];
