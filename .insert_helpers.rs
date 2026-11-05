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
async fn reconcile_loop(
    store: Arc<LiveStore>,
    config: Arc<LiveConfig>,
    wallet: Pubkey,
) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(config.reconcile_interval_secs)).await;
        let orders = match store.unsettled_orders(20_000) {
            Ok(orders) => orders,
            Err(error) => {
                warn!(error = %redact::redact_error(&error.to_string(), &[]), "journal read failed");
                continue;
            }
        };
        for order in orders {
            match order.signature.as_deref() {
                None => {
                    // Never submitted (crashed before send). A buy intent is
                    // safe to retire; a sell without a signature is genuinely
                    // unknown and needs a human.
                    let note = if order.side == "buy" {
                        "intent recorded but never submitted"
                    } else {
                        "no signature recorded; submit status unknown"
                    };
                    if let Err(error) = store.journal_unknown(order.id, note) {
                        warn!(%error, "journal update failed");
                    }
                    if order.side == "sell" {
                        error!(
                            mint = %order.mint,
                            kind = %order.kind,
                            "UNKNOWN sell order — trade requires operator review"
                        );
                    }
                }
                Some(signature) => match tx_outcome(&config.rpc_url, signature, &wallet).await {
                    Ok(Some((true, delta, _))) => {
                        if let Err(error) = store.journal_confirmed(order.id, delta.abs(), order.tokens) {
                            warn!(%error, "journal update failed");
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
                        // Not visible yet: stay pending. After 5 minutes the
                        // cluster has dropped it, so it is genuinely unknown.
                        if now_ms() - order.created_ms > 300_000 {
                            if let Err(error) = store.journal_unknown(order.id, "not found after 5m") {
                                warn!(%error, "journal update failed");
                            }
                            error!(
                                signature = %signature, mint = %order.mint,
                                "UNKNOWN order — resolve manually before trusting PnL"
                            );
                        }
                    }
                    Err(error) => warn!(
                        error = %redact::redact_error(&error.to_string(), &[]),
                        "reconcile RPC error"
                    ),
                },
            }
        }
    }
}

/// Independent exit task: position management never shares a loop with entry,
/// so a slow buy or a blocked RPC call cannot delay a stop.
async fn exit_loop(store: Arc<LiveStore>, client: Arc<SolanaTrade>, config: Arc<LiveConfig>) {
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
        for trade in trades {
            if let Err(error) = poll_live_position(&client, &store, &config, trade).await {
                warn!(
                    error = %redact::redact_error(&error.to_string(), &[config.private_key.as_str()]),
                    "exit evaluation error"
                );
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
    }
}

