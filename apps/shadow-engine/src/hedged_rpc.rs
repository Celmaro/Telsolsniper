//! Hedged RPC: race N endpoints with the same request, take the first
//! successful response. Kills tail latency (public API endpoints have
//! p99 spikes of 500ms+) and gives free failover.

use anyhow::Result;
use sol_trade_sdk::common::SolanaRpcClient;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct HedgedRpc {
    clients: Vec<Arc<SolanaRpcClient>>,
}

impl HedgedRpc {
    pub fn new(urls: &[String]) -> Self {
        let clients = urls
            .iter()
            .map(|url| {
                Arc::new(SolanaRpcClient::new_with_timeout_and_commitment(
                    url.clone(),
                    Duration::from_secs(10),
                    solana_commitment_config::CommitmentConfig::confirmed(),
                ))
            })
            .collect();
        Self { clients }
    }

    pub fn primary(&self) -> &SolanaRpcClient {
        &self.clients[0]
    }

    /// Run `f` against every endpoint concurrently; first success wins.
    /// Losers are aborted when their handles drop.
    pub async fn hedge<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: Fn(Arc<SolanaRpcClient>) -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send + 'static,
    {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        for client in &self.clients {
            let tx = tx.clone();
            let f = f.clone();
            let client = Arc::clone(client);
            tokio::spawn(async move {
                if let Ok(value) = f(client).await {
                    let _ = tx.send(value);
                }
            });
        }
        drop(tx);
        match rx.recv().await {
            Some(value) => Ok(value),
            None => Err(anyhow::anyhow!("all hedged endpoints failed")),
        }
    }
}
