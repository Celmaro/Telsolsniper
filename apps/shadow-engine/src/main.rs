//! Shadow trading engine.
//!
//! Consumes `new_mint` events from the Telegram signal intake's event
//! fanout socket, resolves the real Pump.fun bonding curve over RPC,
//! and simulates a buy at the real curve price. Open positions are
//! polled; exit rules (take profit / stop loss / trailing stop / max
//! hold) close them at simulated prices. Everything is persisted to
//! SQLite. No transactions are ever broadcast and no keys are needed.

mod hedged_rpc;

use anyhow::{bail, Context, Result};
use hedged_rpc::HedgedRpc;
use rusqlite::Connection;
use serde::Deserialize;
use sol_trade_sdk::common::SolanaRpcClient;
use sol_trade_sdk::instruction::utils::pumpfun::{fetch_bonding_curve_account, get_bonding_curve_pda};
use sol_trade_sdk::instruction::utils::pumpswap::find_pool;
use sol_trade_sdk::trading::core::params::PumpSwapParams;
use sol_trade_sdk::utils::calc::pumpswap::{
    buy_quote_input_internal_with_fees, sell_base_input_internal_with_fees,
};
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
    rpc_urls: Vec<String>,
    buy_sol_amount: f64,
    // Staged exit: TP1 / TP2 / moon bag
    tp1_multiple: f64,
    tp1_fraction: f64,
    tp2_multiple: f64,
    tp2_fraction: f64,
    moon_trail_pct: f64,
    initial_stop_pct: f64,
    breakeven_after_tp1: bool,
    max_hold_secs: u64,
    poll_interval_secs: u64,
    max_open_positions: usize,
    poll_concurrency: usize,
}

impl ShadowConfig {
    fn from_env() -> Result<Self> {
        let f = |name: &str, default: &str| env::var(name).unwrap_or_else(|_| default.to_string());
        // Comma-separated list; all endpoints are raced in parallel per request.
        let rpc_urls: Vec<String> = f(
            "RPC_URLS",
            "https://api.mainnet-beta.solana.com,https://solana-rpc.publicnode.com",
        )
        .split(',')
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
        .collect();
        if rpc_urls.is_empty() {
            bail!("RPC_URLS must contain at least one endpoint");
        }
        let cfg = Self {
            event_socket_path: f("EVENT_SOCKET_PATH", "/tmp/telsolsniper-events.sock"),
            database_path: f("SHADOW_DATABASE_PATH", "./data/shadow.sqlite"),
            rpc_urls,
            buy_sol_amount: f("SHADOW_BUY_SOL", "0.5").parse()?,
            tp1_multiple: f("SHADOW_TP1_X", "1.1").parse()?,
            tp1_fraction: f("SHADOW_TP1_SELL_PCT", "40").parse()?,
            tp2_multiple: f("SHADOW_TP2_X", "1.5").parse()?,
            tp2_fraction: f("SHADOW_TP2_SELL_PCT", "40").parse()?,
            moon_trail_pct: f("SHADOW_MOON_TRAIL_PCT", "35").parse()?,
            initial_stop_pct: f("SHADOW_STOP_LOSS_PCT", "35").parse()?,
            breakeven_after_tp1: f("SHADOW_BREAKEVEN_AFTER_TP1", "true")
                .eq_ignore_ascii_case("true"),
            max_hold_secs: f("SHADOW_MAX_HOLD_SECS", "0").parse()?,
            poll_interval_secs: f("SHADOW_POLL_SECS", "5").parse()?,
            max_open_positions: f("SHADOW_MAX_OPEN", "20").parse()?,
            poll_concurrency: f("SHADOW_POLL_CONCURRENCY", "8").parse()?,
        };
        Self::validate(&cfg)?;
        Ok(cfg)
    }

    /// Staged ladder must leave a moon bag and escalate.
    fn validate(cfg: &Self) -> Result<()> {
        if cfg.tp2_multiple <= cfg.tp1_multiple {
            bail!("SHADOW_TP2_X must be greater than SHADOW_TP1_X");
        }
        if cfg.tp1_fraction <= 0.0 || cfg.tp1_fraction >= 100.0 {
            bail!("SHADOW_TP1_SELL_PCT must be between 0 and 100");
        }
        if cfg.tp2_fraction <= 0.0 || cfg.tp1_fraction + cfg.tp2_fraction >= 100.0 {
            bail!("SHADOW_TP1_SELL_PCT + SHADOW_TP2_SELL_PCT must be below 100 (leave a moon bag)");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
                tp1_done INTEGER NOT NULL DEFAULT 0,
                tp2_done INTEGER NOT NULL DEFAULT 0,
                realized_pnl_sol REAL NOT NULL DEFAULT 0,
                remaining_tokens REAL NOT NULL DEFAULT 0,
                UNIQUE (mint, entry_time_unix_ms)
            );
            CREATE TABLE IF NOT EXISTS shadow_fills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                trade_id INTEGER NOT NULL REFERENCES shadow_trades(id),
                fill_time_unix_ms INTEGER NOT NULL,
                kind TEXT NOT NULL,             -- tp1 | tp2 | moon_bag | stop | graduated
                multiple REAL NOT NULL,         -- price / entry_price at fill
                price_sol REAL NOT NULL,
                tokens_sold REAL NOT NULL,
                proceeds_sol REAL NOT NULL
            );
            "#,
        )?;
        // Migration: pre-staging databases lack the staged-exit columns.
        let existing_columns: Vec<String> = {
            let mut stmt =
                connection.prepare("PRAGMA table_info(shadow_trades)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let alter = |col: &str, ddl: &str| -> Result<()> {
            if !existing_columns.iter().any(|c| c == col) {
                connection.execute(&format!("ALTER TABLE shadow_trades ADD COLUMN {ddl}"), [])?;
            }
            Ok(())
        };
        alter("tp1_done", "tp1_done INTEGER NOT NULL DEFAULT 0")?;
        alter("tp2_done", "tp2_done INTEGER NOT NULL DEFAULT 0")?;
        alter("realized_pnl_sol", "realized_pnl_sol REAL NOT NULL DEFAULT 0")?;
        alter("remaining_tokens", "remaining_tokens REAL NOT NULL DEFAULT 0")?;
        Ok(Self { connection: Mutex::new(connection) })
    }

    fn record_entry(&self, trade: &ShadowTrade) -> Result<bool> {
        let connection = self.connection.lock().unwrap();
        let inserted = connection
            .execute(
                r#"INSERT OR IGNORE INTO shadow_trades
                   (mint, ticker, source, message_id, entry_time_unix_ms, entry_price_sol,
                    entry_market_cap_sol, buy_sol_amount, token_amount, curve_complete, peak_price_sol)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?6)"#,
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
                      peak_price_sol, tp1_done, tp2_done, realized_pnl_sol, remaining_tokens
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
                tp1_done: row.get::<_, i64>(12)? != 0,
                tp2_done: row.get::<_, i64>(13)? != 0,
                realized_pnl_sol: row.get(14)?,
                remaining_tokens: row.get(15)?,
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

    fn record_fill(
        &self,
        trade_id: i64,
        kind: &str,
        multiple: f64,
        price_sol: f64,
        tokens_sold: f64,
        proceeds_sol: f64,
    ) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            r#"INSERT INTO shadow_fills (trade_id, fill_time_unix_ms, kind, multiple, price_sol, tokens_sold, proceeds_sol)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            rusqlite::params![trade_id, now_ms(), kind, multiple, price_sol, tokens_sold, proceeds_sol],
        )?;
        Ok(())
    }

    /// Persist a staged fill's effect. `kind` maps to tp1_done/tp2_done;
    /// fractional positions stay open with remaining_tokens set.
    fn update_stage(
        &self,
        id: i64,
        kind: &str,
        realized_pnl_sol: f64,
        remaining_tokens: f64,
        _fractional: bool,
    ) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        let tp1 = if kind == "tp1" { 1 } else { 0 };
        let tp2 = if kind == "tp2" { 1 } else { 0 };
        connection.execute(
            r#"UPDATE shadow_trades
               SET realized_pnl_sol = ?2, remaining_tokens = ?3,
                   tp1_done = tp1_done | ?4, tp2_done = tp2_done | ?5
               WHERE id = ?1"#,
            rusqlite::params![id, realized_pnl_sol, remaining_tokens, tp1, tp2],
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
    tp1_done: bool,
    tp2_done: bool,
    realized_pnl_sol: f64,
    remaining_tokens: f64,
}

impl ShadowTrade {
    fn tokens_at_entry(&self) -> f64 {
        if self.remaining_tokens > 0.0 {
            self.remaining_tokens
        } else {
            self.token_amount
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Snapshot of the real bonding curve used for pricing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Venue {
    #[allow(dead_code)] // logged in future work; kept for venue-specific rules
    PumpFunCurve,
    PumpSwap,
}

#[derive(Clone)]
struct CurveSnapshot {
    #[allow(dead_code)] // venue-specific exit rules planned; venue set on every snapshot
    venue: Venue,
    /// Price per whole token in SOL (curve) — pool snapshot for PumpSwap
    price_sol: f64,
    market_cap_sol: f64,
    complete: bool,
    /// PumpSwap pool state for constant-product sim (None on curve venue)
    pool: Option<PumpSwapParams>,
}

/// Price an active Pump.fun curve; fall back to its graduated PumpSwap
/// pool so shadow positions stay priced across graduation. The curve
/// read is a single getAccountInfo — hedged across all endpoints.
async fn fetch_curve(rpc: &HedgedRpc, mint: &Pubkey) -> Result<CurveSnapshot> {
    if let Some(curve_pda) = get_bonding_curve_pda(mint) {
        let mint = *mint;
        let curve = rpc
            .hedge(move |client| async move {
                fetch_curve_via_client(&client, &mint, &curve_pda).await
            })
            .await;
        if let Ok(curve) = curve {
            return Ok(curve);
        }
    }
    // Graduated or missing curve: fall back to the PumpSwap pool.
    let mint = *mint;
    let pool_address = rpc
        .hedge(move |client| async move { find_pool(&client, &mint).await })
        .await
        .context("no active bonding curve and no PumpSwap pool found for mint")?;
    let pool = PumpSwapParams::from_pool_address_by_rpc(rpc.primary(), &pool_address)
        .await
        .context("found PumpSwap pool but failed to read its state")?;
    let quote_reserve = pool.effective_quote_reserves()? as f64 / LAMPORTS_PER_SOL;
    let base_reserve = pool.pool_base_token_reserves as f64;
    if base_reserve <= 0.0 {
        bail!("PumpSwap pool has zero base reserves");
    }
    let price_sol = quote_reserve / (base_reserve / 1_000_000.0); // base mint is 6dp
    Ok(CurveSnapshot {
        venue: Venue::PumpSwap,
        price_sol,
        // Rough market cap from pool depth; quote vault assumes SOL pairing.
        market_cap_sol: quote_reserve * 2.0,
        complete: false,
        pool: Some(pool),
    })
}

/// One endpoint's curve read: getAccountInfo on the curve PDA, decode.
async fn fetch_curve_via_client(
    client: &SolanaRpcClient,
    mint: &Pubkey,
    _curve_pda: &Pubkey,
) -> Result<CurveSnapshot> {
    let (curve, _) = fetch_bonding_curve_account(client, mint)
        .await
        .context("bonding curve not found on chain")?;
    if curve.complete {
        bail!("curve graduated");
    }
    Ok(CurveSnapshot {
        venue: Venue::PumpFunCurve,
        price_sol: curve.get_token_price(),
        market_cap_sol: curve.get_market_cap_sol() as f64 / LAMPORTS_PER_SOL,
        complete: false,
        pool: None,
    })
}

/// Simulated buy: tokens out for sol in. PumpSwap uses the SDK's
/// constant-product math with real pool fees; the curve uses the
/// spot-price approximation (adequate for shadow accounting).
fn buy_tokens(curve: &CurveSnapshot, sol_in: f64) -> f64 {
    if sol_in <= 0.0 {
        return 0.0;
    }
    if let Some(pool) = &curve.pool {
        let quote_in_lamports = (sol_in * LAMPORTS_PER_SOL).max(1.0) as u64;
        if let Ok(result) = buy_quote_input_internal_with_fees(
            quote_in_lamports,
            0,
            pool.pool_base_token_reserves,
            pool.pool_quote_token_reserves,
            pool.virtual_quote_reserves,
            &pool.fee_basis_points,
        ) {
            return result.base as f64 / 1_000_000.0; // raw base units -> whole tokens (6dp supply)
        }
    }
    if curve.price_sol > 0.0 {
        sol_in / curve.price_sol
    } else {
        0.0
    }
}

/// Simulated sell: SOL out for whole tokens in (PumpSwap venue only).
fn sell_tokens(curve: &CurveSnapshot, tokens_in: f64) -> f64 {
    let Some(pool) = &curve.pool else {
        return tokens_in * curve.price_sol;
    };
    let base_in = (tokens_in * 1_000_000.0).max(1.0) as u64;
    match sell_base_input_internal_with_fees(
        base_in,
        0,
        pool.pool_base_token_reserves,
        pool.pool_quote_token_reserves,
        pool.virtual_quote_reserves,
        &pool.fee_basis_points,
    ) {
        Ok(result) => result.ui_quote as f64 / LAMPORTS_PER_SOL,
        Err(_) => tokens_in * curve.price_sol,
    }
}

async fn open_position(
    rpc: &HedgedRpc,
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
            info!(mint = %event.mint, %error, "no tradable venue (curve or PumpSwap); skipping");
            return Ok(());
        }
    };
    if curve.complete {
        info!(mint = %event.mint, "venue unavailable; skipping");
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
        tp1_done: false,
        tp2_done: false,
        realized_pnl_sol: 0.0,
        remaining_tokens: 0.0,
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

#[derive(Debug, PartialEq)]
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

/// One staged fill: partial sell executed against the current snapshot.
fn record_fill(
    store: &ShadowStore,
    trade: &ShadowTrade,
    kind: &str,
    price_sol: f64,
    tokens_sold: f64,
) -> Result<()> {
    let proceeds = tokens_sold * price_sol;
    store.record_fill(
        trade.id,
        kind,
        price_sol / trade.entry_price_sol,
        price_sol,
        tokens_sold,
        proceeds,
    )?;
    info!(
        mint = %trade.mint,
        kind,
        multiple = format!("{:.3}", price_sol / trade.entry_price_sol),
        tokens_sold,
        proceeds_sol = format!("{:.4}", proceeds),
        "staged fill"
    );
    Ok(())
}

/// Staged exit evaluation. Returns (fills_to_execute, close_position).
/// Rules, in priority order:
///   1. stop loss -SHADOW_STOP_LOSS_PCT (only before TP1 fires)
///   2. TP1 at SHADOW_TP1_X: sell TP1_SELL_PCT of the position
///   3. TP2 at SHADOW_TP2_X: sell TP2_SELL_PCT of the position
///   4. after TP1: breakeven stop — dump everything if price <= entry
///   5. moon bag: trailing stop SHADOW_MOON_TRAIL_PCT off peak, on the remainder
///   6. graduation: dump everything (curve complete)
fn evaluate_staged_exits(
    trade: &ShadowTrade,
    curve: &CurveSnapshot,
    config: &ShadowConfig,
) -> (Vec<(&'static str, f64)>, Option<ExitReason>) {
    let multiple = curve.price_sol / trade.entry_price_sol;
    let mut fills = Vec::new();

    if curve.complete {
        fills.push(("graduated", trade.tokens_at_entry()));
        return (fills, Some(ExitReason::Graduated));
    }

    if !trade.tp1_done && multiple >= config.tp1_multiple {
        let tokens = trade.tokens_at_entry() * config.tp1_fraction / 100.0;
        fills.push(("tp1", tokens));
    }
    if trade.tp1_done && !trade.tp2_done && multiple >= config.tp2_multiple {
        let tokens = trade.tokens_at_entry() * config.tp2_fraction / 100.0;
        fills.push(("tp2", tokens));
    }

    // Stops are evaluated on what remains after any TPs queued this tick.
    // Pre-TP1: hard stop. Post-TP1 (config.breakeven_after_tp1): breakeven
    // dump. Moon bag stage (post-TP2): trailing stop off peak.
    let stop_hit = if !trade.tp1_done {
        multiple <= 1.0 - config.initial_stop_pct / 100.0
    } else if !trade.tp2_done {
        config.breakeven_after_tp1 && multiple <= 1.0
    } else {
        trade.peak_price_sol / trade.entry_price_sol >= config.tp2_multiple
            && curve.price_sol
                <= trade.peak_price_sol * (1.0 - config.moon_trail_pct / 100.0)
    };

    if stop_hit {
        fills.push(("stop", trade.tokens_at_entry()));
        return (fills, Some(ExitReason::StopLoss));
    }

    let remaining_after_fills = trade.tokens_at_entry()
        - fills.iter().map(|(_, t)| *t).sum::<f64>();
    if remaining_after_fills <= 1.0 {
        // Everything sold — position is fully closed.
        return (fills, Some(ExitReason::TakeProfit));
    }
    (fills, None)
}

async fn poll_position(
    rpc: &HedgedRpc,
    store: &ShadowStore,
    config: &ShadowConfig,
    trade: ShadowTrade,
) -> Result<()> {
    let mint = match Pubkey::from_str(&trade.mint) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };
    let curve = match fetch_curve(rpc, &mint).await {
        Ok(c) => c,
        Err(error) => {
            // Migrated/failed accounts: mark graduated-style exit.
            warn!(mint = %trade.mint, %error, "curve unavailable; closing as graduated");
            let exit_price = trade.peak_price_sol.max(trade.entry_price_sol);
            let pnl_pct = (exit_price / trade.entry_price_sol - 1.0) * 100.0;
            store.record_exit(trade.id, exit_price, ExitReason::Graduated.as_str(), 0.0, pnl_pct)?;
            return Ok(());
        }
    };
    if curve.price_sol > trade.peak_price_sol {
        store.update_peak(trade.id, curve.price_sol)?;
    }
    let peak = trade.peak_price_sol.max(curve.price_sol);
    let trade_for_eval = ShadowTrade { peak_price_sol: peak, ..trade.clone() };
    let (fills, close_reason) = evaluate_staged_exits(&trade_for_eval, &curve, config);

    let mut realized = trade.realized_pnl_sol;
    let mut remaining = trade.tokens_at_entry();
    for (kind, tokens_sold) in &fills {
        // Venue-aware valuation: PumpSwap sell math includes LP+protocol fees.
        let proceeds = sell_tokens(&curve, *tokens_sold);
        record_fill(store, &trade_for_eval, kind, curve.price_sol, *tokens_sold)?;
        realized += proceeds - trade.buy_sol_amount * (tokens_sold / trade.token_amount);
        remaining -= tokens_sold;
        let fractional = remaining > 1.0 && close_reason.is_none();
        store.update_stage(trade_for_eval.id, kind, realized, remaining, fractional)?;
    }

    if let Some(reason) = close_reason {
        let pnl_sol = realized;
        let pnl_pct = (realized / trade.buy_sol_amount) * 100.0;
        store.record_exit(trade_for_eval.id, curve.price_sol, reason.as_str(), pnl_sol, pnl_pct)?;
        info!(
            mint = %trade.mint,
            ticker = trade.ticker.as_deref().unwrap_or(""),
            reason = reason.as_str(),
            pnl_pct,
            "shadow exit recorded"
        );
    }
    Ok(())
}

/// Poll all open positions concurrently with a bounded task pool.
/// Sequential polling with N positions took N × RPC-latency; this
/// takes ~1 × latency regardless of N.
async fn poll_positions(
    rpc: &HedgedRpc,
    store: Arc<ShadowStore>,
    config: Arc<ShadowConfig>,
) -> Result<()> {
    let open = store.open_trades()?;
    if open.is_empty() {
        return Ok(());
    }
    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.poll_concurrency));
    let mut handles = Vec::with_capacity(open.len());
    for trade in open {
        let permit = Arc::clone(&semaphore).acquire_owned().await?;
        let rpc = rpc.clone();
        let store = Arc::clone(&store);
        let config = Arc::clone(&config);
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = poll_position(&rpc, &store, &config, trade).await {
                warn!(%error, "position poll failed");
            }
        }));
    }
    for handle in handles {
        let _ = handle.await;
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

    let config = Arc::new(ShadowConfig::from_env()?);
    let store = Arc::new(ShadowStore::open(&config.database_path)?);
    let rpc = HedgedRpc::new(&config.rpc_urls);

    info!(
        event_socket = %config.event_socket_path,
        database = %config.database_path,
        rpc_endpoints = ?config.rpc_urls,
        buy_sol = config.buy_sol_amount,
        tp1_x = config.tp1_multiple,
        tp1_sell_pct = config.tp1_fraction,
        tp2_x = config.tp2_multiple,
        tp2_sell_pct = config.tp2_fraction,
        moon_trail_pct = config.moon_trail_pct,
        sl_pct = config.initial_stop_pct,
        "shadow engine starting; NO transactions will be broadcast"
    );

    // Resume monitoring any open positions from a previous run.
    let open = store.open_trades()?;
    info!(open_positions = open.len(), "resuming open shadow positions");

    let stream = {
        let mut last_error = None;
        let mut stream = None;
        for _ in 0..30 {
            match UnixStream::connect(&config.event_socket_path).await {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        match stream {
            Some(s) => s,
            None => {
                return Err(anyhow::Error::from(last_error.unwrap_or_else(|| {
                    std::io::Error::other("event socket connect failed")
                })))
                    .context("connect to event fanout socket (is telegram-signal-sniper running?)")
            }
        }
    };
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
                if let Err(error) = poll_positions(&rpc, Arc::clone(&store), Arc::clone(&config)).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn trade() -> ShadowTrade {
        ShadowTrade {
            id: 1,
            mint: "TESTmint11111111111111111111111111111111111".into(),
            ticker: Some("TEST".into()),
            source: "soltrenchtrending".into(),
            message_id: 1,
            entry_time_unix_ms: 0,
            entry_price_sol: 1e-6,
            entry_market_cap_sol: 100.0,
            buy_sol_amount: 0.5,
            token_amount: 500_000.0,
            curve_complete: false,
            peak_price_sol: 1e-6,
            tp1_done: false,
            tp2_done: false,
            realized_pnl_sol: 0.0,
            remaining_tokens: 0.0,
        }
    }

    fn snapshot(price_multiple: f64, complete: bool) -> CurveSnapshot {
        CurveSnapshot {
            venue: Venue::PumpFunCurve,
            price_sol: trade().entry_price_sol * price_multiple,
            market_cap_sol: 100.0 * price_multiple,
            complete,
            pool: None,
        }
    }

    fn config() -> ShadowConfig {
        ShadowConfig {
            event_socket_path: String::new(),
            database_path: String::new(),
            rpc_urls: vec![],
            buy_sol_amount: 0.5,
            tp1_multiple: 1.1,
            tp1_fraction: 40.0,
            tp2_multiple: 1.5,
            tp2_fraction: 40.0,
            moon_trail_pct: 35.0,
            initial_stop_pct: 35.0,
            breakeven_after_tp1: true,
            max_hold_secs: 0,
            poll_interval_secs: 5,
            max_open_positions: 20,
            poll_concurrency: 8,
        }
    }

    #[test]
    fn tp1_fires_at_1_1x_sells_40pct() {
        let (fills, close) = evaluate_staged_exits(&trade(), &snapshot(1.15, false), &config());
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].0, "tp1");
        assert!((fills[0].1 - 200_000.0).abs() < 1.0); // 40% of 500k
        assert!(close.is_none());
    }

    #[test]
    fn stop_before_tp1_dumps_everything() {
        let (fills, close) = evaluate_staged_exits(&trade(), &snapshot(0.60, false), &config());
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].0, "stop");
        assert_eq!(close, Some(ExitReason::StopLoss));
    }

    #[test]
    fn tp1_then_breakeven_stop_closes_rest() {
        let mut t = trade();
        t.tp1_done = true;
        t.remaining_tokens = 300_000.0;
        // price fell back to entry: breakeven stop
        let (fills, close) = evaluate_staged_exits(&t, &snapshot(1.0, false), &config());
        assert_eq!(fills[0].0, "stop");
        assert_eq!(close, Some(ExitReason::StopLoss));
    }

    #[test]
    fn tp1_then_tp2_sells_40pct_of_remaining() {
        let mut t = trade();
        t.tp1_done = true;
        t.remaining_tokens = 300_000.0;
        let (fills, close) = evaluate_staged_exits(&t, &snapshot(1.6, false), &config());
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].0, "tp2");
        assert!((fills[0].1 - 120_000.0).abs() < 1.0); // 40% of 300k
        assert!(close.is_none());
    }

    #[test]
    fn moon_bag_trailing_stop_closes() {
        let mut t = trade();
        t.tp1_done = true;
        t.tp2_done = true;
        t.remaining_tokens = 100_000.0;
        t.peak_price_sol = t.entry_price_sol * 3.0;
        // trailing 35% off peak: 3.0 * 0.65 = 1.95x -> price at 1.9x triggers
        let (fills, close) = evaluate_staged_exits(&t, &snapshot(1.9, false), &config());
        assert_eq!(fills[0].0, "stop");
        assert_eq!(close, Some(ExitReason::StopLoss));
    }

    #[test]
    fn moon_bag_holds_while_rising() {
        let mut t = trade();
        t.tp1_done = true;
        t.tp2_done = true;
        t.remaining_tokens = 100_000.0;
        t.peak_price_sol = t.entry_price_sol * 3.0;
        // above trail level (1.95x), below new peak -> hold
        let (_, close) = evaluate_staged_exits(&t, &snapshot(2.5, false), &config());
        assert!(close.is_none());
    }

    #[test]
    fn graduation_dumps_all() {
        let (fills, close) = evaluate_staged_exits(&trade(), &snapshot(1.05, true), &config());
        assert_eq!(fills[0].0, "graduated");
        assert_eq!(close, Some(ExitReason::Graduated));
        assert!((fills[0].1 - 500_000.0).abs() < 1.0);
    }

    #[test]
    fn config_rejects_non_moonbag_split() {
        let mut c = config();
        c.tp1_fraction = 60.0;
        c.tp2_fraction = 40.0; // 100% total -> no moon bag -> invalid
        assert!(ShadowConfig::validate(&c).is_err());
    }
}
