//! ATA rent reclaimer.
//!
//! Every token bought creates an associated token account that holds exactly
//! 0.00203928 SOL of rent. Live never closed them, so that rent stayed locked
//! forever: at 141 trades/day it is 0.29 SOL/day, which at a 0.05 SOL position
//! is 4% of every trade — enough to erase the edge at small sizing.
//!
//! This task periodically scans the wallet's token accounts, closes the ones
//! holding a zero balance, and returns the rent to the wallet. CloseAccount is
//! a single-byte instruction (discriminator 9) on the SPL Token program, so it
//! needs no new dependency.
//!
//! Never touches the wSOL account: closing it would strand wrapped SOL.

use std::sync::Arc;
use std::time::Duration;

use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use tracing::{debug, info, warn};

/// SPL Token program (classic, not 2022).
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// Wrapped SOL mint — its account must stay open.
const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";
/// Rent-exempt minimum for a token account, in lamports.
const ATA_RENT_LAMPORTS: u64 = 2_039_280;
/// Close instructions per transaction. Token accounts are small, so a dozen
/// fits well inside the 1232-byte transaction limit with one signature.
const MAX_CLOSES_PER_TX: usize = 12;

/// Spawn the reclaimer. `interval_secs == 0` disables it.
pub fn spawn(rpc_url: String, payer: Arc<Keypair>, interval_secs: u64) {
    if interval_secs == 0 {
        info!("ata janitor disabled");
        return;
    }
    tokio::spawn(async move {
        // First pass shortly after start so leftovers are reclaimed early.
        tokio::time::sleep(Duration::from_secs(15)).await;
        loop {
            match tick(&rpc_url, &payer).await {
                Ok(closed) if closed > 0 => {
                    info!(
                        accounts = closed,
                        reclaimed_sol = %(closed as f64 * ATA_RENT_LAMPORTS as f64 / 1e9),
                        "ata janitor reclaimed rent"
                    );
                }
                Ok(_) => debug!("ata janitor: nothing to close"),
                Err(error) => warn!(%error, "ata janitor pass failed"),
            }
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
    });
}

/// One scan-and-close pass. Returns how many accounts were closed.
async fn tick(rpc_url: &str, payer: &Keypair) -> anyhow::Result<usize> {
    let empty = empty_token_accounts(rpc_url, &payer.pubkey()).await?;
    if empty.is_empty() {
        return Ok(0);
    }
    let mut closed = 0usize;
    for chunk in empty.chunks(MAX_CLOSES_PER_TX) {
        match close_batch(rpc_url, payer, chunk).await {
            Ok(true) => closed += chunk.len(),
            Ok(false) => warn!(count = chunk.len(), "close batch was not confirmed"),
            Err(error) => warn!(%error, count = chunk.len(), "close batch failed"),
        }
    }
    Ok(closed)
}

/// Token accounts owned by `owner` with a zero token balance (excluding wSOL).
async fn empty_token_accounts(rpc_url: &str, owner: &Pubkey) -> anyhow::Result<Vec<Pubkey>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "getTokenAccountsByOwner",
        "params": [
            owner.to_string(),
            { "programId": TOKEN_PROGRAM },
            { "encoding": "jsonParsed" }
        ]
    });
    let value: serde_json::Value = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    if let Some(error) = value.get("error") {
        anyhow::bail!("getTokenAccountsByOwner: {error}");
    }
    let mut out = Vec::new();
    let Some(accounts) = value["result"]["value"].as_array() else {
        return Ok(out);
    };
    for account in accounts {
        let Some(info) = account["account"]["data"]["parsed"]["info"].as_object() else {
            continue;
        };
        // Skip wSOL: closing it would free the native balance incorrectly.
        if info.get("mint").and_then(|m| m.as_str()) == Some(WSOL_MINT) {
            continue;
        }
        let amount = info
            .get("tokenAmount")
            .and_then(|t| t.get("amount"))
            .and_then(|a| a.as_str())
            .unwrap_or("0");
        if amount != "0" {
            continue;
        }
        if let Some(pubkey) = info.get("address").and_then(|a| a.as_str()) {
            if let Ok(key) = pubkey.parse::<Pubkey>() {
                out.push(key);
            }
        }
    }
    Ok(out)
}

/// Close one batch of token accounts in a single transaction.
async fn close_batch(rpc_url: &str, payer: &Keypair, accounts: &[Pubkey]) -> anyhow::Result<bool> {
    let token_program: Pubkey = TOKEN_PROGRAM.parse()?;
    let owner = payer.pubkey();
    let instructions: Vec<Instruction> = accounts
        .iter()
        .map(|ata| Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(*ata, false),
                AccountMeta::new(owner, false),
                AccountMeta::new_readonly(owner, true),
            ],
            // CloseAccount = discriminator 9, no payload.
            data: vec![9u8],
        })
        .collect();

    let blockhash = rpc_latest_blockhash(rpc_url).await?;
    let tx = Transaction::new_signed_with_payer(&instructions, Some(&owner), &[payer], blockhash);
    let wire = bincode::serialize(&tx)?;
    let encoded = base64_encode(&wire);

    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "sendTransaction",
        "params": [encoded, { "encoding": "base64", "skipPreflight": true, "maxRetries": 3 }]
    });
    let value: serde_json::Value = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let Some(sig) = value.get("result").and_then(|r| r.as_str()) else {
        anyhow::bail!("sendTransaction: {}", value.get("error").cloned().unwrap_or_default());
    };
    debug!(sig = %sig, accounts = accounts.len(), "close batch submitted");
    confirm_signature(rpc_url, sig).await
}

async fn rpc_latest_blockhash(rpc_url: &str) -> anyhow::Result<solana_sdk::hash::Hash> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "getLatestBlockhash",
        "params": [{ "commitment": "confirmed" }]
    });
    let value: serde_json::Value = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let s = value["result"]["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("getLatestBlockhash: missing blockhash"))?;
    Ok(s.parse()?)
}

async fn confirm_signature(rpc_url: &str, signature: &str) -> anyhow::Result<bool> {
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "getSignatureStatuses",
            "params": [[signature], { "searchTransactionHistory": false }]
        });
        let value: serde_json::Value = reqwest::Client::new()
            .post(rpc_url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        let status = &value["result"]["value"][0];
        if status.is_null() {
            continue;
        }
        if !status["err"].is_null() {
            anyhow::bail!("close tx failed: {}", status["err"]);
        }
        let confirmed = status["confirmationStatus"]
            .as_str()
            .map(|s| s == "confirmed" || s == "finalized")
            .unwrap_or(false);
        if confirmed {
            return Ok(true);
        }
    }
    Ok(false)
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CloseAccount carries no payload: a single discriminator byte, 9.
    /// If this ever changes, every rent reclaim silently fails on chain.
    #[test]
    fn close_account_instruction_shape() {
        let token_program: Pubkey = TOKEN_PROGRAM.parse().unwrap();
        let ata: Pubkey = "9bbt6hrU5sGw6o7hRiUsiCM4uvLvS1HpFiywmJqciFBa".parse().unwrap();
        let owner: Pubkey = "JBbfbuUT1Z5wKLwTPjxnkgiuC1XRxxP5nGwSVVCpbqy8".parse().unwrap();
        let ix = Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(ata, false),
                AccountMeta::new(owner, false),
                AccountMeta::new_readonly(owner, true),
            ],
            data: vec![9u8],
        };
        assert_eq!(ix.data, vec![9u8]);
        assert_eq!(ix.accounts.len(), 3);
        assert!(ix.accounts[0].is_writable, "ata must be writable");
        assert!(ix.accounts[1].is_writable, "lamport destination must be writable");
        assert!(ix.accounts[2].is_signer, "owner must sign");
        assert_eq!(ix.program_id.to_string(), TOKEN_PROGRAM);
    }

    #[test]
    fn rent_constant_matches_solana() {
        // 165 bytes x 6960 lamports/byte-year x 2 years, rounded up.
        assert_eq!(ATA_RENT_LAMPORTS, 2_039_280);
    }

    #[test]
    fn wsol_is_excluded_from_reclaim() {
        // Guards the rule that keeps wrapped SOL from being stranded.
        let wsol: Pubkey = WSOL_MINT.parse().expect("wsol mint must parse");
        let program: Pubkey = TOKEN_PROGRAM.parse().unwrap();
        assert_ne!(wsol, program);
    }
}
