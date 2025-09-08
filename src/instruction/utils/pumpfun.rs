use crate::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::PumpFunTradeEvent;
use crate::{
    common::{bonding_curve::BondingCurveAccount, global::GlobalAccount, SolanaRpcClient},
    constants::{self, trade::trade::DEFAULT_SLIPPAGE},
};
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

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

    pub const FEE_CONFIG_SEED: &[u8] = b"fee_config";
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

    pub const CREATOR_FEE: u64 = 30;

    pub const SCALE: u64 = 1_000_000; // 10^6 for token decimals

    pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000; // 10^9 for solana lamports

    pub const COMPLETION_LAMPORTS: u64 = 85 * LAMPORTS_PER_SOL; // ~ 85 SOL

    /// Public key for the fee recipient
    pub const FEE_RECIPIENT: Pubkey = pubkey!("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV");
    pub const FEE_RECIPIENT_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: FEE_RECIPIENT,
            is_signer: false,
            is_writable: true,
        };

    /// Public key for the global PDA
    pub const GLOBAL_ACCOUNT: Pubkey = pubkey!("4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf");
    pub const GLOBAL_ACCOUNT_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: GLOBAL_ACCOUNT,
            is_signer: false,
            is_writable: false,
        };

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

    /// Associated Token Program ID
    pub const ASSOCIATED_TOKEN_PROGRAM: Pubkey =
        pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

    pub const AMM_PROGRAM: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

    pub const FEE_PROGRAM: Pubkey = pubkey!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ");

    pub const GLOBAL_VOLUME_ACCUMULATOR: Pubkey =
        pubkey!("Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y"); // get_global_volume_accumulator_pda().unwrap();

    pub const FEE_CONFIG: Pubkey = pubkey!("8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt"); // get_fee_config_pda().unwrap();

    // META
    pub const PUMPFUN_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: PUMPFUN,
            is_signer: false,
            is_writable: false,
        };

    pub const EVENT_AUTHORITY_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: EVENT_AUTHORITY,
            is_signer: false,
            is_writable: false,
        };

    pub const FEE_PROGRAM_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: FEE_PROGRAM,
            is_signer: false,
            is_writable: false,
        };

    pub const GLOBAL_VOLUME_ACCUMULATOR_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: GLOBAL_VOLUME_ACCUMULATOR,
            is_signer: false,
            is_writable: true,
        };

    pub const FEE_CONFIG_META: solana_sdk::instruction::AccountMeta =
        solana_sdk::instruction::AccountMeta {
            pubkey: FEE_CONFIG,
            is_signer: false,
            is_writable: false,
        };
}

pub struct Symbol;

impl Symbol {
    pub const SOLANA: &'static str = "solana";
}

lazy_static::lazy_static! {
    static ref ACCOUNT_CACHE: RwLock<HashMap<Pubkey, Arc<GlobalAccount>>> = RwLock::new(HashMap::new());
}

#[inline]
pub fn get_global_pda() -> Pubkey {
    static GLOBAL_PDA: once_cell::sync::Lazy<Pubkey> = once_cell::sync::Lazy::new(|| {
        Pubkey::find_program_address(&[seeds::GLOBAL_SEED], &accounts::PUMPFUN).0
    });
    *GLOBAL_PDA
}

#[inline]
pub fn get_mint_authority_pda() -> Pubkey {
    static MINT_AUTHORITY_PDA: once_cell::sync::Lazy<Pubkey> = once_cell::sync::Lazy::new(|| {
        Pubkey::find_program_address(&[seeds::MINT_AUTHORITY_SEED], &accounts::PUMPFUN).0
    });
    *MINT_AUTHORITY_PDA
}

#[inline]
pub fn get_bonding_curve_pda(mint: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[seeds::BONDING_CURVE_SEED, mint.as_ref()];
    let program_id: &Pubkey = &accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_creator(creator_vault_pda: &Pubkey) -> Pubkey {
    if creator_vault_pda.eq(&Pubkey::default()) {
        Pubkey::default()
    } else {
        // Fast check against cached default creator vault
        static DEFAULT_CREATOR_VAULT: std::sync::LazyLock<Option<Pubkey>> =
            std::sync::LazyLock::new(|| get_creator_vault_pda(&Pubkey::default()));
        if creator_vault_pda.eq(&DEFAULT_CREATOR_VAULT.unwrap()) {
            Pubkey::default()
        } else {
            *creator_vault_pda
        }
    }
}

#[inline]
pub fn get_creator_vault_pda(creator: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[seeds::CREATOR_VAULT_SEED, creator.as_ref()];
    let program_id: &Pubkey = &accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_user_volume_accumulator_pda(user: &Pubkey) -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[seeds::USER_VOLUME_ACCUMULATOR_SEED, user.as_ref()];
    let program_id: &Pubkey = &accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_global_volume_accumulator_pda() -> Option<Pubkey> {
    let seeds: &[&[u8]; 1] = &[seeds::GLOBAL_VOLUME_ACCUMULATOR_SEED];
    let program_id: &Pubkey = &accounts::PUMPFUN;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_fee_config_pda() -> Option<Pubkey> {
    let seeds: &[&[u8]; 2] = &[seeds::FEE_CONFIG_SEED, accounts::PUMPFUN.as_ref()];
    let program_id: &Pubkey = &accounts::FEE_PROGRAM;
    let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
    pda.map(|pubkey| pubkey.0)
}

#[inline]
pub fn get_metadata_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[seeds::METADATA_SEED, accounts::MPL_TOKEN_METADATA.as_ref(), mint.as_ref()],
        &accounts::MPL_TOKEN_METADATA,
    )
    .0
}

#[inline]
pub async fn get_global_account(/*rpc: &SolanaRpcClient*/
) -> Result<Arc<GlobalAccount>, anyhow::Error> {
    let global_account = GlobalAccount::new();
    let global_account = Arc::new(global_account);
    Ok(global_account)
}

#[inline]
pub async fn get_initial_buy_price(
    global_account: &Arc<GlobalAccount>,
    amount_sol: u64,
) -> Result<u64, anyhow::Error> {
    let buy_amount = global_account.get_initial_buy_price(amount_sol);
    Ok(buy_amount)
}

#[inline]
pub async fn fetch_bonding_curve_account(
    rpc: &SolanaRpcClient,
    mint: &Pubkey,
) -> Result<(Arc<crate::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::types::BondingCurve>, Pubkey), anyhow::Error>{
    let bonding_curve_pda: Pubkey =
        get_bonding_curve_pda(mint).ok_or(anyhow!("Bonding curve not found"))?;

    let account = rpc.get_account(&bonding_curve_pda).await?;
    if account.data.is_empty() {
        return Err(anyhow!("Bonding curve not found"));
    }

    let bonding_curve = solana_sdk::borsh1::try_from_slice_unchecked::<crate::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::types::BondingCurve>(&account.data[8..])
        .map_err(|e| anyhow::anyhow!("Failed to deserialize bonding curve account: {}", e))?;

    Ok((Arc::new(bonding_curve), bonding_curve_pda))
}

#[inline]
pub async fn init_bonding_curve_account(
    mint: &Pubkey,
    dev_buy_token: u64,
    dev_sol_cost: u64,
    creator: Pubkey,
) -> Result<Arc<BondingCurveAccount>, anyhow::Error> {
    let bonding_curve =
        BondingCurveAccount::from_dev_trade(mint, dev_buy_token, dev_sol_cost, creator);
    let bonding_curve = Arc::new(bonding_curve);
    Ok(bonding_curve)
}

#[inline]
pub fn get_buy_amount_with_slippage(amount_sol: u64, slippage_basis_points: Option<u64>) -> u64 {
    let slippage = slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE);
    amount_sol + (amount_sol * slippage / 10000)
}

#[inline]
pub fn get_buy_price(amount: u64, trade_info: &PumpFunTradeEvent) -> u64 {
    if amount == 0 {
        return 0;
    }

    let n: u128 =
        (trade_info.virtual_sol_reserves as u128) * (trade_info.virtual_token_reserves as u128);
    let i: u128 = (trade_info.virtual_sol_reserves as u128) + (amount as u128);
    let r: u128 = n / i + 1;
    let s: u128 = (trade_info.virtual_token_reserves as u128) - r;
    let s_u64 = s as u64;

    s_u64.min(trade_info.real_token_reserves)
}
