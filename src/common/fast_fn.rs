use clru::CLruCache;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use spl_associated_token_account::{
    get_associated_token_address_with_program_id, ID as ASSOCIATED_TOKEN_PROGRAM_ID,
};
use std::num::NonZeroUsize;

const MAX_PDA_CACHE_SIZE: usize = 10000;
const MAX_ATA_CACHE_SIZE: usize = 10000;
const MAX_INSTRUCTION_CACHE_SIZE: usize = 10000;

// --------------------- Instruction Cache ---------------------

/// Instruction cache key for uniquely identifying instruction types and parameters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstructionCacheKey {
    /// Associated Token Account creation instruction
    CreateAssociatedTokenAccount {
        payer: Pubkey,
        owner: Pubkey,
        mint: Pubkey,
        token_program: Pubkey,
    },
    /// Close wSOL Account
    CloseWsolAccount { payer: Pubkey, wsol_token_account: Pubkey },
}

/// Global instruction cache for storing common instructions
static INSTRUCTION_CACHE: Lazy<RwLock<CLruCache<InstructionCacheKey, Instruction>>> =
    Lazy::new(|| {
        RwLock::new(CLruCache::new(NonZeroUsize::new(MAX_INSTRUCTION_CACHE_SIZE).unwrap()))
    });

/// Get cached instruction, compute and cache if not exists
pub fn get_cached_instruction<F>(cache_key: InstructionCacheKey, compute_fn: F) -> Instruction
where
    F: FnOnce() -> Instruction,
{
    // Try to get from cache (using read lock)
    {
        let cache = INSTRUCTION_CACHE.read();
        if let Some(cached_instruction) = cache.peek(&cache_key) {
            return cached_instruction.clone();
        }
    }

    // Cache miss, compute new instruction
    let instruction = compute_fn();

    // Store computation result in cache (using write lock)
    {
        let mut cache = INSTRUCTION_CACHE.write();
        cache.put(cache_key, instruction.clone());
    }

    instruction
}

// --------------------- Associated Token Account ---------------------

pub fn create_associated_token_account_idempotent_fast(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    // Create cache key
    let cache_key = InstructionCacheKey::CreateAssociatedTokenAccount {
        payer: *payer,
        owner: *owner,
        mint: *mint,
        token_program: *token_program,
    };

    // Use cache to get instruction
    get_cached_instruction(cache_key, || {
        // Get Associated Token Address using cache
        let associated_token_address =
            get_associated_token_address_with_program_id_fast(owner, mint, token_program);

        // Create Associated Token Account instruction
        // Reference implementation of spl_associated_token_account::instruction::create_associated_token_account
        Instruction {
            program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(*payer, true), // Payer (signer, writable)
                AccountMeta::new(associated_token_address, false), // ATA address (writable, non-signer)
                AccountMeta::new_readonly(*owner, false), // Token account owner (readonly, non-signer)
                AccountMeta::new_readonly(*mint, false), // Token mint address (readonly, non-signer)
                crate::constants::SYSTEM_PROGRAM_META,
                AccountMeta::new_readonly(*token_program, false), // Token program (readonly, non-signer)
            ],
            data: vec![1],
        }
    })
}

// --------------------- PDA ---------------------

/// PDA cache key for uniquely identifying PDA computation input parameters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PdaCacheKey {
    PumpFunUserVolume(Pubkey),
    PumpFunBondingCurve(Pubkey),
    PumpFunCreatorVault(Pubkey),
    BonkPool(Pubkey, Pubkey),
    BonkVault(Pubkey, Pubkey),
    PumpSwapUserVolume(Pubkey),
}

/// Global PDA cache for storing computation results
static PDA_CACHE: Lazy<RwLock<CLruCache<PdaCacheKey, Pubkey>>> =
    Lazy::new(|| RwLock::new(CLruCache::new(NonZeroUsize::new(MAX_PDA_CACHE_SIZE).unwrap())));

/// Get cached PDA, compute and cache if not exists
pub fn get_cached_pda<F>(cache_key: PdaCacheKey, compute_fn: F) -> Option<Pubkey>
where
    F: FnOnce() -> Option<Pubkey>,
{
    // Try to get from cache (using read lock)
    {
        let cache = PDA_CACHE.read();
        if let Some(cached_pda) = cache.peek(&cache_key) {
            return Some(*cached_pda);
        }
    }

    // Cache miss, compute new PDA
    let pda_result = compute_fn();

    // If computation succeeds, store result in cache (using write lock)
    if let Some(pda) = pda_result {
        let mut cache = PDA_CACHE.write();
        cache.put(cache_key, pda);
    }

    pda_result
}

// --------------------- ATA ---------------------

/// ATA cache key for Associated Token Address caching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AtaCacheKey {
    wallet_address: Pubkey,
    token_mint_address: Pubkey,
    token_program_id: Pubkey,
}

/// Global ATA cache for storing Associated Token Address computation results
static ATA_CACHE: Lazy<RwLock<CLruCache<AtaCacheKey, Pubkey>>> =
    Lazy::new(|| RwLock::new(CLruCache::new(NonZeroUsize::new(MAX_ATA_CACHE_SIZE).unwrap())));

/// Get cached Associated Token Address, compute and cache if not exists
pub fn get_associated_token_address_with_program_id_fast(
    wallet_address: &Pubkey,
    token_mint_address: &Pubkey,
    token_program_id: &Pubkey,
) -> Pubkey {
    let cache_key = AtaCacheKey {
        wallet_address: *wallet_address,
        token_mint_address: *token_mint_address,
        token_program_id: *token_program_id,
    };

    // Try to get from cache (using read lock)
    {
        let cache = ATA_CACHE.read();
        if let Some(cached_ata) = cache.peek(&cache_key) {
            return *cached_ata;
        }
    }

    // Cache miss, compute new ATA
    let ata = get_associated_token_address_with_program_id(
        wallet_address,
        token_mint_address,
        token_program_id,
    );

    // Store computation result in cache (using write lock)
    {
        let mut cache = ATA_CACHE.write();
        cache.put(cache_key, ata);
    }

    ata
}

// --------------------- Initialize Accounts ---------------------

pub fn fast_init(payer: &Pubkey) {
    // Get PumpFun user volume accumulator PDA
    crate::instruction::utils::pumpfun::get_user_volume_accumulator_pda(payer);
    // Get PumpSwap user volume accumulator PDA
    crate::instruction::utils::pumpswap::get_user_volume_accumulator_pda(payer);
    // Get wSOL ATA address
    let wsol_token_account = get_associated_token_address_with_program_id_fast(
        payer,
        &crate::constants::WSOL_TOKEN_ACCOUNT,
        &crate::constants::TOKEN_PROGRAM,
    );
    // Get Close wSOL Account instruction
    get_cached_instruction(
        crate::common::fast_fn::InstructionCacheKey::CloseWsolAccount {
            payer: *payer,
            wsol_token_account,
        },
        || {
            spl_token::instruction::close_account(
                &crate::constants::TOKEN_PROGRAM,
                &wsol_token_account,
                &payer,
                &payer,
                &[],
            )
            .unwrap()
        },
    );
}
