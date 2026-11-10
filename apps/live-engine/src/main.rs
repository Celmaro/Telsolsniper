//! Telsolsniper live engine — real trades at tiny size (0.001 SOL).
//!
//! Consumes deduped NewMint events from the intake fanout socket (same
//! stream as the shadow engine), buys on Pump.fun curve or PumpSwap,
//! then runs the same staged exit ladder as the shadow engine:
//! TP1 -> TP2 -> moon bag trailing stop, SL, breakeven after TP1.
//!
//! SAFETY: requires LIVE_ENABLED=true, a funded wallet in PRIVATE_KEY,
//! and refuses to start if wallet balance < 2 x per-trade size + rent.

mod journal;
mod redact;

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use serde::Deserialize;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{pubkey::Pubkey, signer::Signer};
use sol_trade_sdk::{
    common::{GasFeeStrategy, SolanaRpcClient, TradeConfig},
    swqos::SwqosConfig,
    trading::{
        core::params::{DexParamEnum, PumpFunParams, PumpSwapParams},
        factory::DexType,
    },
    SolanaTrade, TradeTokenType,
};
use sol_trade_sdk::swqos::common::TradeError;
use sol_trade_sdk::swqos::SwqosType;
use solana_sdk::signature::Signature;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

const TOKEN_DECIMALS: f64 = 1_000_000.0; // pump tokens: 6 dp
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect::<String>().replace('\n', " ")
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewMintEvent {
    source: String,
    message_id: i64,
    mint: String,
    ticker: Option<String>,
}

struct LiveConfig {
    event_socket_path: String,
    database_path: String,
    rpc_url: String,
    private_key_env: String,
    /// Held only to scrub the value out of any error/log text.
    private_key: String,
    buy_sol: f64,
    slippage_bps: u64,
    exit_slippage_bps: u64,
    /// Tip per transaction in SOL. At 0.001 SOL trades a 0.001 tip doubles
    /// the cost — keep it tiny.
    tip_sol: f64,
    cu_price_micro_lamports: u32,
    max_cu_price: u64,
    cu_limit: u32,
    /// Exit near entry if TP1 has not fired within this many seconds (0 = off).
    time_stop_secs: u64,
    /// The time-stop only fires below this multiple; above it, let the ladder run.
    time_stop_min_x: f64,
    tp1_multiple: f64,
    tp1_fraction: f64,
    tp2_multiple: f64,
    tp2_fraction: f64,
    moon_trail_pct: f64,
    initial_stop_pct: f64,
    breakeven_after_tp1: bool,
    poll_interval_secs: u64,
    failure_cooldown_secs: i64,
    reconcile_interval_secs: u64,
    entry_tick_ms: u64,
    entry_timeout_ms: u64,
    max_open_positions: usize,
}

impl LiveConfig {
    fn from_env() -> Result<Self> {
        let f = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
        let buy_sol = f("LIVE_BUY_SOL", "0.001").parse()?;
        let cfg = Self {
            event_socket_path: f("EVENT_SOCKET_PATH", "/tmp/telsolsniper-events.sock"),
            database_path: f("LIVE_DATABASE_PATH", "./data/live.sqlite"),
            rpc_url: f("RPC_URL", "https://api.mainnet-beta.solana.com"),
            private_key_env: f("PRIVATE_KEY_ENV", "PRIVATE_KEY"),
            private_key: std::env::var("PRIVATE_KEY").unwrap_or_default(),
            buy_sol,
            slippage_bps: f("LIVE_SLIPPAGE_BPS", "1000").parse()?, // 10%: trench tokens move
            // Exits must land even in a dump: 40% tolerance is cheaper than a
            // reverted stop, which realises the full loss plus fees.
            exit_slippage_bps: f("LIVE_EXIT_SLIPPAGE_BPS", "4000").parse()?,
            tip_sol: f("LIVE_TIP_SOL", "0.00002").parse()?,
            cu_price_micro_lamports: f("LIVE_CU_PRICE", "150000").parse()?,
            max_cu_price: f("LIVE_MAX_CU_PRICE", "2000000").parse()?,
            // Measured on chain: buys use 78.9k median / 116.9k max CU, sells
            // 76.1k / 84.9k. Asking for 500k made the priority fee 6x larger
            // than necessary (75k -> ~19.5k lamports per tx).
            cu_limit: f("LIVE_CU_LIMIT", "130000").parse()?,
            // Measured on 71 live trades: 68% never reach TP1. Sitting in them
            // until the -15% stop is the biggest single leak.
            time_stop_secs: f("LIVE_TIME_STOP_SECS", "90").parse()?,
            time_stop_min_x: f("LIVE_TIME_STOP_MIN_X", "1.2").parse()?,
            tp1_multiple: f("LIVE_TP1_X", "1.1").parse()?,
            tp1_fraction: f("LIVE_TP1_SELL_PCT", "40").parse()?,
            tp2_multiple: f("LIVE_TP2_X", "1.5").parse()?,
            tp2_fraction: f("LIVE_TP2_SELL_PCT", "40").parse()?,
            moon_trail_pct: f("LIVE_MOON_TRAIL_PCT", "35").parse()?,
            initial_stop_pct: f("LIVE_STOP_LOSS_PCT", "35").parse()?,
            breakeven_after_tp1: f("LIVE_BREAKEVEN_AFTER_TP1", "true").eq_ignore_ascii_case("true"),
            poll_interval_secs: f("LIVE_POLL_SECS", "2").parse()?,
            failure_cooldown_secs: f("LIVE_FAILURE_COOLDOWN_SECS", "60").parse()?,
            reconcile_interval_secs: f("LIVE_RECONCILE_SECS", "30").parse()?,
            entry_tick_ms: f("LIVE_ENTRY_TICK_MS", "250").parse()?,
            entry_timeout_ms: f("LIVE_ENTRY_TIMEOUT_MS", "2500").parse()?,
            max_open_positions: f("LIVE_MAX_OPEN", "5").parse()?,
        };
        // TP2 is optional: LIVE_TP2_X=0 selects single-TP mode, where the
        // remainder becomes the moon bag.
        let two_tps = cfg.tp2_multiple > 0.0;
        let sold = if two_tps { cfg.tp1_fraction + cfg.tp2_fraction } else { cfg.tp1_fraction };
        if !(cfg.tp1_multiple > 1.0)
            || !(cfg.tp1_fraction > 0.0 && cfg.tp1_fraction <= 100.0)
            || sold >= 100.0
            || (two_tps && (cfg.tp2_multiple <= cfg.tp1_multiple || cfg.tp2_fraction <= 0.0))
        {
            bail!(
                "invalid ladder config: tp1_x={} tp1_pct={} tp2_x={} tp2_pct={} (tp2_x=0 means single TP)",
                cfg.tp1_multiple, cfg.tp1_fraction, cfg.tp2_multiple, cfg.tp2_fraction
            );
        }
        Ok(cfg)
    }
}

struct LiveStore {
    /// Mutex so entry, exit and reconciliation tasks can share the store.
    connection: std::sync::Mutex<Connection>,
    /// Per-trade cooldown after a failed transaction: every retry costs
    /// fees, so never hammer a failing sell on every poll tick.
    cooldowns: std::sync::Mutex<std::collections::HashMap<i64, i64>>,
    zero_reads: std::sync::Mutex<std::collections::HashMap<i64, u8>>,
}

#[derive(Debug, Clone)]
struct LiveTrade {
    id: i64,
    mint: String,
    ticker: Option<String>,
    source: String,
    token_amount: f64,
    entry_price_sol: f64,
    peak_price_sol: f64,
    tp1_done: bool,
    tp2_done: bool,
    realized_pnl_sol: f64,
    remaining_tokens: f64,
    entry_time_unix_ms: i64,
}

impl LiveStore {
    fn open(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS live_trades (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mint TEXT NOT NULL UNIQUE,
                ticker TEXT,
                source TEXT NOT NULL,
                entry_time_unix_ms INTEGER NOT NULL,
                entry_price_sol REAL NOT NULL,
                buy_sol_amount REAL NOT NULL,
                token_amount REAL NOT NULL,
                cost_lamports INTEGER NOT NULL DEFAULT 0,
                entry_signature TEXT,
                peak_price_sol REAL,
                tp1_done INTEGER NOT NULL DEFAULT 0,
                tp2_done INTEGER NOT NULL DEFAULT 0,
                realized_pnl_sol REAL NOT NULL DEFAULT 0,
                remaining_tokens REAL NOT NULL DEFAULT 0,
                exit_time_unix_ms INTEGER,
                exit_reason TEXT,
                pnl_sol REAL,
                UNIQUE (mint)
            );
            CREATE TABLE IF NOT EXISTS live_fills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                trade_id INTEGER NOT NULL REFERENCES live_trades(id),
                fill_time_unix_ms INTEGER NOT NULL,
                kind TEXT NOT NULL,
                multiple REAL NOT NULL,
                price_sol REAL NOT NULL,
                tokens_sold REAL NOT NULL,
                proceeds_sol REAL NOT NULL,
                proceeds_lamports INTEGER NOT NULL DEFAULT 0,
                signature TEXT
            );
            "#,
        )?;
        // Migration: real-lamport accounting columns.
        let cols: Vec<String> = {
            let mut stmt = connection.prepare("PRAGMA table_info(live_trades)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if !cols.iter().any(|c| c == "cost_lamports") {
            connection.execute("ALTER TABLE live_trades ADD COLUMN cost_lamports INTEGER NOT NULL DEFAULT 0", [])?;
        }
        if !cols.iter().any(|c| c == "pending_exit_reason") {
            connection.execute("ALTER TABLE live_trades ADD COLUMN pending_exit_reason TEXT", [])?;
        }
        let fill_cols: Vec<String> = {
            let mut stmt = connection.prepare("PRAGMA table_info(live_fills)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if !fill_cols.iter().any(|c| c == "proceeds_lamports") {
            connection.execute("ALTER TABLE live_fills ADD COLUMN proceeds_lamports INTEGER NOT NULL DEFAULT 0", [])?;
        }
        journal::init(&connection)?;
        Ok(Self {
            connection: std::sync::Mutex::new(connection),
            cooldowns: std::sync::Mutex::new(std::collections::HashMap::new()),
            zero_reads: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    fn seen(&self, mint: &str) -> Result<bool> {
        let connection = self.connection.lock().unwrap();
        Ok(connection
            .query_row("SELECT 1 FROM live_trades WHERE mint = ?1", [mint], |_| Ok(()))
            .is_ok())
    }

    fn record_entry(&self, mint: &str, ticker: Option<&str>, source: &str, entry_price_sol: f64, buy_sol: f64, token_amount: f64, signature: &str, cost_lamports: i64) -> Result<i64> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            r#"INSERT INTO live_trades (mint, ticker, source, entry_time_unix_ms, entry_price_sol, buy_sol_amount, token_amount, cost_lamports, entry_signature, peak_price_sol)
               VALUES (?1, ?2, ?3, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), ?4, ?5, ?6, ?8, ?7, ?4)"#,
            rusqlite::params![mint, ticker, source, entry_price_sol, buy_sol, token_amount, signature, cost_lamports],
        )?;
        Ok(connection.last_insert_rowid())
    }

    fn open_trades(&self) -> Result<Vec<LiveTrade>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection.prepare(
            r#"SELECT id, mint, ticker, source, token_amount, entry_price_sol,
                      peak_price_sol, tp1_done, tp2_done, realized_pnl_sol, remaining_tokens,
                      entry_time_unix_ms
               FROM live_trades WHERE exit_time_unix_ms IS NULL"#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LiveTrade {
                id: row.get(0)?,
                mint: row.get(1)?,
                ticker: row.get(2)?,
                source: row.get(3)?,
                token_amount: row.get(4)?,
                entry_price_sol: row.get(5)?,
                peak_price_sol: row.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
                tp1_done: row.get::<_, i64>(7)? != 0,
                tp2_done: row.get::<_, i64>(8)? != 0,
                realized_pnl_sol: row.get(9)?,
                remaining_tokens: row.get(10)?,
                entry_time_unix_ms: row.get(11)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn tokens_at_entry(t: &LiveTrade) -> f64 {
        if t.remaining_tokens > 0.0 { t.remaining_tokens } else { t.token_amount }
    }

    fn record_fill(&self, trade_id: i64, kind: &str, multiple: f64, price_sol: f64, tokens_sold: f64, proceeds_lamports: i64, signature: Option<&str>) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        let proceeds_sol = proceeds_lamports as f64 / LAMPORTS_PER_SOL;
        connection.execute(
            r#"INSERT INTO live_fills (trade_id, fill_time_unix_ms, kind, multiple, price_sol, tokens_sold, proceeds_sol, proceeds_lamports, signature)
               VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), ?2, ?3, ?4, ?5, ?6, ?8, ?7)"#,
            rusqlite::params![trade_id, kind, multiple, price_sol, tokens_sold, proceeds_sol, signature, proceeds_lamports],
        )?;
        Ok(())
    }

    fn update_stage(&self, id: i64, kind: &str, realized: f64, remaining: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            r#"UPDATE live_trades
               SET realized_pnl_sol = ?2, remaining_tokens = ?3,
                   tp1_done = tp1_done | ?4, tp2_done = tp2_done | ?5
               WHERE id = ?1"#,
            rusqlite::params![id, realized, remaining, (kind == "tp1") as i64, (kind == "tp2") as i64],
        )?;
        Ok(())
    }

    fn update_peak(&self, id: i64, peak: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute("UPDATE live_trades SET peak_price_sol = ?2 WHERE id = ?1", rusqlite::params![id, peak])?;
        Ok(())
    }

    /// Mark the intent to exit. The trade stays open, with PnL unbooked, until
    /// its sells are confirmed on chain — booking at submit time made every
    /// trade look like a total loss (proceeds were still zero and the buy cost
    /// still carried the ATA rent).
    fn record_exit(&self, id: i64, exit_reason: &str, _pnl_sol: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            "UPDATE live_trades SET pending_exit_reason = ?2 WHERE id = ?1 AND pending_exit_reason IS NULL",
            rusqlite::params![id, exit_reason],
        )?;
        Ok(())
    }

    /// All journal orders for this trade are terminal (confirmed/failed).
    fn orders_settled(&self, mint: &str) -> Result<bool> {
        let connection = self.connection.lock().unwrap();
        let open: i64 = connection.query_row(
            "SELECT COUNT(*) FROM order_journal WHERE mint = ?1 AND state IN ('intended','submitted','unknown')",
            [mint],
            |r| r.get(0),
        )?;
        Ok(open == 0)
    }

    fn apply_fill_proceeds(&self, signature: &str, lamports: i64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            "UPDATE live_fills SET proceeds_lamports = ?2, proceeds_sol = ?3 WHERE signature = ?1",
            rusqlite::params![signature, lamports, lamports as f64 / LAMPORTS_PER_SOL],
        )?;
        Ok(())
    }

    fn finalize_exit(&self, id: i64, reason: &str, pnl_sol: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            r#"UPDATE live_trades
               SET exit_time_unix_ms = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER),
                   exit_reason = ?2, pnl_sol = ?3, pending_exit_reason = NULL
               WHERE id = ?1"#,
            rusqlite::params![id, reason, pnl_sol],
        )?;
        Ok(())
    }

    fn pending_exits(&self) -> Result<Vec<(i64, String, String)>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection.prepare(
            "SELECT id, mint, pending_exit_reason FROM live_trades WHERE pending_exit_reason IS NOT NULL AND exit_time_unix_ms IS NULL",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Count a consecutive zero-balance read; returns the running count.
    fn note_zero_read(&self, trade_id: i64) -> u8 {
        let mut map = self.zero_reads.lock().unwrap();
        let counter = map.entry(trade_id).or_insert(0);
        *counter = counter.saturating_add(1);
        *counter
    }

    fn clear_zero_reads(&self, trade_id: i64) {
        self.zero_reads.lock().unwrap().remove(&trade_id);
    }

    /// True if this trade may attempt a transaction now (not cooling down).
    fn can_attempt(&self, trade_id: i64) -> bool {
        let map = self.cooldowns.lock().unwrap();
        match map.get(&trade_id) {
            Some(&until) => now_ms() >= until,
            None => true,
        }
    }

    /// Block retries for `secs` after a failure (fee protection).
    fn mark_failure(&self, trade_id: i64, secs: i64) {
        let mut map = self.cooldowns.lock().unwrap();
        map.insert(trade_id, now_ms() + secs * 1000);
    }

    fn journal_intent(
        &self,
        client_order_id: &str,
        trade_id: Option<i64>,
        mint: &str,
        side: &str,
        kind: &str,
        lamports: i64,
        tokens: f64,
    ) -> Result<i64> {
        let connection = self.connection.lock().unwrap();
        journal::record_intent(
            &connection, client_order_id, trade_id, mint, side, kind, lamports, tokens, now_ms(),
        )
    }

    fn journal_submitted(&self, id: i64, signature: &str) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        journal::mark_submitted(&connection, id, signature, now_ms())
    }

    fn journal_confirmed(&self, id: i64, lamports: i64, tokens: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        journal::mark_confirmed(&connection, id, lamports, tokens, now_ms())
    }

    fn journal_failed(&self, id: i64, error: &str) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        journal::mark_failed(&connection, id, error, now_ms())
    }

    fn journal_unknown(&self, id: i64, note: &str) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        journal::mark_unknown(&connection, id, note, now_ms())
    }

    fn link_journal_trade(&self, journal_id: i64, trade_id: i64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            "UPDATE order_journal SET trade_id = ?2 WHERE id = ?1",
            rusqlite::params![journal_id, trade_id],
        )?;
        Ok(())
    }

    /// Age (seconds) of the confirmed entry order for this mint.
    fn entry_age_secs(&self, mint: &str) -> Result<i64> {
        let connection = self.connection.lock().unwrap();
        let created: Option<i64> = connection
            .query_row(
                "SELECT MIN(created_ms) FROM order_journal WHERE mint = ?1 AND side = 'buy' AND state = 'confirmed'",
                [mint],
                |r| r.get(0),
            )
            .unwrap_or(None);
        Ok(created.map(|ms| (now_ms() - ms) / 1000).unwrap_or(0))
    }

    /// True when the entry buy for this mint is confirmed on chain.
    fn entry_confirmed(&self, mint: &str) -> Result<bool> {
        let connection = self.connection.lock().unwrap();
        let buys: i64 = connection.query_row(
            "SELECT COUNT(*) FROM order_journal WHERE mint = ?1 AND side = 'buy'",
            [mint],
            |r| r.get(0),
        )?;
        if buys == 0 {
            // Trade predates the journal: nothing to wait for.
            return Ok(true);
        }
        let confirmed: i64 = connection.query_row(
            "SELECT COUNT(*) FROM order_journal WHERE mint = ?1 AND side = 'buy' AND state = 'confirmed'",
            [mint],
            |r| r.get(0),
        )?;
        Ok(confirmed > 0)
    }

    /// Reconciliation result for an entry: replace the estimate with the
    /// real cost and the real token balance once the buy is confirmed.
    fn apply_entry_confirmation(&self, mint: &str, cost_lamports: i64, tokens: f64) -> Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            r#"UPDATE live_trades
               SET cost_lamports = ?2,
                   token_amount = CASE WHEN ?3 > 0 THEN ?3 ELSE token_amount END,
                   remaining_tokens = CASE WHEN ?3 > 0 THEN ?3 ELSE remaining_tokens END
               WHERE mint = ?1 AND (cost_lamports = 0 OR cost_lamports != ?2)"#,
            rusqlite::params![mint, cost_lamports, tokens],
        )?;
        Ok(())
    }

    fn unsettled_orders(&self, min_age_ms: i64) -> Result<Vec<journal::JournalOrder>> {
        let connection = self.connection.lock().unwrap();
        journal::unsettled(&connection, min_age_ms, now_ms())
    }

    fn has_unresolved(&self, mint: &str) -> Result<bool> {
        let connection = self.connection.lock().unwrap();
        journal::has_unresolved(&connection, mint)
    }

    /// Net SOL PnL from the chain: real proceeds (fees/tips already netted)
    /// minus the real cost of the buy transaction.
    /// PnL from confirmed journal rows only. Falls back to the fill ledger
    /// for trades that predate the journal (legacy rows already reconciled).
    fn realized_pnl_sol(&self, trade_id: i64) -> Result<f64> {
        let connection = self.connection.lock().unwrap();
        let journal_proceeds = journal::confirmed_lamports(&connection, trade_id, "sell")?;
        let journal_cost = journal::confirmed_lamports(&connection, trade_id, "buy")?;
        if journal_proceeds != 0 || journal_cost != 0 {
            return Ok((journal_proceeds - journal_cost) as f64 / LAMPORTS_PER_SOL);
        }
        let proceeds: i64 = connection.query_row(
            "SELECT COALESCE(SUM(proceeds_lamports), 0) FROM live_fills WHERE trade_id = ?1",
            [trade_id], |r| r.get(0),
        )?;
        let cost: i64 = connection.query_row(
            "SELECT COALESCE(cost_lamports, 0) FROM live_trades WHERE id = ?1",
            [trade_id], |r| r.get(0),
        )?;
        Ok((proceeds - cost) as f64 / LAMPORTS_PER_SOL)
    }

    fn open_count(&self) -> Result<usize> {
        let connection = self.connection.lock().unwrap();
        Ok(connection.query_row("SELECT COUNT(*) FROM live_trades WHERE exit_time_unix_ms IS NULL", [], |r| r.get(0))?)
    }
}

async fn build_client(config: &LiveConfig) -> Result<(Arc<SolanaTrade>, Pubkey)> {
    let payer = sol_trade_sdk::common::keypair::load_keypair_from_env(&config.private_key_env)
        .context("PRIVATE_KEY (base58) env var required")?;
    let pubkey = payer.pubkey();
    let rpc_url = config.rpc_url.clone();
    let swqos_configs = vec![SwqosConfig::Default(rpc_url.clone())];
    let trade_config = TradeConfig::builder(rpc_url.clone(), swqos_configs, CommitmentConfig::processed())
        .create_wsol_ata_on_startup(true)
        .build();
    let client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    Ok((Arc::new(client), pubkey))
}

/// Resolved venue cache. The expensive path for graduated tokens is
/// `find_pool`'s getProgramAccounts fallback (3-26s); once we know the pool
/// address, re-reading it directly costs ~40ms. Cache per mint with TTL.
static VENUE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, (i64, Option<Pubkey>)>>,
> = std::sync::OnceLock::new();

fn venue_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, (i64, Option<Pubkey>)>> {
    VENUE_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn cached_pool(mint: &Pubkey) -> Option<Option<Pubkey>> {
    let cache = venue_cache().lock().unwrap();
    cache.get(&mint.to_string()).and_then(|(at, pool)| {
        if now_ms() - at < 900_000 {
            Some(*pool)
        } else {
            None
        }
    })
}

fn remember_pool(mint: &Pubkey, pool: Option<Pubkey>) {
    let mut cache = venue_cache().lock().unwrap();
    cache.insert(mint.to_string(), (now_ms(), pool));
}

fn config_venue_timeout_ms() -> u64 {
    std::env::var("LIVE_VENUE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2500)
}

/// Resolve a PumpSwap pool address from Dexscreener (~35ms) instead of
/// `find_pool`, whose getProgramAccounts fallback costs 6-26s on public RPC.
/// That difference is the whole entry window.
/// Canonical PumpSwap pool: pure PDA maths, no network. This is the fast path
/// for every graduated token (pumpswap.rs::get_canonical_pool_pda).
fn canonical_pool(mint: &Pubkey) -> Pubkey {
    sol_trade_sdk::instruction::utils::pumpswap::get_canonical_pool_pda(mint)
}

async fn dexscreener_pool(mint: &Pubkey) -> Option<Pubkey> {
    static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let http = HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1200))
            .build()
            .unwrap_or_default()
    });
    let url = format!("https://api.dexscreener.com/latest/dex/tokens/{mint}");
    let value: serde_json::Value = http.get(&url).send().await.ok()?.json().await.ok()?;
    let pairs = value.get("pairs")?.as_array()?;
    // Prefer the deepest pumpswap pair; dexscreener returns all venues.
    let mut best: Option<(f64, String)> = None;
    for pair in pairs {
        if pair.get("dexId").and_then(|d| d.as_str()) != Some("pumpswap") {
            continue;
        }
        let Some(address) = pair.get("pairAddress").and_then(|a| a.as_str()) else {
            continue;
        };
        let liquidity = pair
            .pointer("/liquidity/usd")
            .and_then(|l| l.as_f64())
            .unwrap_or(0.0);
        if best.as_ref().map(|(depth, _)| liquidity > *depth).unwrap_or(true) {
            best = Some((liquidity, address.to_string()));
        }
    }
    let (_, address) = best?;
    Pubkey::from_str(&address).ok()
}

async fn fetch_venue_params(
    client: &SolanaTrade,
    mint: &Pubkey,
) -> Result<(DexType, DexParamEnum, f64)> {
    let rpc = &client.infrastructure.rpc;
    // Try Pump.fun curve first, then PumpSwap for graduated tokens.
    if let Ok(params) = PumpFunParams::from_mint_by_rpc(rpc, mint).await {
        let curve = params.bonding_curve.as_ref();
        if !curve.complete {
            let price = (curve.virtual_sol_reserves as f64 / LAMPORTS_PER_SOL)
                / (curve.virtual_token_reserves as f64 / TOKEN_DECIMALS);
            return Ok((DexType::PumpFun, DexParamEnum::PumpFun(params), price));
        }
    }
    // Fast path: we already resolved this mint's pool — read it directly
    // instead of paying for find_pool's getProgramAccounts fallback.
    if let Some(Some(pool)) = cached_pool(mint) {
        if let Ok(params) = PumpSwapParams::from_pool_address_by_rpc(rpc, &pool).await {
            let base = params.pool_base_token_reserves as f64;
            let quote = params.pool_quote_token_reserves as f64 / LAMPORTS_PER_SOL;
            let price = if base > 0.0 { quote / (base / TOKEN_DECIMALS) } else { 0.0 };
            return Ok((DexType::PumpSwap, DexParamEnum::PumpSwap(params), price));
        }
    }
    // 1. Canonical pool by PDA — microseconds, verified by one account read.
    let canonical = canonical_pool(mint);
    if let Ok(params) = PumpSwapParams::from_pool_address_by_rpc(rpc, &canonical).await {
        remember_pool(mint, Some(canonical));
        let base = params.pool_base_token_reserves as f64;
        let quote = params.pool_quote_token_reserves as f64 / LAMPORTS_PER_SOL;
        let price = if base > 0.0 { quote / (base / TOKEN_DECIMALS) } else { 0.0 };
        return Ok((DexType::PumpSwap, DexParamEnum::PumpSwap(params), price));
    }
    // 2. Non-canonical pools (creator-fee / higher index): dexscreener index.
    if let Some(pool) = dexscreener_pool(mint).await {
        if let Ok(params) = PumpSwapParams::from_pool_address_by_rpc(rpc, &pool).await {
            remember_pool(mint, Some(pool));
            let base = params.pool_base_token_reserves as f64;
            let quote = params.pool_quote_token_reserves as f64 / LAMPORTS_PER_SOL;
            let price = if base > 0.0 { quote / (base / TOKEN_DECIMALS) } else { 0.0 };
            return Ok((DexType::PumpSwap, DexParamEnum::PumpSwap(params), price));
        }
    }
    let params = match tokio::time::timeout(
        std::time::Duration::from_millis(config_venue_timeout_ms()),
        PumpSwapParams::from_mint_by_rpc(rpc, mint),
    )
    .await
    {
        Ok(Ok(params)) => params,
        Ok(Err(error)) => anyhow::bail!("no supported venue: {error}"),
        Err(_) => anyhow::bail!("venue resolution timed out (getProgramAccounts fallback)"),
    };
    remember_pool(mint, Some(params.pool));
    // Price discovery via pool reserves is handled by the SDK on execution;
    // for sizing we reuse the shadow engine's approach: quote a tiny buy.
    let price = quote_pumpswap_price(rpc, mint).await?;
    Ok((DexType::PumpSwap, DexParamEnum::PumpSwap(params), price))
}

/// Real SOL balance change for a confirmed signature (lamports), read
/// straight from the chain. Accounting must never rely on modeled prices.
async fn wallet_delta(rpc_url: &str, signature: &str, wallet: &Pubkey) -> Result<i64> {
    #[derive(serde::Deserialize)]
    struct Res {
        result: Option<Tx>,
    }
    #[derive(serde::Deserialize)]
    struct Tx {
        meta: Option<Meta>,
        #[serde(rename = "transaction")]
        transaction: Transaction,
    }
    #[derive(serde::Deserialize)]
    struct Transaction {
        message: Message,
    }
    #[derive(serde::Deserialize)]
    struct Message {
        #[serde(rename = "accountKeys")]
        account_keys: Vec<Key>,
    }
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Key {
        Plain(String),
        Rich { pubkey: String },
    }
    impl Key {
        fn pk(&self) -> &str {
            match self {
                Key::Plain(s) => s,
                Key::Rich { pubkey } => pubkey,
            }
        }
    }
    #[derive(serde::Deserialize)]
    struct Meta {
        #[serde(rename = "preBalances")]
        pre_balances: Vec<u64>,
        #[serde(rename = "postBalances")]
        post_balances: Vec<u64>,
    }

    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getTransaction",
        "params": [signature, {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 0}]
    });
    let response: Res = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let tx = response.result.context("transaction not found")?;
    let meta = tx.meta.context("transaction has no meta")?;
    let target = wallet.to_string();
    let index = tx
        .transaction
        .message
        .account_keys
        .iter()
        .position(|k| k.pk() == target)
        .context("wallet not in transaction account keys")?;
    Ok(meta.post_balances[index] as i64 - meta.pre_balances[index] as i64)
}

/// Token balance of `wallet` for `mint`, read from chain (jsonParsed).
async fn wallet_token_balance(rpc_url: &str, mint: &Pubkey, wallet: &Pubkey) -> Result<f64> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getTokenAccountsByOwner",
        "params": [wallet.to_string(), {"mint": mint.to_string()}, {"encoding": "jsonParsed"}]
    });
    let value: serde_json::Value = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let accounts = value["result"]["value"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut total = 0.0f64;
    for account in accounts {
        let info = &account["account"]["data"]["parsed"]["info"]["tokenAmount"];
        // uiAmount is null for values that round to zero — prefer raw amount.
        let amount = info["uiAmount"].as_f64().or_else(|| {
            info["amount"]
                .as_str()
                .and_then(|raw| raw.parse::<f64>().ok())
                .map(|raw| raw / TOKEN_DECIMALS)
        });
        if let Some(amount) = amount {
            total += amount;
        }
    }
    Ok(total)
}

async fn quote_pumpswap_price(rpc: &SolanaRpcClient, mint: &Pubkey) -> Result<f64> {
    // Price polling must use the same fast PDA/cache path as entry; calling
    // from_mint_by_rpc on every 2s tick would reintroduce the slow scan.
    let params = if let Some(Some(pool)) = cached_pool(mint) {
        PumpSwapParams::from_pool_address_by_rpc(rpc, &pool).await?
    } else {
        let canonical = canonical_pool(mint);
        match PumpSwapParams::from_pool_address_by_rpc(rpc, &canonical).await {
            Ok(params) => {
                remember_pool(mint, Some(canonical));
                params
            }
            Err(_) => {
                let params = PumpSwapParams::from_mint_by_rpc(rpc, mint).await?;
                remember_pool(mint, Some(params.pool));
                params
            }
        }
    };
    let base_reserve = params.pool_base_token_reserves;
    let quote_reserve = params.pool_quote_token_reserves;
    Ok((quote_reserve as f64 / LAMPORTS_PER_SOL) / (base_reserve as f64 / TOKEN_DECIMALS))
}

/// Returns (effective price incl. all fees/rent, tokens received, signature, real cost in lamports).
/// GasFeeStrategy::set_global_fee_strategy only configures the premium
/// SWQOS transport tiers — it SKIPS SwqosType::Default. We run Default,
/// so set it explicitly: low tip (0.00002 SOL) + reasonable CU price.
/// Live priority-fee bid, refreshed from getRecentPrioritizationFees for the
/// accounts we actually contend on (a hot pool), per Solana's guidance that
/// fee markets are per-account, not per-network.
static FEE_BID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(150_000);

fn current_cu_price(config: &LiveConfig) -> u32 {
    let dynamic = FEE_BID.load(std::sync::atomic::Ordering::Relaxed);
    let value = dynamic.max(config.cu_price_micro_lamports as u64);
    value.min(config.max_cu_price) as u32
}

async fn fee_refresh_loop(config: Arc<LiveConfig>) {
    let http = reqwest::Client::new();
    loop {
        let account = venue_cache()
            .lock()
            .unwrap()
            .iter()
            .max_by_key(|(_, (at, _))| *at)
            .and_then(|(_, (_, pool))| *pool)
            .map(|pool| pool.to_string());
        if let Some(account) = account {
            let body = serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "getRecentPrioritizationFees",
                "params": [[account]]
            });
            if let Ok(value) = http.post(&config.rpc_url).json(&body).send().await {
                if let Ok(json) = value.json::<serde_json::Value>().await {
                    let mut samples: Vec<u64> = json["result"]
                        .as_array()
                        .map(|rows| {
                            rows.iter()
                                .filter_map(|r| r["prioritizationFee"].as_u64())
                                .collect()
                        })
                        .unwrap_or_default();
                    samples.sort_unstable();
                    // p75 of recent slots: enough to outbid the crowd without
                    // paying the tail; zero-fee slots are kept (they mean no
                    // contention, not that the fee is free).
                    if !samples.is_empty() {
                        let p75 = samples[(samples.len() * 3) / 4];
                        let bid = p75.max(config.cu_price_micro_lamports as u64).min(config.max_cu_price);
                        FEE_BID.store(bid, std::sync::atomic::Ordering::Relaxed);
                        tracing::debug!(bid, samples = samples.len(), "priority fee bid refreshed");
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    }
}

fn build_gas(config: &LiveConfig) -> GasFeeStrategy {
    use sol_trade_sdk::swqos::{SwqosType, TradeType};
    use sol_trade_sdk::common::GasFeeStrategyType;
    let gas = GasFeeStrategy::new();
    gas.set(
        SwqosType::Default,
        TradeType::Buy,
        GasFeeStrategyType::Normal,
        config.cu_limit,
        current_cu_price(config) as u64,
        config.tip_sol,
    );
    gas.set(
        SwqosType::Default,
        TradeType::Sell,
        GasFeeStrategyType::Normal,
        config.cu_limit,
        current_cu_price(config) as u64,
        config.tip_sol,
    );
    gas
}

/// Returns (market price, tokens received, signature, verified cost lamports,
/// journal id) so the caller can link the order to the trade row.
/// The intent is journaled before submit; if the chain cannot confirm the
/// result, the order is marked `unknown` and the caller must not book PnL.
async fn live_buy(
    client: &SolanaTrade,
    config: &LiveConfig,
    store: &LiveStore,
    mint: Pubkey,
) -> Result<(f64, f64, String, i64, i64)> {
    let (dex_type, extension_params, price_sol) = fetch_venue_params(client, &mint).await?;
    let lamports = (config.buy_sol * LAMPORTS_PER_SOL) as u64;
    let client_order_id = format!("{mint}-buy-{}", now_ms());
    let journal_id = store.journal_intent(
        &client_order_id,
        None,
        &mint.to_string(),
        "buy",
        "entry",
        lamports as i64,
        0.0,
    )?;
    let blockhash = cached_blockhash(client).await?;
    let gas = build_gas(config);

    let params = sol_trade_sdk::TradeBuyParams {
        dex_type,
        input_token_type: TradeTokenType::SOL,
        mint,
        input_token_amount: lamports,
        slippage_basis_points: Some(config.slippage_bps),
        recent_blockhash: Some(blockhash),
        extension_params,
        address_lookup_table_accounts: Vec::new(),
        // Do NOT block on confirmation: submit returns in ~30ms and the
        // reconciler resolves the fill. Blocking here cost p50 1.1s / p95 9s.
        wait_tx_confirmed: false,
        wait_for_all_submits: false,
        // PumpSwap quotes in WSOL: the pool pulls from the user's WSOL ATA,
        // so the wrap instruction must be included or the SPL transfer fails
        // with "insufficient funds" (custom 1). Pump.fun buys in native SOL.
        create_input_token_ata: matches!(dex_type, DexType::PumpSwap),
        close_input_token_ata: false,
        create_mint_ata: true,
        durable_nonce: None,
        fixed_output_token_amount: None,
        gas_fee_strategy: gas,
        simulate: false,
        use_exact_sol_amount: Some(true),
        grpc_recv_us: None,
    };
    // Submit with one bounded retry on transient failures (submit-window
    // timeouts, dropped connections). On-chain rejects are terminal.
    let attempt = send_buy_with_retry(client, config, params, 1).await;
    let (ok, signatures, error, _) = match attempt {
        Ok(value) => value,
        Err(error) => {
            // No signature came back, so no transaction can be reconciled.
            // Classify: unsupported token-quote venues are terminal for now.
            let text = redact::redact_error(&error.to_string(), &[config.private_key.as_str()]);
            let unsupported = text.contains("PumpFunV3Params")
                || text.contains("PumpFunQuoteRoute")
                || text.contains("do not match the supplied pool")
                || text.contains("active non-cashback curve");
            let note = if unsupported {
                format!("unsupported venue (token-quote pool): {}", clip(&text, 160))
            } else {
                format!("not submitted: {}", clip(&text, 160))
            };
            store.journal_failed(journal_id, &note)?;
            return Err(error);
        }
    };
    if !ok {
        let text = redact::redact_error(&format!("{error:?}"), &[config.private_key.as_str()]);
        if text.contains("InstructionError") || text.contains("insufficient") {
            store.journal_failed(journal_id, &text)?;
        } else {
            // Transient: the first submission may still land, so leave the
            // journal as intended for the reconciler to resolve.
            store.journal_unknown(journal_id, &format!("submit returned not-ok: {}", clip(&text, 160)))?;
        }
        bail!("buy failed: {:?}", error);
    }
    let signature = signatures
        .first()
        .map(|s| s.to_string())
        .context("buy returned no signature")?;
    store.journal_submitted(journal_id, &signature)?;
    // Hot path ends here: the reconciler confirms the transaction and
    // records the real cost and token amount a moment later.
    let cost_lamports = (config.buy_sol * LAMPORTS_PER_SOL) as i64;
    // Estimated until reconciliation reads the real balance.
    let token_amount = config.buy_sol / price_sol * 0.97;
    // entry_price stays the market (curve/pool) price so TP/SL multiples
    // measure market movement; cost_lamports carries the all-in real cost.
    Ok((price_sol, token_amount, signature, cost_lamports, journal_id))
}

/// Submit a buy, retrying up to `retries` times when the failure is clearly
/// transport-level (no signature, submit window expired). Never retries an
/// on-chain rejection — that is deterministic and would burn fees.
async fn send_buy_with_retry(
    client: &SolanaTrade,
    _config: &LiveConfig,
    mut params: sol_trade_sdk::TradeBuyParams,
    retries: u32,
) -> Result<(bool, Vec<Signature>, Option<TradeError>, Vec<(SwqosType, i64)>)> {
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 0..=retries {
        if attempt > 0 {
            // A fresh blockhash for the retry; the old one may be near expiry.
            let blockhash = match client.infrastructure.rpc.get_latest_blockhash().await {
                Ok(blockhash) => blockhash,
                Err(error) => return Err(error.into()),
            };
            params.recent_blockhash = Some(blockhash);
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        match client.buy(params.clone()).await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let text = error.to_string();
                if text.contains("InstructionError") || text.contains("insufficient") {
                    return Err(error.into());
                }
                last_error = Some(error);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("buy failed")))
}

/// Cached recent blockhash. A blockhash costs one RPC round trip (~20-40ms) and
/// stays valid ~60s, so the hot path reads a value that a background task keeps
/// fresh instead of paying for it inside the entry/exit window.
static BLOCKHASH: std::sync::OnceLock<parking_lot_shim::RwLock<(Option<solana_sdk::hash::Hash>, i64)>> =
    std::sync::OnceLock::new();

mod parking_lot_shim {
    // Minimal replacement so we avoid a new dependency.
    pub struct RwLock<T>(std::sync::RwLock<T>);
    impl<T> RwLock<T> {
        pub fn new(value: T) -> Self {
            Self(std::sync::RwLock::new(value))
        }
        pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
            self.0.read().unwrap_or_else(|e| e.into_inner())
        }
        pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
            self.0.write().unwrap_or_else(|e| e.into_inner())
        }
    }
}

fn blockhash_store() -> &'static parking_lot_shim::RwLock<(Option<solana_sdk::hash::Hash>, i64)> {
    BLOCKHASH.get_or_init(|| parking_lot_shim::RwLock::new((None, 0)))
}

/// Fresh enough to submit: 10s keeps it well inside the ~60s validity window.
async fn cached_blockhash(client: &SolanaTrade) -> Result<solana_sdk::hash::Hash> {
    {
        let guard = blockhash_store().read();
        if let (Some(hash), at) = *guard {
            if now_ms() - at < 10_000 {
                return Ok(hash);
            }
        }
    }
    let hash = client.infrastructure.rpc.get_latest_blockhash().await?;
    *blockhash_store().write() = (Some(hash), now_ms());
    Ok(hash)
}

async fn blockhash_refresh_loop(client: Arc<SolanaTrade>) {
    loop {
        match client.infrastructure.rpc.get_latest_blockhash().await {
            Ok(hash) => *blockhash_store().write() = (Some(hash), now_ms()),
            Err(error) => tracing::debug!(
                error = %redact::redact_error(&error.to_string(), &[]),
                "blockhash refresh failed"
            ),
        }
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    }
}

async fn send_sell_with_retry(
    client: &SolanaTrade,
    mut params: sol_trade_sdk::TradeSellParams,
    retries: u32,
) -> Result<(bool, Vec<Signature>, Option<TradeError>, Vec<(SwqosType, i64)>)> {
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 0..=retries {
        if attempt > 0 {
            params.recent_blockhash = Some(cached_blockhash(client).await?);
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        match client.sell(params.clone()).await {
            Ok(value) => {
                // A transport timeout ("No SWQOS result within submit result
                // window") can still have landed; only a returned not-ok with
                // an instruction error is terminal.
                let (ok, _, ref error, _) = value;
                let terminal = error
                    .as_ref()
                    .map(|e| {
                        let text = format!("{e:?}");
                        text.contains("InstructionError") || text.contains("insufficient")
                    })
                    .unwrap_or(false);
                if ok || terminal || attempt == retries {
                    return Ok(value);
                }
                let text = format!("{error:?}");
                last_error = Some(anyhow::anyhow!("{text}"));
            }
            Err(error) => {
                let text = error.to_string();
                if text.contains("InstructionError") || text.contains("insufficient") {
                    return Err(error.into());
                }
                last_error = Some(error.into());
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("sell failed")))
}

async fn live_sell(
    client: &SolanaTrade,
    config: &LiveConfig,
    store: &LiveStore,
    trade_id: i64,
    kind: &str,
    mint: Pubkey,
    token_amount: f64,
    close_ata: bool,
) -> Result<(f64, String, i64)> {
    let (dex_type, extension_params, price_sol) = fetch_venue_params(client, &mint).await?;
    let raw_amount = (token_amount * TOKEN_DECIMALS) as u64;
    let client_order_id = format!("{mint}-{kind}-{}", now_ms());
    let journal_id = store.journal_intent(
        &client_order_id,
        Some(trade_id),
        &mint.to_string(),
        "sell",
        kind,
        0,
        token_amount,
    )?;
    let blockhash = cached_blockhash(client).await?;
    let gas = build_gas(config);

    // Exits prioritise landing over price: a stop that reverts because the
    // price gapped past 10% slippage is a total loss, not a save.
    let exit_slippage = config.exit_slippage_bps.max(config.slippage_bps);

    let params = sol_trade_sdk::TradeSellParams {
        dex_type,
        output_token_type: TradeTokenType::SOL,
        mint,
        input_token_amount: raw_amount,
        slippage_basis_points: Some(exit_slippage),
        recent_blockhash: Some(blockhash),
        extension_params,
        address_lookup_table_accounts: Vec::new(),
        // Same rule as entries: return after submit, let the reconciler
        // confirm. Blocking here cost 1-11s per exit.
        wait_tx_confirmed: false,
        wait_for_all_submits: false,
        create_output_token_ata: false,
        // Never close the wSOL ATA: a residual balance makes close_account
        // fail with "Non-native account can only be closed if its balance
        // is zero". The token ATA close is where the rent is reclaimed.
        close_output_token_ata: false,
        close_mint_token_ata: close_ata,
        with_tip: false,
        durable_nonce: None,
        fixed_output_token_amount: None,
        gas_fee_strategy: gas,
        simulate: false,
        grpc_recv_us: None,
    };
    let sent = send_sell_with_retry(client, params.clone(), 1).await;
    let (ok, signatures, error, _) = match sent {
        Ok(value) => value,
        Err(error) => {
            let text = redact::redact_error(&error.to_string(), &[config.private_key.as_str()]);
            store.journal_unknown(journal_id, &text)?;
            return Err(error);
        }
    };
    if !ok {
        let text = redact::redact_error(&format!("{error:?}"), &[config.private_key.as_str()]);
        // SPL Token 11 = NonNativeHasBalance: the ATA still held dust, so
        // close_mint_token_ata made the whole sell revert. Retry the same
        // amount without the close — landing the exit matters more than rent.
        let dust_close = text.contains("Custom\":11") || text.contains("Custom\": 11");
        if dust_close && close_ata {
            let mut retry_params = params.clone();
            retry_params.close_mint_token_ata = false;
            if let Ok((ok2, sigs2, err2, _)) = send_sell_with_retry(client, retry_params, 0).await {
                if ok2 {
                    if let Some(sig) = sigs2.first() {
                        store.journal_submitted(journal_id, &sig.to_string())?;
                        warn!(mint = %mint.to_string(), "sell landed without ATA close (dust left; rent forfeited)");
                        return Ok((price_sol, sig.to_string(), 0));
                    }
                }
                let text2 = redact::redact_error(&format!("{err2:?}"), &[config.private_key.as_str()]);
                store.journal_failed(journal_id, &text2)?;
                bail!("sell failed after dust retry: {text2}");
            }
        }
        store.journal_failed(journal_id, &text)?;
        bail!("sell failed: {:?}", error);
    }
    let signature = signatures
        .first()
        .map(|s| s.to_string())
        .context("sell returned no signature")?;
    store.journal_submitted(journal_id, &signature)?;
    // Non-blocking exit: the reconciler confirms the sell and writes the real
    // proceeds into the journal, which is what realized_pnl_sol reads. Verifying
    // inline cost 1-11s and wrongly marked fresh sells "unknown" when the node
    // had not indexed them yet.
    Ok((price_sol, signature, 0))
}

async fn poll_live_position(
    client: &Arc<SolanaTrade>,
    store: &LiveStore,
    config: &LiveConfig,
    trade: LiveTrade,
) -> Result<()> {
    let Ok(mint) = Pubkey::from_str(&trade.mint) else { return Ok(()) };
    let rpc = &client.infrastructure.rpc;

    // Price is always read from the pool over RPC: the stream's priceSol
    // measures 7-22% above the true token-account ratio, so it is used only
    // to trigger this call earlier (see price_feed::last_tick), never as the
    // price itself.
    let price = match PumpFunParams::from_mint_by_rpc(rpc, &mint).await {
        Ok(p) if !p.bonding_curve.as_ref().complete => {
            let c = p.bonding_curve.as_ref();
            (c.virtual_sol_reserves as f64 / LAMPORTS_PER_SOL)
                / (c.virtual_token_reserves as f64 / TOKEN_DECIMALS)
        }
        _ => match quote_pumpswap_price(rpc, &mint).await {
            Ok(p) => p,
            Err(error) => {
                warn!(mint = %trade.mint, %error, "price unavailable; skipping tick");
                return Ok(());
            }
        },
    };

    if price > trade.peak_price_sol {
        store.update_peak(trade.id, price)?;
    }
    let peak = trade.peak_price_sol.max(price);
    let multiple = price / trade.entry_price_sol;
    // Always sell against the real on-chain balance: partial sells round to
    // integers and leave dust, and any dust breaks close_account.
    let wallet = client.payer.pubkey();
    let tokens_now = match wallet_token_balance(&config.rpc_url, &mint, &wallet).await {
        Ok(balance) if balance > 0.0 => {
            store.clear_zero_reads(trade.id);
            balance
        }
        Ok(_) => {
            // RPC indexing lags right after a buy: require 3 consecutive
            // zero reads (>=15s) before declaring the position empty.
            let zeros = store.note_zero_read(trade.id);
            if zeros >= 3 {
                // A position may only be declared empty when its entry is
                // confirmed on chain and old enough that lag is implausible.
                if !store.entry_confirmed(&trade.mint)? {
                    tracing::debug!(mint = %trade.mint, "zero balance but entry unconfirmed; waiting");
                    return Ok(());
                }
                let age_secs = store.entry_age_secs(&trade.mint)?;
                if age_secs < 45 {
                    tracing::debug!(mint = %trade.mint, age_secs, "zero balance just after entry; waiting");
                    return Ok(());
                }
                warn!(mint = %trade.mint, age_secs, "no tokens on chain; marking closed");
                store.record_exit(trade.id, "empty", store.realized_pnl_sol(trade.id)?)?;
            }
            return Ok(());
        }
        Err(error) => {
            warn!(mint = %trade.mint, %error, "balance read failed; using recorded size");
            LiveStore::tokens_at_entry(&trade)
        }
    };

    // PnL SAFEGUARD: if any order on this mint is unresolved, the position's
    // ledger is incomplete — an in-flight sell may already have executed.
    // Do not trade it until reconciliation settles the journal.
    if store.has_unresolved(&trade.mint)? {
        warn!(mint = %trade.mint, "unresolved journal order; pausing exits until reconciled");
        return Ok(());
    }

    if !store.can_attempt(trade.id) {
        return Ok(());
    }

    // Ladder (same rules as shadow).
    if !trade.tp1_done && multiple >= config.tp1_multiple {
        let tokens = tokens_now * config.tp1_fraction / 100.0;
        match live_sell(client, config, store, trade.id, "tp1", mint, tokens, false).await {
            Ok((fill_price, sig, proceeds_lamports)) => {
                store.record_fill(trade.id, "tp1", fill_price / trade.entry_price_sol, fill_price, tokens, proceeds_lamports, Some(&sig))?;
                let realized = store.realized_pnl_sol(trade.id)?;
                let remaining = tokens_now - tokens;
                store.update_stage(trade.id, "tp1", realized, remaining)?;
                info!(mint = %trade.mint, sig = %sig, proceeds_sol = %(proceeds_lamports as f64 / LAMPORTS_PER_SOL), "LIVE tp1 filled");
            }
            Err(error) => {
                store.mark_failure(trade.id, config.failure_cooldown_secs);
                error!(
                    mint = %trade.mint,
                    error = %redact::redact_error(&error.to_string(), &[config.private_key.as_str()]),
                    "LIVE tp1 sell failed"
                );
            }
        }
        return Ok(()); // one action per tick
    }
    if config.tp2_multiple > 0.0 && trade.tp1_done && !trade.tp2_done && multiple >= config.tp2_multiple {
        let tokens = tokens_now * config.tp2_fraction / 100.0;
        match live_sell(client, config, store, trade.id, "tp2", mint, tokens, false).await {
            Ok((fill_price, sig, proceeds_lamports)) => {
                store.record_fill(trade.id, "tp2", fill_price / trade.entry_price_sol, fill_price, tokens, proceeds_lamports, Some(&sig))?;
                let realized = store.realized_pnl_sol(trade.id)?;
                let remaining = tokens_now - tokens;
                store.update_stage(trade.id, "tp2", realized, remaining)?;
                info!(mint = %trade.mint, sig = %sig, proceeds_sol = %(proceeds_lamports as f64 / LAMPORTS_PER_SOL), "LIVE tp2 filled");
            }
            Err(error) => {
                store.mark_failure(trade.id, config.failure_cooldown_secs);
                error!(
                    mint = %trade.mint,
                    error = %redact::redact_error(&error.to_string(), &[config.private_key.as_str()]),
                    "LIVE tp2 sell failed"
                );
            }
        }
        return Ok(());
    }

    let trailing_enabled = if trade.tp1_done {
        // Single-TP mode (tp2 disabled): the whole remainder is the moon bag
        // and trails the peak immediately. With TP2 enabled, trailing starts
        // once TP2 has fired.
        config.tp2_multiple <= 0.0 || trade.tp2_done
    } else {
        false
    };
    // Time-stop: 68% of these signals never reach TP1 and just bleed to the
    // stop. If TP1 is not hit within the window, exit near entry and free the
    // slot. Measured: worth +64% expectancy (expectancy_real.py ladder E).
    let timed_out = !trade.tp1_done
        && config.time_stop_secs > 0
        && (now_ms() - trade.entry_time_unix_ms) / 1000 >= config.time_stop_secs as i64
        && multiple < config.time_stop_min_x;

    let stop_hit = timed_out
        || if !trade.tp1_done {
            multiple <= 1.0 - config.initial_stop_pct / 100.0
        } else if config.tp2_multiple > 0.0 && !trade.tp2_done {
            config.breakeven_after_tp1 && multiple <= 1.0
        } else if trailing_enabled {
            price <= peak * (1.0 - config.moon_trail_pct / 100.0)
        } else {
            false
        };
    if stop_hit && tokens_now > 1.0 {
        let kind = if timed_out {
            "time_stop"
        } else if trade.tp1_done {
            "moon_bag"
        } else {
            "stop"
        };
        match live_sell(client, config, store, trade.id, kind, mint, tokens_now, true).await {
            Ok((fill_price, sig, proceeds_lamports)) => {
                store.record_fill(trade.id, kind, fill_price / trade.entry_price_sol, fill_price, tokens_now, proceeds_lamports, Some(&sig))?;
                let realized = store.realized_pnl_sol(trade.id)?;
                store.update_stage(trade.id, kind, realized, 0.0)?;
                store.record_exit(trade.id, kind, realized)?;
                info!(mint = %trade.mint, pnl_sol = %realized, "LIVE exit");
            }
            Err(error) => {
                store.mark_failure(trade.id, config.failure_cooldown_secs);
                error!(
                    mint = %trade.mint,
                    error = %redact::redact_error(&error.to_string(), &[config.private_key.as_str()]),
                    "LIVE stop sell failed"
                );
            }
        }
    }
    Ok(())
}

/// Persistent event reader: one long-lived socket connection, parsing
/// every line as it arrives. Restarts the connection on failure.
fn spawn_event_reader(
    path: String,
    tx: tokio::sync::mpsc::UnboundedSender<NewMintEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        loop {
            match tokio::net::UnixStream::connect(&path).await {
                Ok(stream) => {
                    info!(path = %path, "event stream connected");
                    let mut lines = tokio::io::BufReader::new(stream).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        match serde_json::from_str::<NewMintEvent>(&line) {
                            Ok(event) => {
                                if tx.send(event).is_err() {
                                    return; // receiver gone
                                }
                            }
                            Err(error) => warn!(%error, "unparsable event"),
                        }
                    }
                    warn!("event stream closed; reconnecting");
                }
                Err(error) => warn!(%error, "event socket unavailable"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
}

/// Chain outcome for a signature: None when the node does not know it.
async fn tx_outcome(
    rpc_url: &str,
    signature: &str,
    wallet: &Pubkey,
) -> Result<Option<(bool, i64, String)>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getTransaction",
        "params": [signature, {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 0}]
    });
    let value: serde_json::Value = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let Some(result) = value.get("result").filter(|r| !r.is_null()) else {
        return Ok(None);
    };
    let meta = &result["meta"];
    let ok = meta["err"].is_null();
    let error_text = if ok { String::new() } else { meta["err"].to_string() };
    let keys = result["transaction"]["message"]["accountKeys"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let target = wallet.to_string();
    let mut delta = 0i64;
    for (index, key) in keys.iter().enumerate() {
        let pubkey = key
            .as_str()
            .map(str::to_string)
            .or_else(|| key.get("pubkey").and_then(|p| p.as_str()).map(str::to_string));
        if pubkey.as_deref() == Some(target.as_str()) {
            let pre = meta["preBalances"][index].as_i64().unwrap_or(0);
            let post = meta["postBalances"][index].as_i64().unwrap_or(0);
            delta = post - pre;
            break;
        }
    }
    Ok(Some((ok, delta, error_text)))
}

/// Reconciliation: resolve journal orders that never reported a final state.
/// Runs independently of trading so a stuck order cannot block entries.
async fn signature_status_batch(
    rpc_url: &str,
    signatures: &[String],
) -> Result<std::collections::HashMap<String, Option<(bool, Option<String>)>>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getSignatureStatuses",
        "params": [signatures, {"searchTransactionHistory": true}]
    });
    let value: serde_json::Value = reqwest::Client::new()
        .post(rpc_url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(4))
        .send()
        .await?
        .json()
        .await?;
    let values = value["result"]["value"]
        .as_array()
        .context("signature status response missing value")?;
    let mut result = std::collections::HashMap::new();
    for (signature, status) in signatures.iter().zip(values) {
        if status.is_null() {
            result.insert(signature.clone(), None);
        } else {
            let err = if status["err"].is_null() {
                None
            } else {
                Some(status["err"].to_string())
            };
            result.insert(signature.clone(), Some((err.is_none(), err)));
        }
    }
    Ok(result)
}

async fn reconcile_loop(
    store: Arc<LiveStore>,
    config: Arc<LiveConfig>,
    wallet: Pubkey,
) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(config.reconcile_interval_secs)).await;
        let orders = match store.unsettled_orders(8_000) {
            Ok(orders) => orders,
            Err(error) => {
                warn!(error = %redact::redact_error(&error.to_string(), &[]), "journal read failed");
                continue;
            }
        };
        // Deferred settlements: exits whose sells are now confirmed get
        // booked with real proceeds. This is what makes PnL trustworthy.
        if let Ok(pending) = store.pending_exits() {
            for (trade_id, mint, reason) in pending {
                let settled = store.orders_settled(&mint).unwrap_or(false);
                if !settled {
                    continue;
                }
                match store.realized_pnl_sol(trade_id) {
                    Ok(pnl) => {
                        if let Err(error) = store.finalize_exit(trade_id, &reason, pnl) {
                            warn!(%error, "failed to finalize exit");
                        } else {
                            info!(trade_id, mint = %mint, reason = %reason, pnl_sol = pnl, "LIVE exit settled");
                        }
                    }
                    Err(error) => warn!(%error, "exit finalize skipped"),
                }
            }
        }

        // Batch lookup of all outstanding signatures in ONE RPC call, instead
        // of one getTransaction per order. Most rows are not confirmed yet, so
        // a single status check resolves them without fetching full txs.
        let signed: Vec<(usize, String)> = orders
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.signature.clone().map(|s| (i, s)))
            .collect();
        let statuses: std::collections::HashMap<String, Option<(bool, Option<String>)>> =
            if signed.is_empty() {
                std::collections::HashMap::new()
            } else {
                signature_status_batch(
                    &config.rpc_url,
                    signed.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>().as_slice(),
                )
                .await
                .unwrap_or_default()
            };

        for order in orders.iter() {
            match order.signature.as_deref() {
                None => {
                    // No signature. A sell here is genuinely unknown and needs
                    // a human. A buy may still have landed without returning a
                    // signature (entry timed out mid-send), so before retiring
                    // it, check whether tokens actually arrived: if they did,
                    // adopt the position so exits can close it.
                    if order.side == "buy" {
                        if now_ms() - order.created_ms < 45_000 {
                            continue;
                        }
                        let mint = Pubkey::from_str(&order.mint).ok();
                        let tokens = match mint {
                            Some(mint) => wallet_token_balance(&config.rpc_url, &mint, &wallet)
                                .await
                                .unwrap_or(0.0),
                            None => 0.0,
                        };
                        let already = store.seen(&order.mint).unwrap_or(true);
                        if tokens > 0.0001 && !already {
                            let cost = (order.lamports.max(1) as f64) / LAMPORTS_PER_SOL;
                            let price = cost / tokens;
                            match store.record_entry(
                                &order.mint,
                                None,
                                "adopted",
                                price,
                                cost,
                                tokens,
                                "adopted-no-signature",
                                order.lamports,
                            ) {
                                Ok(trade_id) => {
                                    let _ = store.link_journal_trade(order.id, trade_id);
                                    let _ = store.journal_confirmed(order.id, order.lamports, tokens);
                                    warn!(
                                        mint = %order.mint, tokens, price_sol = price,
                                        "ADOPTED position: buy landed without a signature — exits enabled"
                                    );
                                }
                                Err(error) => warn!(%error, "failed to adopt position"),
                            }
                        } else {
                            // Keep whatever the submit path already recorded:
                            // "unsupported venue" is a different bug from "timed out".
                            let note = match order.error.as_deref() {
                                Some(previous) if !previous.is_empty() && previous != "None" => {
                                    format!("{previous} — never submitted (no signature)")
                                }
                                _ => "never submitted (no signature)".to_string(),
                            };
                            if let Err(error) = store.journal_failed(order.id, &note) {
                                warn!(%error, "journal update failed");
                            }
                        }

                    } else {
                        if let Err(error) = store.journal_unknown(order.id, "no signature recorded; submit status unknown") {
                            warn!(%error, "journal update failed");
                        }
                        error!(
                            mint = %order.mint,
                            kind = %order.kind,
                            "UNKNOWN sell order — trade requires operator review"
                        );
                    }
                }
                Some(signature) => {
                    // Cheap path first: the batch already told us the state.
                    let known = statuses.get(signature).cloned().flatten();
                    if let Some((false, Some(error_text))) = known {
                        let text = redact::redact_error(&error_text, &[config.private_key.as_str()]);
                        if let Err(error) = store.journal_failed(order.id, &text) {
                            warn!(%error, "journal update failed");
                        }
                        warn!(signature = %signature, mint = %order.mint, error = %text, "journal reconciled failed");
                        continue;
                    }
                    if known.is_none() {
                        // Not visible yet: stay pending. A blockhash lives ~90s,
                        // so a signature still unknown after 10 minutes never
                        // landed — that is terminal, not ambiguous.
                        if now_ms() - order.created_ms > 600_000 {
                            let note = "signature not found after 10m: transaction never landed";
                            if let Err(error) = store.journal_failed(order.id, note) {
                                warn!(%error, "journal update failed");
                            }
                            warn!(
                                signature = %signature, mint = %order.mint, kind = %order.kind,
                                "order never landed; marked failed"
                            );
                        }
                        continue;
                    }
                    match tx_outcome(&config.rpc_url, signature, &wallet).await {
                    Ok(Some((true, delta, _))) => {
                        if let Err(error) = store.journal_confirmed(order.id, delta.abs(), order.tokens) {
                            warn!(%error, "journal update failed");
                        }
                        if order.side == "sell" {
                            // Fill rows were written at submit time with 0 proceeds;
                            // the confirmed lamport delta is the truth.
                            if let Err(error) = store.apply_fill_proceeds(signature, delta.max(0)) {
                                warn!(%error, "failed to backfill fill proceeds");
                            }
                        }
                        if order.side == "buy" {
                            // Fill in the real cost and the real token balance.
                            let tokens = match Pubkey::from_str(&order.mint) {
                                Ok(mint) => wallet_token_balance(&config.rpc_url, &mint, &wallet)
                                    .await
                                    .unwrap_or(0.0),
                                Err(_) => 0.0,
                            };
                            if let Err(error) = store.apply_entry_confirmation(&order.mint, delta.abs(), tokens) {
                                warn!(%error, "failed to apply entry confirmation");
                            }
                            info!(mint = %order.mint, cost_lamports = delta.abs(), tokens, "entry confirmed from chain");
                        }
                        info!(
                            signature = %signature, mint = %order.mint,
                            delta_lamports = delta, "journal reconciled confirmed"
                        );
                    }
                    Ok(Some((false, _, error_text))) => {
                        let text = redact::redact_error(&error_text, &[config.private_key.as_str()]);
                        if let Err(error) = store.journal_failed(order.id, &text) {
                            warn!(%error, "journal update failed");
                        }
                        warn!(signature = %signature, mint = %order.mint, error = %text, "journal reconciled failed");
                    }
                    Ok(None) => {
                        // Not visible yet: stay pending. A blockhash lives ~90s,
                        // so a signature still unknown after 10 minutes never
                        // landed — that is terminal, not ambiguous.
                        if now_ms() - order.created_ms > 600_000 {
                            let note = "signature not found after 10m: transaction never landed";
                            if let Err(error) = store.journal_failed(order.id, note) {
                                warn!(%error, "journal update failed");
                            }
                            warn!(
                                signature = %signature, mint = %order.mint, kind = %order.kind,
                                "order never landed; marked failed"
                            );
                        }
                    }
                    Err(error) => warn!(
                        error = %redact::redact_error(&error.to_string(), &[]),
                        "reconcile RPC error"
                    ),
                    }
                }
            }
        }
    }
}

/// Independent exit task: position management never shares a loop with entry,
/// so a slow buy or a blocked RPC call cannot delay a stop.
#[derive(Debug, PartialEq, Eq)]
enum EvalDue {
    /// Re-price this position now.
    Poll,
    /// Not scheduled yet and no fresh tick for this pool.
    Wait,
    /// A tick or the interval wants a poll, but MIN_GAP_MS has not elapsed.
    RateLimited,
}

/// Decide whether a position should be re-priced.
///
/// `tick_at_ms` is when the pool last traded (from the price feed), already
/// converted to the same clock as `now_ms`. A tick newer than the last
/// evaluation wakes the position early; the scheduled interval still applies
/// as a floor so a quiet pool is checked on the regular cadence.
fn eval_due(
    now_ms: i64,
    last_eval_ms: i64,
    tick_at_ms: Option<i64>,
    scheduled_ms: i64,
    min_gap_ms: i64,
) -> EvalDue {
    let since = now_ms.saturating_sub(last_eval_ms);
    let scheduled_due = since >= scheduled_ms;
    let tick_new = tick_at_ms.map(|at| at > last_eval_ms).unwrap_or(false);
    if !scheduled_due && !tick_new {
        return EvalDue::Wait;
    }
    if since < min_gap_ms {
        return EvalDue::RateLimited;
    }
    EvalDue::Poll
}

async fn exit_loop(store: Arc<LiveStore>, client: Arc<SolanaTrade>, config: Arc<LiveConfig>) {
    // Per-mint last evaluation, so a busy pool cannot hammer RPC: the stream
    // can push 20+ ticks/s for a hot token.
    const MIN_GAP_MS: i64 = 250;
    const WAKE_SLICE_MS: u64 = 100;
    let mut last_eval: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    // Observability: proves the stream drives polls rather than merely
    // connecting. Logged once a minute.
    let mut tick_polls: u64 = 0;
    let mut scheduled_polls: u64 = 0;
    let mut last_stats = std::time::Instant::now();

    loop {
        let trades = match store.open_trades() {
            Ok(trades) => trades,
            Err(error) => {
                warn!(
                    error = %redact::redact_error(&error.to_string(), &[]),
                    "open trade read failed"
                );
                tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
                continue;
            }
        };
        let scheduled_ms = (config.poll_interval_secs as i64) * 1_000;
        let mut any_due = false;
        for trade in trades {
            // Backstop: a position that never resolves (no price, no balance)
            // must still be closed so it cannot linger for hours.
            const STALE_POSITION_MINUTES: i64 = 240;
            let age_minutes = now_ms().saturating_sub(trade.entry_time_unix_ms) / 60_000;
            if age_minutes > STALE_POSITION_MINUTES {
                warn!(mint = %trade.mint, age_minutes, "position exceeded stale limit; force closing");
                // Try to recover value first: a stale position may still hold
                // tokens. Only book the exit after the balance is empty.
                if let Ok(mint) = Pubkey::from_str(&trade.mint) {
                    let wallet = client.payer.pubkey();
                    let tokens = wallet_token_balance(&config.rpc_url, &mint, &wallet)
                        .await
                        .unwrap_or(0.0);
                    if tokens > 0.0001 {
                        match live_sell(&client, &config, &store, trade.id, "stale", mint, tokens, true).await {
                            Ok(_) => info!(mint = %trade.mint, tokens, "stale position sold"),
                            Err(error) => {
                                warn!(
                                    mint = %trade.mint,
                                    error = %redact::redact_error(&error.to_string(), &[config.private_key.as_str()]),
                                    "stale sell failed; booking exit anyway"
                                );
                            }
                        }
                    }
                }
                match store.realized_pnl_sol(trade.id) {
                    Ok(pnl) => {
                        if let Err(error) = store.record_exit(trade.id, "stale", pnl) {
                            warn!(%error, "failed to close stale position");
                        }
                    }
                    Err(error) => warn!(%error, "stale close skipped: pnl unavailable"),
                }
                continue;
            }
            // Due when the scheduled interval elapsed OR the stream just
            // showed a trade for this pool. MIN_GAP_MS keeps a hot pool from
            // flooding the RPC with reads.
            let now = now_ms();
            let last = last_eval.get(&trade.mint).copied().unwrap_or(0);
            let tick_at_ms = price_feed::last_tick(&trade.mint)
                .map(|at| now.saturating_sub(at.elapsed().as_millis() as i64));
            match eval_due(now, last, tick_at_ms, scheduled_ms, MIN_GAP_MS) {
                EvalDue::Poll => {
                    last_eval.insert(trade.mint.clone(), now);
                    if tick_at_ms.map(|at| at > last).unwrap_or(false) {
                        tick_polls += 1;
                    } else {
                        scheduled_polls += 1;
                    }
                }
                EvalDue::RateLimited => {
                    any_due = true;
                    continue;
                }
                EvalDue::Wait => continue,
            }

            if let Err(error) = poll_live_position(&client, &store, &config, trade).await {
                warn!(
                    error = %redact::redact_error(&error.to_string(), &[config.private_key.as_str()]),
                    "exit evaluation error"
                );
            }
        }

        // Short slice so a stream tick wakes us in ~100ms instead of waiting
        // out the whole poll interval. any_due means a position wanted to run
        // but was rate-limited, so re-check immediately.
        let slice = if any_due { 0 } else { WAKE_SLICE_MS };
        tokio::time::sleep(std::time::Duration::from_millis(slice)).await;

        if last_stats.elapsed() >= std::time::Duration::from_secs(60) {
            last_stats = std::time::Instant::now();
            info!(
                tick_polls,
                scheduled_polls,
                open = store.open_trades().map(|t| t.len()).unwrap_or(0),
                tracked_mints = price_feed::probe_count().unwrap_or(0),
                "exit wake stats (tick-driven vs scheduled polls in the last minute)"
            );
            tick_polls = 0;
            scheduled_polls = 0;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    ).init();

    // Read-only diagnostic: verify the free price stream connects and ticks
    // without touching funds or requiring LIVE_ENABLED.
    if std::env::args().any(|a| a == "--price-probe") {
        info!("price probe: connecting to pumpapi.io stream for 20s (read-only)");
        price_feed::ensure_tls();
        tokio::spawn(price_feed::run(Vec::new()));
        let mut seen = 0usize;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if let Some(tracked) = price_feed::probe_count() {
                if tracked > seen {
                    seen = tracked;
                }
            }
        }
        info!(mints_tracked = seen, "price probe finished");
        return Ok(());
    }

    if std::env::var("LIVE_ENABLED").ok().as_deref() != Some("true") {
        bail!("LIVE_ENABLED=true required — this engine spends real SOL");
    }
    let config = LiveConfig::from_env()?;
    let store = Arc::new(LiveStore::open(&config.database_path)?);
    let (client, wallet) = build_client(&config).await?;

    let balance = client.infrastructure.rpc.get_balance(&wallet).await? as f64 / LAMPORTS_PER_SOL;
    info!(%wallet, balance_sol = balance, buy_sol = config.buy_sol, "LIVE engine starting — real trades");
    if balance < config.buy_sol * 3.0 + 0.01 {
        bail!("wallet balance {:.6} SOL too low for live trading", balance);
    }

    // Restore open positions into the poll loop.
    for trade in store.open_trades()? {
        info!(mint = %trade.mint, "resuming live position");
    }

    // Restore any pre-existing journal backlog before trading resumes.
    if let Ok(orders) = store.unsettled_orders(20_000) {
        if !orders.is_empty() {
            warn!(count = orders.len(), "unresolved journal orders at startup; reconciling");
        }
    }

    let config = Arc::new(config);
    // Independent tasks: exits and reconciliation never share the entry loop.
    let _exit_task = tokio::spawn(exit_loop(
        Arc::clone(&store),
        Arc::clone(&client),
        Arc::clone(&config),
    ));
    let _fee_task = tokio::spawn(fee_refresh_loop(Arc::clone(&config)));
    // Free pumpapi.io data stream: pushes priceSol per trade so exits read a
    // cached tick instead of waiting on a 2s RPC poll. Read-only.
    let _price_task = tokio::spawn(price_feed::run(Vec::new()));
    let _hash_task = tokio::spawn(blockhash_refresh_loop(Arc::clone(&client)));
    let _reconcile_task = tokio::spawn(reconcile_loop(
        Arc::clone(&store),
        Arc::clone(&config),
        wallet,
    ));

    // Persistent reader task -> channel
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<NewMintEvent>();
    let _reader = spawn_event_reader(config.event_socket_path.clone(), event_tx);

    loop {
        // 1. Buy on every freshly arrived signal (dedup handled by store).
        while let Ok(event) = event_rx.try_recv() {
            if store.seen(&event.mint)? {
                continue;
            }
            if store.open_count()? >= config.max_open_positions {
                warn!(mint = %event.mint, "max open positions reached; skipping signal");
                continue;
            }
            let Ok(mint) = Pubkey::from_str(&event.mint) else {
                warn!(mint = %event.mint, "invalid mint in signal");
                continue;
            };
                // Hard bound on the whole entry: a graded token whose submit
                // hangs must not freeze the loop and miss other signals.
                let attempt = tokio::time::timeout(
                    std::time::Duration::from_millis(config.entry_timeout_ms),
                    live_buy(&client, &config, &store, mint),
                )
                .await;
                let attempt = match attempt {
                    Ok(result) => result,
                    Err(_) => {
                        error!(
                            mint = %event.mint,
                            timeout_ms = config.entry_timeout_ms,
                            "entry timed out; journal left to reconciliation"
                        );
                        continue;
                    }
                };
                match attempt {
                Ok((price, tokens, sig, cost_lamports, journal_id)) => {
                    let trade_id = store.record_entry(&event.mint, event.ticker.as_deref(), &event.source, price, config.buy_sol, tokens, &sig, cost_lamports)?;
                    if let Err(error) = store.link_journal_trade(journal_id, trade_id) {
                        warn!(%error, "failed to link journal entry to trade");
                    }
                    info!(mint = %event.mint, sig = %sig, "LIVE entry");
                }
                Err(error) => error!(
                    mint = %event.mint,
                    error = %redact::redact_error(&error.to_string(), &[config.private_key.as_str()]),
                    "LIVE buy failed"
                ),
            }
        }

        // Exits run in their own task; this loop only takes entries.
        tokio::time::sleep(std::time::Duration::from_millis(config.entry_tick_ms)).await;
    }
}
#[cfg(test)]
mod exit_wake_tests {
    use super::{eval_due, EvalDue};

    const SCHEDULED: i64 = 2_000;
    const GAP: i64 = 250;

    #[test]
    fn waits_when_not_scheduled_and_no_tick() {
        // last eval 500ms ago, interval 2s, no tick
        assert_eq!(eval_due(10_500, 10_000, None, SCHEDULED, GAP), EvalDue::Wait);
    }

    #[test]
    fn polls_on_schedule_even_without_ticks() {
        // last eval 2s ago: scheduled floor reached
        assert_eq!(eval_due(12_000, 10_000, None, SCHEDULED, GAP), EvalDue::Poll);
    }

    #[test]
    fn a_new_tick_wakes_long_before_the_interval() {
        // interval 2s, but the pool traded 100ms after the last eval
        assert_eq!(
            eval_due(10_300, 10_000, Some(10_100), SCHEDULED, GAP),
            EvalDue::Poll,
            "a fresh tick must re-price immediately, not wait out 2s"
        );
    }

    #[test]
    fn a_stale_tick_does_not_wake() {
        // tick predates the last evaluation: already acted on
        assert_eq!(
            eval_due(10_300, 10_000, Some(9_000), SCHEDULED, GAP),
            EvalDue::Wait
        );
    }

    #[test]
    fn min_gap_protects_the_rpc_from_a_hot_pool() {
        // tick arrived 50ms after last eval; 20 ticks/s must not become 20 polls/s
        assert_eq!(
            eval_due(10_050, 10_000, Some(10_020), SCHEDULED, GAP),
            EvalDue::RateLimited
        );
    }

    #[test]
    fn rate_limit_clears_once_the_gap_passes() {
        assert_eq!(
            eval_due(10_300, 10_000, Some(10_020), SCHEDULED, GAP),
            EvalDue::Poll
        );
    }

    #[test]
    fn first_evaluation_of_a_new_position_polls_immediately() {
        assert_eq!(eval_due(10_000, 0, None, SCHEDULED, GAP), EvalDue::Poll);
    }
}
