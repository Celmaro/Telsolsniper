//! Live price feed from pumpapi.io's free WebSocket stream.
//!
//! Why: exits currently poll an RPC every 2s for the pool price. That 2s is
//! the dominant exit latency and it is the reason a trailing stop gives back
//! ~42% off the peak instead of the 35% it intends. This stream pushes every
//! trade with `priceSol` already computed, so the exit loop can read the latest
//! price from memory (microseconds) and refresh from RPC only as a fallback.
//!
//! Scope: read-only. No key, no funds, no orders — we only consume prices.
//! Filtering happens client-side because the server sends every event.

use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

const STREAM_URL: &str = "wss://stream.pumpapi.io/";

/// rustls 0.23 refuses to connect without an installed crypto provider.
/// Called once from each entry point (probe and main).
pub fn ensure_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// mint -> (price_sol, when we saw it)
type PriceMap = HashMap<String, (f64, Instant)>;

static PRICES: OnceLock<RwLock<PriceMap>> = OnceLock::new();

fn store() -> &'static RwLock<PriceMap> {
    PRICES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Latest streamed SOL price for a mint, if it arrived recently.
/// `max_age_ms` guards against trading on a stale tick.
pub fn latest_price(mint: &str, max_age_ms: u64) -> Option<f64> {
    let guard = store().read().ok()?;
    let (price, at) = guard.get(mint)?;
    if at.elapsed() > Duration::from_millis(max_age_ms) {
        return None;
    }
    Some(*price)
}

/// When a tick last arrived for this mint — used as a wake signal.
///
/// Note: the stream's `priceSol` runs **7-22% above** the pool's true
/// token-account ratio on measured samples, so it must not be used as the
/// price. It is only a trigger: it tells the exit loop that this pool just
/// traded, so re-pricing over RPC now is worth the round trip.
pub fn last_tick(mint: &str) -> Option<Instant> {
    let guard = store().read().ok()?;
    guard.get(mint).map(|(_, at)| *at)
}

/// Number of mints currently cached — used by --price-probe.
pub fn probe_count() -> Option<usize> {
    store().read().ok().map(|g| g.len())
}

fn remember(mint: String, price: f64) {
    if let Ok(mut guard) = store().write() {
        guard.insert(mint, (price, Instant::now()));
        // Bound memory: drop anything older than 10 minutes.
        if guard.len() > 5_000 {
            guard.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(600));
        }
    }
}

/// Pull (mint, priceSol) out of a stream event. The stream emits several
/// shapes (create / trade / migration); they all carry `mint` (or
/// `baseMint`) plus a `priceSol` field at the top level.
fn extract(event: &serde_json::Value) -> Option<(String, f64)> {
    let price = event.get("priceSol")?.as_f64()?;
    if !(price > 0.0) {
        return None;
    }
    let mint = event
        .get("mint")
        .or_else(|| event.get("baseMint"))
        .and_then(|m| m.as_str())?;
    Some((mint.to_string(), price))
}

/// Persistent consumer with reconnect. Runs forever as its own task.
pub async fn run(filters: Vec<String>) {
    ensure_tls();
    let filter_set: std::collections::HashSet<String> = filters.into_iter().collect();
    let mut backoff = Duration::from_secs(1);

    loop {
        match tokio_tungstenite::connect_async(STREAM_URL).await {
            Ok((mut socket, _)) => {
                tracing::info!(url = STREAM_URL, "price stream connected");
                backoff = Duration::from_secs(1);
                let mut events: u64 = 0;

                while let Some(message) = socket.next().await {
                    let payload = match message {
                        Ok(Message::Text(text)) => text.to_string(),
                        Ok(Message::Binary(bytes)) => {
                            match std::str::from_utf8(&bytes) {
                                Ok(text) => text.to_string(),
                                Err(_) => continue,
                            }
                        }
                        Ok(_) => continue,
                        Err(error) => {
                            tracing::warn!(%error, "price stream error; reconnecting");
                            break;
                        }
                    };
                    let Ok(event) = serde_json::from_str::<serde_json::Value>(&payload) else {
                        continue;
                    };
                    // Only track mints we actually hold an open position in,
                    // when a filter list is supplied (keeps the map small).
                    if let Some((mint, price)) = extract(&event) {
                        if filter_set.is_empty() || filter_set.contains(&mint) {
                            remember(mint, price);
                            events += 1;
                        }
                    }
                }
                tracing::warn!(events, "price stream closed; reconnecting");
            }
            Err(error) => {
                tracing::warn!(%error, "price stream unavailable");
            }
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(pool: &str, mint: &str, price: f64) -> serde_json::Value {
        serde_json::json!({
            "action": "buy",
            "pool": pool,
            "mint": mint,
            "priceSol": price,
            "marketCapSol": 100.0
        })
    }

    #[test]
    fn extracts_price_and_mint_from_pump_event() {
        let (mint, price) = extract(&event("pump", "MintOne", 1.5e-6)).unwrap();
        assert_eq!(mint, "MintOne");
        assert!((price - 1.5e-6).abs() < 1e-12);
    }

    #[test]
    fn extracts_from_pumpswap_event() {
        let (mint, _) = extract(&event("pump-amm", "Graduated", 3.4e-7)).unwrap();
        assert_eq!(mint, "Graduated");
    }

    #[test]
    fn ignores_events_without_price() {
        let ev = serde_json::json!({"action": "transfer", "mint": "M"});
        assert!(extract(&ev).is_none());
    }

    #[test]
    fn ignores_zero_or_negative_price() {
        assert!(extract(&event("pump", "M", 0.0)).is_none());
        assert!(extract(&event("pump", "M", -1.0)).is_none());
    }

    #[test]
    fn falls_back_to_base_mint() {
        let ev = serde_json::json!({"baseMint": "BM", "priceSol": 2.0e-6});
        assert_eq!(extract(&ev).unwrap().0, "BM");
    }

    #[test]
    fn latest_price_respects_staleness() {
        remember("Stale".into(), 1.0);
        assert!(latest_price("Stale", 60_000).is_some());
        assert!(latest_price("Stale", 0).is_none());
        assert!(latest_price("Never", 60_000).is_none());
    }

    #[test]
    fn last_tick_is_recorded() {
        remember("Ticked".into(), 1.0);
        assert!(last_tick("Ticked").is_some());
    }
}
