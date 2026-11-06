//! Venue probe: for every signal mint, log which venue it resolves to and
//! how long resolution took — before we extend execution support.
//!
//! Output: JSONL, one record per mint, appended to VENUE_PROBE_LOG
//! (default data/venue-probe.jsonl):
//!   {"at":..., "mint":..., "venue":"pumpfun_curve|pumpswap|unknown",
//!    "resolve_ms":..., "price_sol":..., "complete":bool, "note":...}
//!
//! Usage:
//!   venue-probe --mint <mint> [--mint <mint> ...]
//!   venue-probe --socket /tmp/telsolsniper-events.sock     (live signals)
//!   venue-probe --file mints.txt                           (one per line)

use anyhow::Result;
use sol_trade_sdk::{
    common::SolanaRpcClient,
    instruction::utils::{
        pumpfun::fetch_bonding_curve_account,
        pumpswap::find_pool,
    },
    trading::core::params::PumpSwapParams,
};
use solana_sdk::pubkey::Pubkey;
use std::io::Write;
use std::str::FromStr;
use std::time::Instant;
use tracing::{info, warn};

const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

struct Config {
    rpc_url: String,
    log_path: String,
    mints: Vec<String>,
    socket: Option<String>,
}

impl Config {
    fn from_args() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut mints = Vec::new();
        let mut socket = None;
        let mut file = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--mint" => {
                    if let Some(value) = args.next() {
                        mints.push(value);
                    }
                }
                "--socket" => socket = args.next(),
                "--file" => file = args.next(),
                other => mints.push(other.to_string()),
            }
        }
        if let Some(path) = file {
            let content = std::fs::read_to_string(&path)?;
            for line in content.lines() {
                let mint = line.trim();
                if !mint.is_empty() && !mint.starts_with('#') {
                    mints.push(mint.to_string());
                }
            }
        }
        Ok(Self {
            rpc_url: std::env::var("RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into()),
            log_path: std::env::var("VENUE_PROBE_LOG")
                .unwrap_or_else(|_| "data/venue-probe.jsonl".into()),
            mints,
            socket,
        })
    }
}

#[derive(Debug)]
struct Resolution {
    venue: &'static str,
    price_sol: Option<f64>,
    complete: bool,
    note: String,
    resolve_ms: u128,
}

async fn resolve(rpc: &SolanaRpcClient, mint: &Pubkey) -> Resolution {
    let started = Instant::now();

    // 0. Canonical PumpSwap pool PDA first: pure local maths, one account read.
    // This is the same fast path the live engine uses.
    let canonical = sol_trade_sdk::instruction::utils::pumpswap::get_canonical_pool_pda(mint);
    if let Ok(params) = PumpSwapParams::from_pool_address_by_rpc(rpc, &canonical).await {
        let base = params.pool_base_token_reserves as f64;
        let quote = params.pool_quote_token_reserves as f64 / LAMPORTS_PER_SOL;
        return Resolution {
            venue: "pumpswap",
            price_sol: if base > 0.0 { Some(quote / (base / 1_000_000.0)) } else { None },
            complete: true,
            note: format!("pool={canonical}"),
            resolve_ms: started.elapsed().as_millis(),
        };
    }

    // 1. Active Pump.fun bonding curve.
    match fetch_bonding_curve_account(rpc, mint).await {
        Ok((curve, _)) => {
            let elapsed = started.elapsed().as_millis();
            if !curve.complete {
                return Resolution {
                    venue: "pumpfun_curve",
                    price_sol: Some(curve.get_token_price()),
                    complete: false,
                    note: format!("mcap={:.0} SOL", curve.get_market_cap_sol() as f64 / LAMPORTS_PER_SOL),
                    resolve_ms: elapsed,
                };
            }
            // Graduated: fall through to PumpSwap, keeping the elapsed time.
            match find_pool(rpc, mint).await {
                Ok(pool_address) => {
                    let params = PumpSwapParams::from_pool_address_by_rpc(rpc, &pool_address).await;
                    let elapsed = started.elapsed().as_millis();
                    match params {
                        Ok(params) => {
                            let base = params.pool_base_token_reserves as f64;
                            let quote = params.pool_quote_token_reserves as f64 / LAMPORTS_PER_SOL;
                            return Resolution {
                                venue: "pumpswap",
                                price_sol: if base > 0.0 { Some(quote / (base / 1_000_000.0)) } else { None },
                                complete: true,
                                note: format!("pool={pool_address}"),
                                resolve_ms: elapsed,
                            };
                        }
                        Err(error) => {
                            return Resolution {
                                venue: "pumpswap_unreadable",
                                price_sol: None,
                                complete: true,
                                note: error.to_string(),
                                resolve_ms: elapsed,
                            };
                        }
                    }
                }
                Err(error) => {
                    return Resolution {
                        venue: "graduated_no_pool",
                        price_sol: None,
                        complete: true,
                        note: error.to_string(),
                        resolve_ms: started.elapsed().as_millis(),
                    };
                }
            }
        }
        Err(curve_error) => {
            // 2. No curve: try PumpSwap directly (some launches never had one).
            match find_pool(rpc, mint).await {
                Ok(pool_address) => {
                    let elapsed = started.elapsed().as_millis();
                    match PumpSwapParams::from_pool_address_by_rpc(rpc, &pool_address).await {
                        Ok(params) => {
                            let base = params.pool_base_token_reserves as f64;
                            let quote = params.pool_quote_token_reserves as f64 / LAMPORTS_PER_SOL;
                            return Resolution {
                                venue: "pumpswap",
                                price_sol: if base > 0.0 { Some(quote / (base / 1_000_000.0)) } else { None },
                                complete: true,
                                note: format!("no curve; pool={pool_address}"),
                                resolve_ms: elapsed,
                            };
                        }
                        Err(error) => Resolution {
                            venue: "pumpswap_unreadable",
                            price_sol: None,
                            complete: true,
                            note: error.to_string(),
                            resolve_ms: elapsed,
                        },
                    }
                }
                Err(_) => Resolution {
                    venue: "unknown",
                    price_sol: None,
                    complete: false,
                    note: format!(
                        "no curve ({}), no pumpswap pool — likely another launchpad",
                        short(&curve_error.to_string())
                    ),
                    resolve_ms: started.elapsed().as_millis(),
                },
            }
        }
    }
}

fn short(text: &str) -> String {
    let clipped: String = text.chars().take(90).collect();
    clipped.replace('\n', " ")
}

fn log_record(path: &str, record: &serde_json::Value) -> Result<()> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{record}")?;
    Ok(())
}

async fn probe_one(rpc: &SolanaRpcClient, config: &Config, mint_str: &str) {
    let Ok(mint) = Pubkey::from_str(mint_str) else {
        warn!(mint = mint_str, "invalid mint");
        return;
    };
    let resolution = resolve(rpc, &mint).await;
    let record = serde_json::json!({
        "at": unix_ms(),
        "mint": mint_str,
        "venue": resolution.venue,
        "resolve_ms": resolution.resolve_ms,
        "price_sol": resolution.price_sol,
        "complete": resolution.complete,
        "note": resolution.note,
    });
    if let Err(error) = log_record(&config.log_path, &record) {
        warn!(%error, "failed to write probe log");
    }
    info!(
        mint = mint_str,
        venue = resolution.venue,
        resolve_ms = resolution.resolve_ms,
        price_sol = ?resolution.price_sol,
        note = %short(&resolution.note),
        "venue resolved"
    );
}

fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_args()?;
    let rpc = SolanaRpcClient::new(config.rpc_url.clone());

    // --pda: print the canonical PumpSwap pool PDA for each mint (no network).
    if std::env::args().any(|a| a == "--pda") {
        for mint_str in &config.mints {
            if let Ok(mint) = Pubkey::from_str(mint_str) {
                let pool = sol_trade_sdk::instruction::utils::pumpswap::get_canonical_pool_pda(&mint);
                println!("{mint_str} -> {pool}");
            }
        }
        return Ok(());
    }

    for mint in &config.mints {
        probe_one(&rpc, &config, mint).await;
    }

    if let Some(socket) = &config.socket {
        info!(%socket, "watching live signals for venue resolution");
        loop {
            let stream = match tokio::net::UnixStream::connect(socket).await {
                Ok(stream) => stream,
                Err(error) => {
                    warn!(%error, "event socket unavailable; retrying");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                if let Some(mint) = event.get("mint").and_then(|m| m.as_str()) {
                    probe_one(&rpc, &config, mint).await;
                }
            }
        }
    }

    Ok(())
}
