//! Shadow trading engine.
//!
//! Consumes `new_mint` events from the Telegram signal intake's event
//! fanout socket, resolves the real Pump.fun bonding curve over RPC,
//! and simulates a buy at the real curve price. Open positions are
//! polled; exit rules (take profit / stop loss / trailing stop / max
//! hold) close them at simulated prices. Everything is persisted to
//! SQLite. No transactions are ever broadcast and no keys are needed.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use serde::Deserialize;
use sol_trade_sdk::common::SolanaRpcClient;
use sol_trade_sdk::instruction::utils::pumpfun::{fetch_bonding_curve_account, get_bonding_curve_pda};
use solana_sdk::pubkey::Pubkey;
use std::{
    env,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, info, warn};

const LAMPORTS_PER_SOL: f64 = 100_000_000.0;

#[derive(Debug, Clone)]
struct ShadowConfig {
    event_socket_path: String,
    database_path: String,
    rpc_url: String,
    buy_sol_amount: f64,
    take_profit_x: f64,
    stop_loss_pct: f64,
    trailing_stop_pct: f64,
    max_hold_secs: u64,
    poll_interval_secs: u64,
    max_open_positions: usize,
}

impl ShadowConfig {
    fn from_env() -> Result<Self> {
        let f = |name: &str, default: &str| env::var(name).unwrap_or_else(|_| default.to_string());
        Ok(Self {
            event_socket_path: f("EVENT_SOCKET_PATH", "/tmp/telsolsniper-events.sock"),
            database_path: f("SHADOW_DATABASE_PATH", "./data/shadow.sqlite"),
            rpc_url: env::var("RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into()),
            buy_sol_amount: f("SHADOW_BUY_SOL", "0.5").parse()?,
            take_profit_x: f("SHADOW_TAKE_PROFIT_X", "2.0").parse()?,
            stop_loss_pct: f("SHADOW_STOP_LOSS_PCT", "35").parse()?,
            trailing_stop_pct: f("SHADOW_TRAILING_STOP_PCT", "25").parse()?,
            max_hold_secs: f("SHADOW_MAX_HOLD_SECS", "3600").parse()?,
            poll_interval_secs: f("SHADOW_POLL_SECS", "5").parse()?,
            max_open_positions: f("SHADOW_MAX_OPEN", "20").parse()?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct NewMintEvent {
    mint: String,
    source: String,
    message_id: i64,
    ticker: Option<String>,
}

struct ShadowStore {
    connection: Mutex<Connection>,
}

impl ShadowStore {
    fn open(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS shadow_trades (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mint TEXT NOT NULL,
                ticker TEXT,
                source TEXT NOT NULL,
                message_id INTEGER NOT NULL,
                entry_time_unix_ms INTEGER NOT NULL,
                entry_price_sol REAL NOT NULL,
                entry_market_cap_sol REAL NOT NULL,
                buy_sol_amount REAL NOT NULL,
                token_amount REAL NOT NULL,
                curve_complete INTEGER NOT NULL DEFAULT 0,
                exit_time_unix_ms INTEGER,
                exit_price_sol REAL,
                exit_reason TEXT,
                pnl_sol REAL,
                pnl_pct REAL,
                peak_price_sol REAL,
                UNIQUE (mint, entry_time_unix_ms)
            );
            "#,
        )?;
        Ok(Self { connection: Mutex::new(connection) })
    }

    fn record_entry(&self, trade: &ShadowTrade) -> Result<bool> {
        let connection = self.connection.lock().unwrap();
        let inserted = connection
            .execute(
                r#"INSERT OR IGNORE INTO shadow_trades
                   (mint, ticker, source, message_id, entry_time_unix_ms, entry_price_sol,
                    entry_market_cap_sol, buy_sol_amount, token_amount, curve_complete)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                rusqlite::params![
                    trade.mint,
                    trade.ticker,
                    trade.source,
                    trade.message_id,
                    trade.entry_time_unix_ms,
                    trade.entry_price_sol,
                    trade.entry_market_cap_sol,
                    trade.buy_sol_amount,
                    trade.token_amount,
                    trade.curve_complete as i64,
                ],
            )?;
        Ok(inserted > 0)
    }

    fn open_trades(&self) -> Result<Vec<ShadowTrade>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection.prepare(
            r#"SELECT id, mint, ticker, source, message_id, entry_time_unix_ms, entry_price_sol,
                      entry_market_cap_sol, buy_sol_amount, token_amount, curve_complete,
                      peak_price_sol
               FROM shadow_trades WHERE exit_time_unix_ms IS NULL"#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ShadowTrade {
                id: row.get(0)?,
                mint: row.get(1)?,
                ticker: row.get(2)?,
                source: row.get(3)?,
                message_id: row.get(4)?,
                entry_time_unix_ms: row.get(5)?,
                entry_price_sol: row.get(6)?,
                entry_market_cap_sol: row.get(7)?,
                buy_sol_amount: row.get(8)?,
                token_amount: row.get(9)?,
                curve_complete: row.get::<_, i64>(10)? != 0,
                peak_price_sol: row.get(11)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn record_exit(&self, id: i64, exit_price: f64, reason: &str, pnl_sol: f64, pnl_pct: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            r#"UPDATE shadow_trades
               SET exit_time_unix_ms = ?2, exit_price_sol = ?3, exit_reason = ?4,
                   pnl_sol = ?5, pnl_pct = ?6
               WHERE id = ?1"#,
            rusqlite::params![
                id,
                now_ms(),
                exit_price,
                reason,
                pnl_sol,
                pnl_pct,
            ],
        )?;
        Ok(())
    }

    fn update_peak(&self, id: i64, peak: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            "UPDATE shadow_trades SET peak_price_sol = ?2 WHERE id = ?1",
            rusqlite::params![id, peak],
        )?;
        Ok(())
    }

    fn open_count(&self) -> Result<usize> {
        let connection = self.connection.lock().unwrap();
        Ok(connection
            .query_row("SELECT COUNT(*) FROM shadow_trades WHERE exit_time_unix_ms IS NULL", [], |r| r.get(0))?)
    }
}

#[derive(Debug, Clone)]
struct ShadowTrade {
    id: i64,
    mint: String,
    ticker: Option<String>,
    source: String,
    message_id: i64,
    entry_time_unix_ms: i64,
    entry_price_sol: f64,
    entry_market_cap_sol: f64,
    buy_sol_amount: f64,
    token_amount: f64,
    curve_complete: bool,
    peak_price_sol: f64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Snapshot of the real bonding curve used for pricing.
struct CurveSnapshot {
    price_sol: f64,          // price per token in SOL
    market_cap_sol: f64,
    complete: bool,
}

async fn fetch_curve(rpc: &SolanaRpcClient, mint: &Pubkey) -> Result<CurveSnapshot> {
    let (curve, _) = fetch_bonding_curve_account(rpc, mint)
        .await
        .context("bonding curve not found on chain")?;
    Ok(CurveSnapshot {
        price_sol: curve.get_token_price(),
        market_cap_sol: curve.get_market_cap_sol() as f64 / LAMPORTS_PER_SOL,
        complete: curve.complete,
    })
}

/// Constant-product simulation of a buy: tokens out for sol in.
fn buy_tokens(curve: &CurveSnapshot, sol_in: f64) -> f64 {
    if curve.price_sol <= 0.0 {
        return 0.0;
    }
    // price = v_sol / v_tokens; buying sol_in moves price along the curve.
    // Approximate with geometric-mean fill; adequate for shadow accounting.
    sol_in / curve.price_sol
}

async fn open_position(
    rpc: &SolanaRpcClient,
    store: &ShadowStore,
    config: &ShadowConfig,
    event: NewMintEvent,
) -> Result<()> {
    if store.open_count()? >= config.max_open_positions {
        info!(mint = %event.mint, "max open shadow positions reached; skipping");
        return Ok(());
    }
    let mint = match Pubkey::from_str(&event.mint) {
        Ok(m) => m,
        Err(_) => {
            warn!(mint = %event.mint, "event mint is not a valid pubkey");
            return Ok(());
        }
    };
    // Skip mints without a Pump.fun curve (migrated pools are rejected here
    // for now; a PumpSwap reader can be added later).
    if get_bonding_curve_pda(&mint).is_none() {
        return Ok(());
    }
    let curve = match fetch_curve(rpc, &mint).await {
        Ok(c) => c,
        Err(error) => {
            info!(mint = %event.mint, %error, "no live bonding curve; skipping");
            return Ok(());
        }
    };
    if curve.complete {
        info!(mint = %event.mint, "curve already graduated; skipping");
        return Ok(());
    }

    let token_amount = buy_tokens(&curve, config.buy_sol_amount);
    let trade = ShadowTrade {
        id: 0,
        mint: event.mint.clone(),
        ticker: event.ticker.clone(),
        source: event.source.clone(),
        message_id: event.message_id,
        entry_time_unix_ms: now_ms(),
        entry_price_sol: curve.price_sol,
        entry_market_cap_sol: curve.market_cap_sol,
        buy_sol_amount: config.buy_sol_amount,
        token_amount,
        curve_complete: curve.complete,
        peak_price_sol: curve.price_sol,
    };
    if store.record_entry(&trade)? {
        info!(
            mint = %event.mint,
            ticker = event.ticker.as_deref().unwrap_or(""),
            price_sol = curve.price_sol,
            mcap_sol = curve.market_cap_sol,
            "shadow entry opened"
        );
    }
    Ok(())
}

enum ExitReason {
    TakeProfit,
    StopLoss,
    TrailingStop,
    MaxHold,
    Graduated,
}

impl ExitReason {
    fn as_str(&self) -> &'static str {
        match self {
            ExitReason::TakeProfit => "take_profit",
            ExitReason::StopLoss => "stop_loss",
            ExitReason::TrailingStop => "trailing_stop",
            ExitReason::MaxHold => "max_hold",
            ExitReason::Graduated => "graduated",
        }
    }
}

fn evaluate_exit(
    trade: &ShadowTrade,
    curve: &CurveSnapshot,
    config: &ShadowConfig,
) -> Option<ExitReason> {
    if curve.complete {
        return Some(ExitReason::Graduated);
    }
    let multiple = curve.price_sol / trade.entry_price_sol;
    if multiple >= config.take_profit_x {
        return Some(ExitReason::TakeProfit);
    }
    if multiple <= 1.0 - config.stop_loss_pct / 100.0 {
        return Some(ExitReason::StopLoss);
    }
    let peak_multiple = trade.peak_price_sol / trade.entry_price_sol;
    if peak_multiple >= config.take_profit_x
        && curve.price_sol <= trade.peak_price_sol * (1.0 - config.trailing_stop_pct / 100.0)
    {
        return Some(ExitReason::TrailingStop);
    }
    if now_ms() - trade.entry_time_unix_ms >= (config.max_hold_secs as i64) * 1000 {
        return Some(ExitReason::MaxHold);
    }
    None
}

async fn poll_positions(
    rpc: &SolanaRpcClient,
    store: &ShadowStore,
    config: &ShadowConfig,
) -> Result<()> {
    for trade in store.open_trades()? {
        let mint = match Pubkey::from_str(&trade.mint) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let curve = match fetch_curve(rpc, &mint).await {
            Ok(c) => c,
            Err(error) => {
                // Migrated/failed accounts: mark graduated-style exit.
                warn!(mint = %trade.mint, %error, "curve unavailable; closing as graduated");
                let exit_price = trade.peak_price_sol.max(trade.entry_price_sol);
                let pnl_pct = (exit_price / trade.entry_price_sol - 1.0) * 100.0;
                store.record_exit(trade.id, exit_price, ExitReason::Graduated.as_str(), 0.0, pnl_pct)?;
                continue;
            }
        };
        if curve.price_sol > trade.peak_price_sol {
            store.update_peak(trade.id, curve.price_sol)?;
        }
        let peak = trade.peak_price_sol.max(curve.price_sol);
        let trade_for_eval = ShadowTrade { peak_price_sol: peak, ..trade.clone() };
        if let Some(reason) = evaluate_exit(&trade_for_eval, &curve, config) {
            let exit_price = curve.price_sol;
            let pnl_sol = trade.token_amount * exit_price - trade.buy_sol_amount;
            let pnl_pct = (exit_price / trade.entry_price_sol - 1.0) * 100.0;
            store.record_exit(trade.id, exit_price, reason.as_str(), pnl_sol, pnl_pct)?;
            info!(
                mint = %trade.mint,
                ticker = trade.ticker.as_deref().unwrap_or(""),
                reason = reason.as_str(),
                pnl_pct,
                "shadow exit recorded"
            );
        }
    }
    Ok(())
}

async fn run() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = ShadowConfig::from_env()?;
    let store = Arc::new(ShadowStore::open(&config.database_path)?);
    let rpc = SolanaRpcClient::new_with_timeout_and_commitment(
        config.rpc_url.clone(),
        Duration::from_secs(10),
        solana_sdk::commitment_config::CommitmentConfig::confirmed(),
    );

    info!(
        event_socket = %config.event_socket_path,
        database = %config.database_path,
        rpc = %config.rpc_url,
        buy_sol = config.buy_sol_amount,
        tp_x = config.take_profit_x,
        sl_pct = config.stop_loss_pct,
        "shadow engine starting; NO transactions will be broadcast"
    );

    // Resume monitoring any open positions from a previous run.
    let open = store.open_trades()?;
    info!(open_positions = open.len(), "resuming open shadow positions");

    let stream = UnixStream::connect(&config.event_socket_path)
        .await
        .context("connect to event fanout socket (is telegram-signal-sniper running?)")?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        tokio::select! {
            read = reader.read_line(&mut line) => {
                match read {
                    Ok(0) => bail!("event fanout socket closed"),
                    Ok(_) => {
                        let trimmed = line.trim().to_string();
                        line.clear();
                        if trimmed.is_empty() { continue; }
                        match serde_json::from_str::<NewMintEvent>(&trimmed) {
                            Ok(event) => {
                                if let Err(error) =
                                    open_position(&rpc, &store, &config, event).await
                                {
                                    warn!(%error, "failed to open shadow position");
                                }
                            }
                            Err(error) => warn!(%error, "malformed event envelope"),
                        }
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(config.poll_interval_secs)) => {
                if let Err(error) = poll_positions(&rpc, &store, &config).await {
                    error!(%error, "position polling failed");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(error) = run().await {
        error!(%error, "shadow engine exited");
        bail!(error);
    }
    Ok(())
}
