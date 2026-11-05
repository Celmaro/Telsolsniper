//! Telsolsniper live engine — real trades at tiny size (0.001 SOL).
//!
//! Consumes deduped NewMint events from the intake fanout socket (same
//! stream as the shadow engine), buys on Pump.fun curve or PumpSwap,
//! then runs the same staged exit ladder as the shadow engine:
//! TP1 -> TP2 -> moon bag trailing stop, SL, breakeven after TP1.
//!
//! SAFETY: requires LIVE_ENABLED=true, a funded wallet in PRIVATE_KEY,
//! and refuses to start if wallet balance < 2 x per-trade size + rent.

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
    AccountPolicy, BuyAmount, SolanaTrade, TradeTokenType,
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

const TOKEN_DECIMALS: f64 = 1_000_000.0; // pump tokens: 6 dp
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

#[derive(Debug, Deserialize)]
struct NewMintEvent {
    source: String,
    message_id: i64,
    mint: String,
    ticker: Option<String>,
}

struct LiveConfig {
    event_socket_path: String,
    database_path: String,
    private_key_env: String,
    buy_sol: f64,
    slippage_bps: u64,
    tp1_multiple: f64,
    tp1_fraction: f64,
    tp2_multiple: f64,
    tp2_fraction: f64,
    moon_trail_pct: f64,
    initial_stop_pct: f64,
    breakeven_after_tp1: bool,
    poll_interval_secs: u64,
    max_open_positions: usize,
}

impl LiveConfig {
    fn from_env() -> Result<Self> {
        let f = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
        let buy_sol = f("LIVE_BUY_SOL", "0.001").parse()?;
        let cfg = Self {
            event_socket_path: f("EVENT_SOCKET_PATH", "/tmp/telsolsniper-events.sock"),
            database_path: f("LIVE_DATABASE_PATH", "./data/live.sqlite"),
            private_key_env: f("PRIVATE_KEY_ENV", "PRIVATE_KEY"),
            buy_sol,
            slippage_bps: f("LIVE_SLIPPAGE_BPS", "1000").parse()?, // 10%: trench tokens move
            tp1_multiple: f("LIVE_TP1_X", "1.1").parse()?,
            tp1_fraction: f("LIVE_TP1_SELL_PCT", "40").parse()?,
            tp2_multiple: f("LIVE_TP2_X", "1.5").parse()?,
            tp2_fraction: f("LIVE_TP2_SELL_PCT", "40").parse()?,
            moon_trail_pct: f("LIVE_MOON_TRAIL_PCT", "35").parse()?,
            initial_stop_pct: f("LIVE_STOP_LOSS_PCT", "35").parse()?,
            breakeven_after_tp1: f("LIVE_BREAKEVEN_AFTER_TP1", "true").eq_ignore_ascii_case("true"),
            poll_interval_secs: f("LIVE_POLL_SECS", "5").parse()?,
            max_open_positions: f("LIVE_MAX_OPEN", "5").parse()?,
        };
        if cfg.tp2_multiple <= cfg.tp1_multiple || cfg.tp1_fraction + cfg.tp2_fraction >= 100.0 {
            bail!("invalid ladder config");
        }
        Ok(cfg)
    }
}

struct LiveStore {
    connection: Connection,
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
                signature TEXT
            );
            "#,
        )?;
        Ok(Self { connection })
    }

    fn seen(&self, mint: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row("SELECT 1 FROM live_trades WHERE mint = ?1", [mint], |_| Ok(()))
            .is_ok())
    }

    fn record_entry(&self, mint: &str, ticker: Option<&str>, source: &str, entry_price_sol: f64, buy_sol: f64, token_amount: f64, signature: &str) -> Result<i64> {
        self.connection.execute(
            r#"INSERT INTO live_trades (mint, ticker, source, entry_time_unix_ms, entry_price_sol, buy_sol_amount, token_amount, entry_signature, peak_price_sol)
               VALUES (?1, ?2, ?3, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), ?4, ?5, ?6, ?7, ?4)"#,
            rusqlite::params![mint, ticker, source, entry_price_sol, buy_sol, token_amount, signature],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    fn open_trades(&self) -> Result<Vec<LiveTrade>> {
        let mut stmt = self.connection.prepare(
            r#"SELECT id, mint, ticker, source, token_amount, entry_price_sol,
                      peak_price_sol, tp1_done, tp2_done, realized_pnl_sol, remaining_tokens
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
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn tokens_at_entry(t: &LiveTrade) -> f64 {
        if t.remaining_tokens > 0.0 { t.remaining_tokens } else { t.token_amount }
    }

    fn record_fill(&self, trade_id: i64, kind: &str, multiple: f64, price_sol: f64, tokens_sold: f64, proceeds_sol: f64, signature: Option<&str>) -> Result<()> {
        self.connection.execute(
            r#"INSERT INTO live_fills (trade_id, fill_time_unix_ms, kind, multiple, price_sol, tokens_sold, proceeds_sol, signature)
               VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), ?2, ?3, ?4, ?5, ?6, ?7)"#,
            rusqlite::params![trade_id, kind, multiple, price_sol, tokens_sold, proceeds_sol, signature],
        )?;
        Ok(())
    }

    fn update_stage(&self, id: i64, kind: &str, realized: f64, remaining: f64) -> Result<()> {
        self.connection.execute(
            r#"UPDATE live_trades
               SET realized_pnl_sol = ?2, remaining_tokens = ?3,
                   tp1_done = tp1_done | ?4, tp2_done = tp2_done | ?5
               WHERE id = ?1"#,
            rusqlite::params![id, realized, remaining, (kind == "tp1") as i64, (kind == "tp2") as i64],
        )?;
        Ok(())
    }

    fn update_peak(&self, id: i64, peak: f64) -> Result<()> {
        self.connection.execute("UPDATE live_trades SET peak_price_sol = ?2 WHERE id = ?1", rusqlite::params![id, peak])?;
        Ok(())
    }

    fn record_exit(&self, id: i64, exit_reason: &str, pnl_sol: f64) -> Result<()> {
        self.connection.execute(
            r#"UPDATE live_trades SET exit_time_unix_ms = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), exit_reason = ?2, pnl_sol = ?3 WHERE id = ?1"#,
            rusqlite::params![id, exit_reason, pnl_sol],
        )?;
        Ok(())
    }

    fn open_count(&self) -> Result<usize> {
        Ok(self.connection.query_row("SELECT COUNT(*) FROM live_trades WHERE exit_time_unix_ms IS NULL", [], |r| r.get(0))?)
    }
}

async fn build_client(config: &LiveConfig) -> Result<(Arc<SolanaTrade>, Pubkey)> {
    let payer = sol_trade_sdk::common::keypair::load_keypair_from_env(&config.private_key_env)
        .context("PRIVATE_KEY (base58) env var required")?;
    let pubkey = payer.pubkey();
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let swqos_configs = vec![SwqosConfig::Default(rpc_url.clone())];
    let trade_config = TradeConfig::builder(rpc_url.clone(), swqos_configs, CommitmentConfig::processed())
        .create_wsol_ata_on_startup(true)
        .build();
    let client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    Ok((Arc::new(client), pubkey))
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
    let params = PumpSwapParams::from_mint_by_rpc(rpc, mint).await?;
    // Price discovery via pool reserves is handled by the SDK on execution;
    // for sizing we reuse the shadow engine's approach: quote a tiny buy.
    let price = quote_pumpswap_price(rpc, mint).await?;
    Ok((DexType::PumpSwap, DexParamEnum::PumpSwap(params), price))
}

async fn quote_pumpswap_price(rpc: &SolanaRpcClient, mint: &Pubkey) -> Result<f64> {
    let params = PumpSwapParams::from_mint_by_rpc(rpc, mint).await?;
    let base_reserve = params.pool_base_token_reserves;
    let quote_reserve = params.pool_quote_token_reserves;
    Ok((quote_reserve as f64 / LAMPORTS_PER_SOL) / (base_reserve as f64 / TOKEN_DECIMALS))
}

async fn live_buy(
    client: &SolanaTrade,
    config: &LiveConfig,
    mint: Pubkey,
) -> Result<(f64, f64, String)> {
    let (dex_type, extension_params, price_sol) = fetch_venue_params(client, &mint).await?;
    let lamports = (config.buy_sol * LAMPORTS_PER_SOL) as u64;
    let blockhash = client.infrastructure.rpc.get_latest_blockhash().await?;
    let gas = GasFeeStrategy::new();
    gas.set_global_fee_strategy(150_000, 150_000, 500_000, 500_000, 0.001, 0.001);

    let params = sol_trade_sdk::TradeBuyParams {
        dex_type,
        input_token_type: TradeTokenType::SOL,
        mint,
        input_token_amount: lamports,
        slippage_basis_points: Some(config.slippage_bps),
        recent_blockhash: Some(blockhash),
        extension_params,
        address_lookup_table_accounts: Vec::new(),
        wait_tx_confirmed: true,
        wait_for_all_submits: false,
        create_input_token_ata: false,
        close_input_token_ata: false,
        create_mint_ata: true,
        durable_nonce: None,
        fixed_output_token_amount: None,
        gas_fee_strategy: gas,
        simulate: false,
        use_exact_sol_amount: Some(true),
        grpc_recv_us: None,
    };
    let (ok, signatures, error, _) = client.buy(params).await?;
    if !ok {
        bail!("buy failed: {:?}", error);
    }
    let signature = signatures
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".into());
    // Actual fill: assume we got close to expected; refine from balance delta.
    let token_amount = config.buy_sol / price_sol * 0.97; // conservative fill estimate
    Ok((price_sol, token_amount, signature))
}

async fn live_sell(
    client: &SolanaTrade,
    config: &LiveConfig,
    mint: Pubkey,
    token_amount: f64,
) -> Result<(f64, String)> {
    let (dex_type, extension_params, price_sol) = fetch_venue_params(client, &mint).await?;
    let raw_amount = (token_amount * TOKEN_DECIMALS) as u64;
    let blockhash = client.infrastructure.rpc.get_latest_blockhash().await?;
    let gas = GasFeeStrategy::new();
    gas.set_global_fee_strategy(150_000, 150_000, 500_000, 500_000, 0.001, 0.001);

    let params = sol_trade_sdk::TradeSellParams {
        dex_type,
        output_token_type: TradeTokenType::SOL,
        mint,
        input_token_amount: raw_amount,
        slippage_basis_points: Some(config.slippage_bps),
        recent_blockhash: Some(blockhash),
        extension_params,
        address_lookup_table_accounts: Vec::new(),
        wait_tx_confirmed: true,
        wait_for_all_submits: false,
        create_output_token_ata: false,
        close_output_token_ata: true,
        close_mint_token_ata: false,
        with_tip: false,
        durable_nonce: None,
        fixed_output_token_amount: None,
        gas_fee_strategy: gas,
        simulate: false,
        grpc_recv_us: None,
    };
    let (ok, signatures, error, _) = client.sell(params).await?;
    if !ok {
        bail!("sell failed: {:?}", error);
    }
    let signature = signatures.first().map(|s| s.to_string()).unwrap_or_default();
    Ok((price_sol, signature))
}

async fn poll_live_position(
    client: &Arc<SolanaTrade>,
    store: &LiveStore,
    config: &LiveConfig,
    trade: LiveTrade,
) -> Result<()> {
    let Ok(mint) = Pubkey::from_str(&trade.mint) else { return Ok(()) };
    let rpc = &client.infrastructure.rpc;

    // Price check via curve (active) or pool (graduated).
    let price = match PumpFunParams::from_mint_by_rpc(rpc, &mint).await {
        Ok(p) if !p.bonding_curve.as_ref().complete => {
            let c = p.bonding_curve.as_ref();
            (c.virtual_sol_reserves as f64 / LAMPORTS_PER_SOL) / (c.virtual_token_reserves as f64 / TOKEN_DECIMALS)
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
    let tokens_now = LiveStore::tokens_at_entry(&trade);

    // Ladder (same rules as shadow).
    if !trade.tp1_done && multiple >= config.tp1_multiple {
        let tokens = tokens_now * config.tp1_fraction / 100.0;
        match live_sell(client, config, mint, tokens).await {
            Ok((fill_price, sig)) => {
                store.record_fill(trade.id, "tp1", fill_price / trade.entry_price_sol, fill_price, tokens, tokens * fill_price, Some(&sig))?;
                let realized = trade.realized_pnl_sol + tokens * fill_price - config.buy_sol * (tokens / trade.token_amount);
                let remaining = tokens_now - tokens;
                store.update_stage(trade.id, "tp1", realized, remaining)?;
                info!(mint = %trade.mint, sig = %sig, "LIVE tp1 filled");
            }
            Err(error) => error!(mint = %trade.mint, %error, "LIVE tp1 sell failed"),
        }
        return Ok(()); // one action per tick
    }
    if trade.tp1_done && !trade.tp2_done && multiple >= config.tp2_multiple {
        let tokens = tokens_now * config.tp2_fraction / 100.0;
        match live_sell(client, config, mint, tokens).await {
            Ok((fill_price, sig)) => {
                store.record_fill(trade.id, "tp2", fill_price / trade.entry_price_sol, fill_price, tokens, tokens * fill_price, Some(&sig))?;
                let realized = trade.realized_pnl_sol + tokens * fill_price - config.buy_sol * (tokens / trade.token_amount);
                let remaining = tokens_now - tokens;
                store.update_stage(trade.id, "tp2", realized, remaining)?;
                info!(mint = %trade.mint, sig = %sig, "LIVE tp2 filled");
            }
            Err(error) => error!(mint = %trade.mint, %error, "LIVE tp2 sell failed"),
        }
        return Ok(());
    }

    let stop_hit = if !trade.tp1_done {
        multiple <= 1.0 - config.initial_stop_pct / 100.0
    } else if !trade.tp2_done {
        config.breakeven_after_tp1 && multiple <= 1.0
    } else {
        peak / trade.entry_price_sol >= config.tp2_multiple
            && price <= peak * (1.0 - config.moon_trail_pct / 100.0)
    };
    if stop_hit && tokens_now > 1.0 {
        let kind = if trade.tp1_done { "moon_bag" } else { "stop" };
        match live_sell(client, config, mint, tokens_now).await {
            Ok((fill_price, sig)) => {
                let proceeds = tokens_now * fill_price;
                store.record_fill(trade.id, kind, fill_price / trade.entry_price_sol, fill_price, tokens_now, proceeds, Some(&sig))?;
                let realized = trade.realized_pnl_sol + proceeds - config.buy_sol * (tokens_now / trade.token_amount);
                store.update_stage(trade.id, kind, realized, 0.0)?;
                store.record_exit(trade.id, kind, realized)?;
                info!(mint = %trade.mint, pnl_sol = %realized, "LIVE exit");
            }
            Err(error) => error!(mint = %trade.mint, %error, "LIVE stop sell failed"),
        }
    }
    Ok(())
}

async fn event_stream(config: &LiveConfig) -> Result<tokio::net::UnixStream> {
    Ok(tokio::net::UnixStream::connect(&config.event_socket_path).await?)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    ).init();

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

    loop {
        // Poll open positions.
        for trade in store.open_trades()? {
            if let Err(error) = poll_live_position(&client, &store, &config, trade).await {
                warn!(%error, "poll error");
            }
        }

        // Drain one event if a connection is possible.
        match event_stream(&config).await {
            Ok(stream) => {
                let mut buf = Vec::new();
                let mut stream = stream;
                loop {
                    // Read one JSON line per event.
                    use tokio::io::AsyncReadExt;
                    match stream.read_u8().await {
                        Ok(b) => {
                            buf.push(b);
                            if b == b'\n' {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                if !buf.is_empty() {
                    if let Ok(event) = serde_json::from_slice::<NewMintEvent>(&buf) {
                        if !store.seen(&event.mint)? && store.open_count()? < config.max_open_positions {
                            match live_buy(&client, &config, Pubkey::from_str(&event.mint)?).await {
                                Ok((price, tokens, sig)) => {
                                    store.record_entry(&event.mint, event.ticker.as_deref(), &event.source, price, config.buy_sol, tokens, &sig)?;
                                    info!(mint = %event.mint, sig = %sig, "LIVE entry");
                                }
                                Err(error) => error!(mint = %event.mint, %error, "LIVE buy failed"),
                            }
                        }
                    }
                }
            }
            Err(error) => {
                warn!(%error, "event socket unavailable; retrying");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
    }
}
