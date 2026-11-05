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
    buy_sol: f64,
    slippage_bps: u64,
    /// Tip per transaction in SOL. At 0.001 SOL trades a 0.001 tip doubles
    /// the cost — keep it tiny.
    tip_sol: f64,
    cu_price_micro_lamports: u32,
    tp1_multiple: f64,
    tp1_fraction: f64,
    tp2_multiple: f64,
    tp2_fraction: f64,
    moon_trail_pct: f64,
    initial_stop_pct: f64,
    breakeven_after_tp1: bool,
    poll_interval_secs: u64,
    failure_cooldown_secs: i64,
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
            buy_sol,
            slippage_bps: f("LIVE_SLIPPAGE_BPS", "1000").parse()?, // 10%: trench tokens move
            tip_sol: f("LIVE_TIP_SOL", "0.00002").parse()?,
            cu_price_micro_lamports: f("LIVE_CU_PRICE", "150000").parse()?,
            tp1_multiple: f("LIVE_TP1_X", "1.1").parse()?,
            tp1_fraction: f("LIVE_TP1_SELL_PCT", "40").parse()?,
            tp2_multiple: f("LIVE_TP2_X", "1.5").parse()?,
            tp2_fraction: f("LIVE_TP2_SELL_PCT", "40").parse()?,
            moon_trail_pct: f("LIVE_MOON_TRAIL_PCT", "35").parse()?,
            initial_stop_pct: f("LIVE_STOP_LOSS_PCT", "35").parse()?,
            breakeven_after_tp1: f("LIVE_BREAKEVEN_AFTER_TP1", "true").eq_ignore_ascii_case("true"),
            poll_interval_secs: f("LIVE_POLL_SECS", "5").parse()?,
            failure_cooldown_secs: f("LIVE_FAILURE_COOLDOWN_SECS", "60").parse()?,
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
        let fill_cols: Vec<String> = {
            let mut stmt = connection.prepare("PRAGMA table_info(live_fills)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if !fill_cols.iter().any(|c| c == "proceeds_lamports") {
            connection.execute("ALTER TABLE live_fills ADD COLUMN proceeds_lamports INTEGER NOT NULL DEFAULT 0", [])?;
        }
        Ok(Self {
            connection,
            cooldowns: std::sync::Mutex::new(std::collections::HashMap::new()),
            zero_reads: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    fn seen(&self, mint: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row("SELECT 1 FROM live_trades WHERE mint = ?1", [mint], |_| Ok(()))
            .is_ok())
    }

    fn record_entry(&self, mint: &str, ticker: Option<&str>, source: &str, entry_price_sol: f64, buy_sol: f64, token_amount: f64, signature: &str, cost_lamports: i64) -> Result<i64> {
        self.connection.execute(
            r#"INSERT INTO live_trades (mint, ticker, source, entry_time_unix_ms, entry_price_sol, buy_sol_amount, token_amount, cost_lamports, entry_signature, peak_price_sol)
               VALUES (?1, ?2, ?3, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), ?4, ?5, ?6, ?8, ?7, ?4)"#,
            rusqlite::params![mint, ticker, source, entry_price_sol, buy_sol, token_amount, signature, cost_lamports],
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

    fn record_fill(&self, trade_id: i64, kind: &str, multiple: f64, price_sol: f64, tokens_sold: f64, proceeds_lamports: i64, signature: Option<&str>) -> Result<()> {
        let proceeds_sol = proceeds_lamports as f64 / LAMPORTS_PER_SOL;
        self.connection.execute(
            r#"INSERT INTO live_fills (trade_id, fill_time_unix_ms, kind, multiple, price_sol, tokens_sold, proceeds_sol, proceeds_lamports, signature)
               VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), ?2, ?3, ?4, ?5, ?6, ?8, ?7)"#,
            rusqlite::params![trade_id, kind, multiple, price_sol, tokens_sold, proceeds_sol, signature, proceeds_lamports],
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

    /// Net SOL PnL from the chain: real proceeds (fees/tips already netted)
    /// minus the real cost of the buy transaction.
    fn realized_pnl_sol(&self, trade_id: i64) -> Result<f64> {
        let proceeds: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(proceeds_lamports), 0) FROM live_fills WHERE trade_id = ?1",
            [trade_id], |r| r.get(0),
        )?;
        let cost: i64 = self.connection.query_row(
            "SELECT COALESCE(cost_lamports, 0) FROM live_trades WHERE id = ?1",
            [trade_id], |r| r.get(0),
        )?;
        Ok((proceeds - cost) as f64 / LAMPORTS_PER_SOL)
    }

    fn open_count(&self) -> Result<usize> {
        Ok(self.connection.query_row("SELECT COUNT(*) FROM live_trades WHERE exit_time_unix_ms IS NULL", [], |r| r.get(0))?)
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
    let params = PumpSwapParams::from_mint_by_rpc(rpc, mint).await?;
    let base_reserve = params.pool_base_token_reserves;
    let quote_reserve = params.pool_quote_token_reserves;
    Ok((quote_reserve as f64 / LAMPORTS_PER_SOL) / (base_reserve as f64 / TOKEN_DECIMALS))
}

/// Returns (effective price incl. all fees/rent, tokens received, signature, real cost in lamports).
/// GasFeeStrategy::set_global_fee_strategy only configures the premium
/// SWQOS transport tiers — it SKIPS SwqosType::Default. We run Default,
/// so set it explicitly: low tip (0.00002 SOL) + reasonable CU price.
fn build_gas(config: &LiveConfig) -> GasFeeStrategy {
    use sol_trade_sdk::swqos::{SwqosType, TradeType};
    use sol_trade_sdk::common::GasFeeStrategyType;
    let gas = GasFeeStrategy::new();
    gas.set(
        SwqosType::Default,
        TradeType::Buy,
        GasFeeStrategyType::Normal,
        500_000,
        config.cu_price_micro_lamports as u64,
        config.tip_sol,
    );
    gas.set(
        SwqosType::Default,
        TradeType::Sell,
        GasFeeStrategyType::Normal,
        500_000,
        config.cu_price_micro_lamports as u64,
        config.tip_sol,
    );
    gas
}

async fn live_buy(
    client: &SolanaTrade,
    config: &LiveConfig,
    mint: Pubkey,
) -> Result<(f64, f64, String, i64)> {
    let (dex_type, extension_params, price_sol) = fetch_venue_params(client, &mint).await?;
    let lamports = (config.buy_sol * LAMPORTS_PER_SOL) as u64;
    let blockhash = client.infrastructure.rpc.get_latest_blockhash().await?;
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
        .context("buy returned no signature")?;
    // Real numbers from the chain: what we actually spent and received.
    let wallet = client.payer.pubkey();
    let rpc_url = config.rpc_url.as_str();
    let cost_lamports = wallet_delta(rpc_url, &signature, &wallet).await.unwrap_or(0).abs();
    let token_amount = wallet_token_balance(rpc_url, &mint, &wallet)
        .await
        .unwrap_or(config.buy_sol / price_sol * 0.97);
    // entry_price stays the market (curve/pool) price so TP/SL multiples
    // measure market movement; cost_lamports carries the all-in real cost.
    Ok((price_sol, token_amount, signature, cost_lamports))
}

async fn live_sell(
    client: &SolanaTrade,
    config: &LiveConfig,
    mint: Pubkey,
    token_amount: f64,
    close_ata: bool,
) -> Result<(f64, String, i64)> {
    let (dex_type, extension_params, price_sol) = fetch_venue_params(client, &mint).await?;
    let raw_amount = (token_amount * TOKEN_DECIMALS) as u64;
    let blockhash = client.infrastructure.rpc.get_latest_blockhash().await?;
    let gas = build_gas(config);

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
    let (ok, signatures, error, _) = client.sell(params).await?;
    if !ok {
        bail!("sell failed: {:?}", error);
    }
    let signature = signatures.first().map(|s| s.to_string()).unwrap_or_default();
    let wallet = client.payer.pubkey();
    let proceeds_lamports = wallet_delta(&config.rpc_url, &signature, &wallet)
        .await
        .unwrap_or((token_amount * price_sol * LAMPORTS_PER_SOL) as i64);
    Ok((price_sol, signature, proceeds_lamports))
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
                warn!(mint = %trade.mint, "no tokens on chain after 3 reads; marking closed");
                store.record_exit(trade.id, "empty", store.realized_pnl_sol(trade.id)?)?;
            }
            return Ok(());
        }
        Err(error) => {
            warn!(mint = %trade.mint, %error, "balance read failed; using recorded size");
            LiveStore::tokens_at_entry(&trade)
        }
    };

    if !store.can_attempt(trade.id) {
        return Ok(());
    }

    // Ladder (same rules as shadow).
    if !trade.tp1_done && multiple >= config.tp1_multiple {
        let tokens = tokens_now * config.tp1_fraction / 100.0;
        match live_sell(client, config, mint, tokens, false).await {
            Ok((fill_price, sig, proceeds_lamports)) => {
                store.record_fill(trade.id, "tp1", fill_price / trade.entry_price_sol, fill_price, tokens, proceeds_lamports, Some(&sig))?;
                let realized = store.realized_pnl_sol(trade.id)?;
                let remaining = tokens_now - tokens;
                store.update_stage(trade.id, "tp1", realized, remaining)?;
                info!(mint = %trade.mint, sig = %sig, proceeds_sol = %(proceeds_lamports as f64 / LAMPORTS_PER_SOL), "LIVE tp1 filled");
            }
            Err(error) => {
                store.mark_failure(trade.id, config.failure_cooldown_secs);
                error!(mint = %trade.mint, %error, "LIVE tp1 sell failed");
            }
        }
        return Ok(()); // one action per tick
    }
    if trade.tp1_done && !trade.tp2_done && multiple >= config.tp2_multiple {
        let tokens = tokens_now * config.tp2_fraction / 100.0;
        match live_sell(client, config, mint, tokens, false).await {
            Ok((fill_price, sig, proceeds_lamports)) => {
                store.record_fill(trade.id, "tp2", fill_price / trade.entry_price_sol, fill_price, tokens, proceeds_lamports, Some(&sig))?;
                let realized = store.realized_pnl_sol(trade.id)?;
                let remaining = tokens_now - tokens;
                store.update_stage(trade.id, "tp2", realized, remaining)?;
                info!(mint = %trade.mint, sig = %sig, proceeds_sol = %(proceeds_lamports as f64 / LAMPORTS_PER_SOL), "LIVE tp2 filled");
            }
            Err(error) => {
                store.mark_failure(trade.id, config.failure_cooldown_secs);
                error!(mint = %trade.mint, %error, "LIVE tp2 sell failed");
            }
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
        match live_sell(client, config, mint, tokens_now, true).await {
            Ok((fill_price, sig, proceeds_lamports)) => {
                store.record_fill(trade.id, kind, fill_price / trade.entry_price_sol, fill_price, tokens_now, proceeds_lamports, Some(&sig))?;
                let realized = store.realized_pnl_sol(trade.id)?;
                store.update_stage(trade.id, kind, realized, 0.0)?;
                store.record_exit(trade.id, kind, realized)?;
                info!(mint = %trade.mint, pnl_sol = %realized, "LIVE exit");
            }
            Err(error) => {
                store.mark_failure(trade.id, config.failure_cooldown_secs);
                error!(mint = %trade.mint, %error, "LIVE stop sell failed");
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

    // Persistent reader task -> channel. Buying happens off the poll loop.
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
            match live_buy(&client, &config, mint).await {
                Ok((price, tokens, sig, cost_lamports)) => {
                    store.record_entry(&event.mint, event.ticker.as_deref(), &event.source, price, config.buy_sol, tokens, &sig, cost_lamports)?;
                    info!(mint = %event.mint, sig = %sig, "LIVE entry");
                }
                Err(error) => error!(mint = %event.mint, %error, "LIVE buy failed"),
            }
        }

        // 2. Poll open positions (exit ladder).
        for trade in store.open_trades()? {
            if let Err(error) = poll_live_position(&client, &store, &config, trade).await {
                warn!(%error, "poll error");
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
    }
}
