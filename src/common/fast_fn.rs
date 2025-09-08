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

/// 指令缓存键，用于唯一标识指令类型和参数
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstructionCacheKey {
    /// Associated Token Account 创建指令
    CreateAssociatedTokenAccount {
        payer: Pubkey,
        owner: Pubkey,
        mint: Pubkey,
        token_program: Pubkey,
    },
}

/// 全局指令缓存，用于存储常用指令
static INSTRUCTION_CACHE: Lazy<RwLock<CLruCache<InstructionCacheKey, Instruction>>> =
    Lazy::new(|| {
        RwLock::new(CLruCache::new(NonZeroUsize::new(MAX_INSTRUCTION_CACHE_SIZE).unwrap()))
    });

/// 获取缓存的指令，如果不存在则计算并缓存
pub fn get_cached_instruction<F>(cache_key: InstructionCacheKey, compute_fn: F) -> Instruction
where
    F: FnOnce() -> Instruction,
{
    // 尝试从缓存中获取（使用读锁）
    {
        let cache = INSTRUCTION_CACHE.read();
        if let Some(cached_instruction) = cache.peek(&cache_key) {
            return cached_instruction.clone();
        }
    }

    // 缓存未命中，计算新的指令
    let instruction = compute_fn();

    // 将计算结果存入缓存（使用写锁）
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
    // 创建缓存键
    let cache_key = InstructionCacheKey::CreateAssociatedTokenAccount {
        payer: *payer,
        owner: *owner,
        mint: *mint,
        token_program: *token_program,
    };

    // 使用缓存获取指令
    get_cached_instruction(cache_key, || {
        // 使用缓存的方式获取 Associated Token Address
        let associated_token_address =
            get_associated_token_address_with_program_id_fast(owner, mint, token_program);

        // 创建 Associated Token Account 指令
        // 参考 spl_associated_token_account::instruction::create_associated_token_account 的实现
        Instruction {
            program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(*payer, true), // 支付者（签名者，可写）
                AccountMeta::new(associated_token_address, false), // ATA地址（可写，非签名者）
                AccountMeta::new_readonly(*owner, false), // Token账户拥有者（只读，非签名者）
                AccountMeta::new_readonly(*mint, false), // Token mint地址（只读，非签名者）
                crate::constants::SYSTEM_PROGRAM_META,
                AccountMeta::new_readonly(*token_program, false), // Token程序（只读，非签名者）
            ],
            data: vec![1],
        }
    })
}

// --------------------- PDA ---------------------

/// PDA 缓存键，用于唯一标识 PDA 计算的输入参数
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PdaCacheKey {
    PumpFunUserVolume(Pubkey),
    PumpFunBondingCurve(Pubkey),
    PumpFunCreatorVault(Pubkey),
}

/// 全局 PDA 缓存，用于存储计算结果
static PDA_CACHE: Lazy<RwLock<CLruCache<PdaCacheKey, Pubkey>>> =
    Lazy::new(|| RwLock::new(CLruCache::new(NonZeroUsize::new(MAX_PDA_CACHE_SIZE).unwrap())));

/// 获取缓存的 PDA，如果不存在则计算并缓存
pub fn get_cached_pda<F>(cache_key: PdaCacheKey, compute_fn: F) -> Option<Pubkey>
where
    F: FnOnce() -> Option<Pubkey>,
{
    // 尝试从缓存中获取（使用读锁）
    {
        let cache = PDA_CACHE.read();
        if let Some(cached_pda) = cache.peek(&cache_key) {
            return Some(*cached_pda);
        }
    }

    // 缓存未命中，计算新的 PDA
    let pda_result = compute_fn();

    // 如果计算成功，将结果存入缓存（使用写锁）
    if let Some(pda) = pda_result {
        let mut cache = PDA_CACHE.write();
        cache.put(cache_key, pda);
    }

    pda_result
}

// --------------------- ATA ---------------------

/// ATA 缓存键，用于 Associated Token Address 缓存
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AtaCacheKey {
    wallet_address: Pubkey,
    token_mint_address: Pubkey,
    token_program_id: Pubkey,
}

/// 全局 ATA 缓存，用于存储 Associated Token Address 计算结果
static ATA_CACHE: Lazy<RwLock<CLruCache<AtaCacheKey, Pubkey>>> =
    Lazy::new(|| RwLock::new(CLruCache::new(NonZeroUsize::new(MAX_ATA_CACHE_SIZE).unwrap())));

/// 获取缓存的 Associated Token Address，如果不存在则计算并缓存
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

    // 尝试从缓存中获取（使用读锁）
    {
        let cache = ATA_CACHE.read();
        if let Some(cached_ata) = cache.peek(&cache_key) {
            return *cached_ata;
        }
    }

    // 缓存未命中，计算新的 ATA
    let ata = get_associated_token_address_with_program_id(
        wallet_address,
        token_mint_address,
        token_program_id,
    );

    // 将计算结果存入缓存（使用写锁）
    {
        let mut cache = ATA_CACHE.write();
        cache.put(cache_key, ata);
    }

    ata
}

// --------------------- 初始化账号 ---------------------

pub fn fast_init(payer: &Pubkey) {
    crate::instruction::utils::pumpfun::get_user_volume_accumulator_pda(payer);
}
