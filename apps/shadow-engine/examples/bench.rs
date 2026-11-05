use sol_trade_sdk::common::SolanaRpcClient;
use sol_trade_sdk::instruction::utils::pumpfun::fetch_bonding_curve_account;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::time::Instant;

/// Latency bench: sequential vs hedged curve reads (20 samples each).
#[tokio::main]
async fn main() {
    let urls = vec![
        "https://api.mainnet-beta.solana.com".to_string(),
        "https://solana-rpc.publicnode.com".to_string(),
    ];
    let mint = Pubkey::from_str("HQf2jARrAEr3cZoLTCZgGMY4dUWAraWhsJwaj77FebNJ").unwrap();

    // sequential (single endpoint)
    let seq = SolanaRpcClient::new(urls[0].clone());
    let mut seq_ms = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let _ = fetch_bonding_curve_account(&seq, &mint).await;
        seq_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }

    // hedged (both endpoints raced)
    let clients: Vec<std::sync::Arc<SolanaRpcClient>> = urls
        .iter()
        .map(|u| std::sync::Arc::new(SolanaRpcClient::new(u.clone())))
        .collect();
    let mut hedge_ms = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        for c in &clients {
            let tx = tx.clone();
            let c = std::sync::Arc::clone(c);
            let mint = mint;
            tokio::spawn(async move {
                if fetch_bonding_curve_account(&c, &mint).await.is_ok() {
                    let _ = tx.send(());
                }
            });
        }
        drop(tx);
        let _ = rx.recv().await;
        hedge_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }

    let pct = |v: &mut Vec<f64>, p: f64| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[((v.len() as f64 - 1.0) * p).floor() as usize]
    };
    println!(
        "sequential p50 {:.0}ms p95 {:.0}ms max {:.0}ms",
        pct(&mut seq_ms, 0.5),
        pct(&mut seq_ms, 0.95),
        seq_ms.last().unwrap()
    );
    println!(
        "hedged     p50 {:.0}ms p95 {:.0}ms max {:.0}ms",
        pct(&mut hedge_ms, 0.5),
        pct(&mut hedge_ms, 0.95),
        hedge_ms.last().unwrap()
    );
}
