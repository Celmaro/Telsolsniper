//! sol-trade-sdk global log switch
//!
//! Controlled by `TradeConfig::log_enabled`, set in `TradingClient::new`.
//! All SDK logs (timing, SWQOS submit/confirm, WSOL, blacklist, etc.) should check this before output.

use std::sync::atomic::{AtomicBool, Ordering};

static SDK_LOG_ENABLED: AtomicBool = AtomicBool::new(true);

/// Whether SDK logging is enabled (set from TradeConfig.log_enabled in TradingClient::new).
#[inline(always)]
pub fn sdk_log_enabled() -> bool {
    SDK_LOG_ENABLED.load(Ordering::Relaxed)
}

/// Set the SDK global log switch (only called from TradingClient::new).
pub fn set_sdk_log_enabled(enabled: bool) {
    SDK_LOG_ENABLED.store(enabled, Ordering::Relaxed);
}
