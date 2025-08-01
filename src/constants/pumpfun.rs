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
    /// Seed for the global state PDA
    pub const GLOBAL_SEED: &[u8] = b"global";

    /// Seed for the mint authority PDA
    pub const MINT_AUTHORITY_SEED: &[u8] = b"mint-authority";

    /// Seed for bonding curve PDAs
    pub const BONDING_CURVE_SEED: &[u8] = b"bonding-curve";

    /// Seed for creator vault PDAs
    pub const CREATOR_VAULT_SEED: &[u8] = b"creator-vault";

    /// Seed for metadata PDAs
    pub const METADATA_SEED: &[u8] = b"metadata";

    /// Seed for user volume accumulator PDAs
    pub const USER_VOLUME_ACCUMULATOR_SEED: &[u8] = b"user_volume_accumulator";

    /// Seed for global volume accumulator PDAs
    pub const GLOBAL_VOLUME_ACCUMULATOR_SEED: &[u8] = b"global_volume_accumulator";
}

pub mod global_constants {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub const INITIAL_VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000;

    pub const INITIAL_VIRTUAL_SOL_RESERVES: u64 = 30_000_000_000;

    pub const INITIAL_REAL_TOKEN_RESERVES: u64 = 793_100_000_000_000;

    pub const TOKEN_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;

    pub const FEE_BASIS_POINTS: u64 = 95;

    pub const ENABLE_MIGRATE: bool = false;

    pub const POOL_MIGRATION_FEE: u64 = 15_000_001;

    pub const CREATOR_FEE: u64 = 5;

    pub const SCALE: u64 = 1_000_000; // 10^6 for token decimals

    pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000; // 10^9 for solana lamports

    pub const TOTAL_SUPPLY: u64 = 1_000_000_000 * SCALE; // 1 billion tokens

    pub const BONDING_CURVE_SUPPLY: u64 = 793_100_000 * SCALE; // total supply of bonding curve tokens

    pub const COMPLETION_LAMPORTS: u64 = 85 * LAMPORTS_PER_SOL; // ~ 85 SOL

    /// Public key for the fee recipient
    pub const FEE_RECIPIENT: Pubkey = pubkey!("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV");

    /// Public key for the global PDA
    pub const GLOBAL_ACCOUNT: Pubkey = pubkey!("4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf");

    /// Public key for the authority
    pub const AUTHORITY: Pubkey = pubkey!("FFWtrEQ4B4PKQoVuHYzZq8FabGkVatYzDpEVHsK5rrhF");

    /// Public key for the withdraw authority
    pub const WITHDRAW_AUTHORITY: Pubkey = pubkey!("39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg");

    pub const PUMPFUN_AMM_FEE_1: Pubkey = pubkey!("7VtfL8fvgNfhz17qKRMjzQEXgbdpnHHHQRh54R9jP2RJ"); // Pump.fun AMM: Protocol Fee 1
    pub const PUMPFUN_AMM_FEE_2: Pubkey = pubkey!("7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX"); // Pump.fun AMM: Protocol Fee 2
    pub const PUMPFUN_AMM_FEE_3: Pubkey = pubkey!("9rPYyANsfQZw3DnDmKE3YCQF5E8oD89UXoHn9JFEhJUz"); // Pump.fun AMM: Protocol Fee 3
    pub const PUMPFUN_AMM_FEE_4: Pubkey = pubkey!("AVmoTthdrX6tKt4nDjco2D775W2YK3sDhxPcMmzUAmTY"); // Pump.fun AMM: Protocol Fee 4
    pub const PUMPFUN_AMM_FEE_5: Pubkey = pubkey!("CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM"); // Pump.fun AMM: Protocol Fee 5
    pub const PUMPFUN_AMM_FEE_6: Pubkey = pubkey!("FWsW1xNtWscwNmKv6wVsU1iTzRN6wmmk3MjxRP5tT7hz"); // Pump.fun AMM: Protocol Fee 6
    pub const PUMPFUN_AMM_FEE_7: Pubkey = pubkey!("G5UZAVbAf46s7cKWoyKu8kYTip9DGTpbLZ2qa9Aq69dP");
    // Pump.fun AMM: Protocol Fee 7
}

/// Constants related to program accounts and authorities
pub mod accounts {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    /// Public key for the Pump.fun program
    pub const PUMPFUN: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

    /// Public key for the MPL Token Metadata program
    pub const MPL_TOKEN_METADATA: Pubkey = pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

    /// Authority for program events
    pub const EVENT_AUTHORITY: Pubkey = pubkey!("Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1");

    /// System Program ID
    pub const SYSTEM_PROGRAM: Pubkey = pubkey!("11111111111111111111111111111111");

    /// Token Program ID
    pub const TOKEN_PROGRAM: Pubkey = spl_token::ID;

    /// Associated Token Program ID
    pub const ASSOCIATED_TOKEN_PROGRAM: Pubkey =
        pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

    /// Rent Sysvar ID
    pub const RENT: Pubkey = pubkey!("SysvarRent111111111111111111111111111111111");

    pub const AMM_PROGRAM: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
}

pub struct Symbol;

impl Symbol {
    pub const SOLANA: &'static str = "solana";
}
