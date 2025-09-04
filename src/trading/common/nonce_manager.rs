use anyhow::anyhow;
use solana_hash::Hash;
use solana_sdk::{instruction::Instruction, signature::Keypair, signer::Signer};
use solana_system_interface::instruction::advance_nonce_account;

use crate::common::nonce_cache::NonceCache;

/// Add nonce advance instruction to the instruction set
///
/// Nonce functionality is only used when nonce_pubkey is provided
/// Returns error if nonce is locked, already used, or not ready
/// On success, locks and marks nonce as used
pub fn add_nonce_instruction(
    instructions: &mut Vec<Instruction>,
    payer: &Keypair,
) -> Result<(), anyhow::Error> {
    let nonce_cache = NonceCache::get_instance();
    let nonce_info = nonce_cache.get_nonce_info();

    // Only check if nonce_account exists
    if let Some(nonce_pubkey) = nonce_info.nonce_account {
        if nonce_info.used {
            return Err(anyhow!("Nonce is used"));
        }
        if nonce_info.current_nonce == Hash::default() {
            return Err(anyhow!("Nonce is not ready"));
        }

        // Create Solana system nonce advance instruction - using system program ID
        let nonce_advance_ix = advance_nonce_account(&nonce_pubkey, &payer.pubkey());

        instructions.push(nonce_advance_ix);
    }

    Ok(())
}

/// Get blockhash for transaction
/// If nonce account is used, return blockhash from nonce, otherwise return the provided recent_blockhash
pub fn get_transaction_blockhash(recent_blockhash: Hash) -> Hash {
    let nonce_cache = NonceCache::get_instance();
    let nonce_info = nonce_cache.get_nonce_info();

    if nonce_info.nonce_account.is_some() {
        nonce_info.current_nonce
    } else {
        recent_blockhash
    }
}

/// Check if using nonce account
pub fn is_using_nonce() -> bool {
    let nonce_cache = NonceCache::get_instance();
    let nonce_info = nonce_cache.get_nonce_info();
    nonce_info.nonce_account.is_some()
}
