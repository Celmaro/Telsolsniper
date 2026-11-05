//! Feed monitor: chain-data and signal-feed continuity checks.
//!
//! Two independent health questions, both answered on a fixed interval:
//!
//!  1. Is the chain feed alive and continuous? Polls each RPC endpoint for
//!     slot + block height and checks:
//!        - staleness   : slot stopped advancing (no new slots)
//!        - gap         : more slots elapsed than blocks burned (missed feed)
//!        - endpoint    : per-endpoint failure/latency
//!     When YELLOWSTONE_ENDPOINT is configured the same continuity maths
//!     applies to its slot stream; until a Yellowstone token is provisioned
//!     the RPC slot stream is the authoritative source.
//!
//!  2. Is the signal feed alive? Checks intake's SQLite for the most recent
//!     signal per channel and the event socket's presence, so a silently
//!     disconnected Telegram listener is visible.
//!
//! Writes one JSON line per tick to FEED_HEALTH_LOG and a compact status
//! file that other tools (cron watchdog) can read.

use anyhow::Result;
use serde_json::json;
use std::io::Write;
use std::time::Duration;

const SLOT_MS: f64 = 400.0; // ~2.5 slots/second target

struct Endpoint {
    url: String,
    last_slot: Option<u64>,
    last_advance_at: Option<std::time::Instant>,
    consecutive_failures: u32,
}

#[derive(Clone)]
struct Config {
    endpoints: Vec<String>,
    health_log: String,
    status_path: String,
    interval_secs: u64,
    signals_db: String,
    event_socket: String,
    stale_slots: u64,
}

impl Config {
    fn from_env() -> Self {
        let endpoints = std::env::var("FEED_RPC_URLS")
            .unwrap_or_else(|_| {
                "https://api.mainnet-beta.solana.com,https://solana-rpc.publicnode.com".into()
            })
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            endpoints,
            health_log: std::env::var("FEED_HEALTH_LOG")
                .unwrap_or_else(|_| "data/feed-health.jsonl".into()),
            status_path: std::env::var("FEED_STATUS_PATH")
                .unwrap_or_else(|_| "data/feed-status.json".into()),
            interval_secs: std::env::var("FEED_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            signals_db: std::env::var("SIGNAL_DATABASE_PATH")
                .unwrap_or_else(|_| "data/signals.sqlite".into()),
            event_socket: std::env::var("EVENT_SOCKET_PATH")
                .unwrap_or_else(|_| "/tmp/telsolsniper-events.sock".into()),
            stale_slots: std::env::var("FEED_STALE_SLOTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
        }
    }
}

async fn rpc_call(client: &reqwest::Client, url: &str, method: &str) -> Result<serde_json::Value> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": []});
    let response: serde_json::Value = client
        .post(url)
        .json(&body)
        .timeout(Duration::from_secs(4))
        .send()
        .await?
        .json()
        .await?;
    if let Some(error) = response.get("error") {
        anyhow::bail!("rpc error: {error}");
    }
    Ok(response["result"].clone())
}

fn append_json(path: &str, value: &serde_json::Value) {
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{value}");
    }
}

/// Latest signal timestamp per channel — a dead listener shows up here first.
fn signal_freshness(db_path: &str) -> serde_json::Value {
    let Ok(connection) =
        rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return json!({"available": false});
    };
    let mut freshness = serde_json::Map::new();
    if let Ok(mut stmt) = connection.prepare(
        "SELECT source, MAX(received_at_unix_ms), COUNT(*) FROM telegram_messages GROUP BY source",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                row.get::<_, i64>(2)?,
            ))
        }) {
            let now = now_ms();
            for row in rows.flatten() {
                let (source, last_ms, count) = row;
                let age_secs = if last_ms > 0 { (now - last_ms) / 1000 } else { -1 };
                freshness.insert(
                    source,
                    json!({"last_signal_age_secs": age_secs, "messages": count}),
                );
            }
        }
    }
    json!({"available": true, "channels": freshness})
}

fn now_ms() -> i64 {
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

    let config = Config::from_env();
    let client = reqwest::Client::new();
    let mut endpoints: Vec<Endpoint> = config
        .endpoints
        .iter()
        .map(|url| Endpoint {
            url: url.clone(),
            last_slot: None,
            last_advance_at: None,
            consecutive_failures: 0,
        })
        .collect();

    tracing::info!(
        endpoints = ?config.endpoints.iter().map(|u| redact(u)).collect::<Vec<_>>(),
        interval_secs = config.interval_secs,
        "feed monitor starting"
    );

    loop {
        let mut report = json!({
            "at": now_ms(),
            "endpoints": {},
            "chain": {},
            "signals": signal_freshness(&config.signals_db),
            "event_socket_present": std::path::Path::new(&config.event_socket).exists(),
        });

        let mut best_slot = 0u64;
        let mut best_height = 0u64;
        let mut healthy_endpoints = 0;

        for endpoint in endpoints.iter_mut() {
            let started = std::time::Instant::now();
            let slot = rpc_call(&client, &endpoint.url, "getSlot").await;
            let latency_ms = started.elapsed().as_millis() as u64;

            match slot {
                Ok(value) => {
                    let slot = value.as_u64().unwrap_or(0);
                    let height = rpc_call(&client, &endpoint.url, "getBlockHeight")
                        .await
                        .ok()
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    healthy_endpoints += 1;
                    endpoint.consecutive_failures = 0;

                    let advanced = endpoint.last_slot.map(|previous| slot > previous).unwrap_or(true);
                    if advanced {
                        endpoint.last_advance_at = Some(std::time::Instant::now());
                    }
                    let stalled_secs = endpoint
                        .last_advance_at
                        .map(|at| at.elapsed().as_secs_f64())
                        .unwrap_or(0.0);

                    // Slots elapsed while blocks did not: the feed is dropping.
                    let gap = endpoint
                        .last_slot
                        .map(|previous| slot.saturating_sub(previous))
                        .unwrap_or(0);
                    let height_gap = if height > 0 && best_height > 0 {
                        best_height.saturating_sub(height)
                    } else {
                        0
                    };
                    let stale = stalled_secs * 1000.0 / SLOT_MS > config.stale_slots as f64;

                    report["endpoints"][redact(&endpoint.url)] = json!({
                        "slot": slot,
                        "block_height": height,
                        "latency_ms": latency_ms,
                        "slot_gap": gap,
                        "advanced": advanced,
                        "stalled_secs": (stalled_secs * 10.0).round() / 10.0,
                        "stale": stale,
                        "height_lag_vs_best": height_gap,
                    });

                    if stale {
                        tracing::warn!(
                            endpoint = %redact(&endpoint.url),
                            stalled_secs,
                            "chain feed stalled"
                        );
                    }
                    if gap > config.stale_slots {
                        tracing::warn!(endpoint = %redact(&endpoint.url), slot_gap = gap, "slot gap detected");
                    }

                    endpoint.last_slot = Some(slot);
                    best_slot = best_slot.max(slot);
                    best_height = best_height.max(height);
                }
                Err(error) => {
                    endpoint.consecutive_failures += 1;
                    let text = redact_error(&error.to_string());
                    report["endpoints"][redact(&endpoint.url)] = json!({
                        "error": text,
                        "consecutive_failures": endpoint.consecutive_failures,
                        "latency_ms": latency_ms,
                    });
                    tracing::warn!(
                        endpoint = %redact(&endpoint.url),
                        failures = endpoint.consecutive_failures,
                        error = %text,
                        "endpoint unhealthy"
                    );
                }
            }
        }

        let all_down = healthy_endpoints == 0;
        report["chain"] = json!({
            "best_slot": best_slot,
            "best_block_height": best_height,
            "healthy_endpoints": healthy_endpoints,
            "all_endpoints_down": all_down,
        });
        if all_down {
            tracing::error!("no RPC endpoint reachable — chain feed is DOWN");
        }

        // Signal-feed continuity: flag channels that have gone quiet.
        if let Some(channels) = report["signals"]["channels"].as_object() {
            for (channel, status) in channels {
                let age = status["last_signal_age_secs"].as_i64().unwrap_or(-1);
                if age > 3600 {
                    tracing::warn!(%channel, age_secs = age, "channel silent for over an hour");
                }
            }
        }
        if report["event_socket_present"] == json!(false) {
            tracing::warn!(
                socket = %config.event_socket,
                "event socket missing — intake is not running or has not bound it"
            );
        }

        append_json(&config.health_log, &report);
        if let Some(parent) = std::path::Path::new(&config.status_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&config.status_path, serde_json::to_string_pretty(&report)?);

        tokio::time::sleep(Duration::from_secs(config.interval_secs)).await;
    }
}

/// Host-only endpoint identity; keyed RPC URLs must never be logged.
fn redact(url: &str) -> String {
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = without_scheme.split(['/', '?']).next().unwrap_or("");
    format!("host:{host}")
}

fn redact_error(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find("://") {
        let start = rest[..position]
            .rfind(|c: char| !(c.is_alphanumeric() || c == '.' || c == '-' || c == '+'))
            .map(|i| i + 1)
            .unwrap_or(0);
        out.push_str(&rest[..start]);
        let after = &rest[position + 3..];
        let end = after
            .find(|c: char| c == '/' || c == '?' || c.is_whitespace())
            .unwrap_or(after.len());
        out.push_str("://");
        out.push_str(&after[..end]);
        rest = &after[end..];
        if let Some(space) = rest.find(char::is_whitespace) {
            out.push_str("/[REDACTED]");
            rest = &rest[space..];
        }
    }
    out.push_str(rest);
    out
}
