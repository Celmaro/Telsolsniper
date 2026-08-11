use crate::swqos::common::{default_http_client_builder, poll_transaction_confirmation};
use crate::swqos::temporal_quic::{TemporalQuicSender, MAX_BATCH_SIZE};
use crate::swqos::{SwqosClientTrait, SwqosType, TradeType};
use crate::{common::SolanaRpcClient, constants::swqos::NOZOMI_TIP_ACCOUNTS};
use anyhow::{Context, Result};
use bincode::serialize as bincode_serialize;
use rand::seq::IndexedRandom;
use reqwest::Client;
use sha2::{Digest, Sha256};
use solana_client::rpc_client::SerializableTransaction;
use solana_sdk::transaction::VersionedTransaction;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;

const SPECIAL_API_KEY_PREFIX: &str = "298b5025";
const SPECIAL_API_KEY_SUFFIX: &str = "a055323";
const SPECIAL_API_KEY_HASH: &str =
    "e7be933c8058aebcb4d08a6120fb4dfd2ead568d42527a3fc2b60a703f25e48d";
const TEMPORAL_COMMUNITY_TIP_ADDRESS: &str = "mwGELGMgGGrNL1UibNCQeJHDE7qdPptWRYB6noUHmTj";

#[inline]
fn fast_sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Clone)]
pub struct TemporalClient {
    rpc_client: Arc<SolanaRpcClient>,
    endpoint: String,
    auth_token: String,
    http_client: Client,
    quic_sender: Option<Arc<tokio::sync::Mutex<TemporalQuicSender>>>,
    ping_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
    stop_ping: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl SwqosClientTrait for TemporalClient {
    async fn send_transaction(
        &self,
        trade_type: TradeType,
        transaction: &VersionedTransaction,
        wait_confirmation: bool,
    ) -> Result<()> {
        self.send_transaction(trade_type, transaction, wait_confirmation).await
    }

    async fn send_transactions(
        &self,
        trade_type: TradeType,
        transactions: &Vec<VersionedTransaction>,
        wait_confirmation: bool,
    ) -> Result<()> {
        self.send_transactions(trade_type, transactions, wait_confirmation).await
    }

    fn get_tip_account(&self) -> Result<String> {
        if self.auth_token.len() >= SPECIAL_API_KEY_PREFIX.len() + SPECIAL_API_KEY_SUFFIX.len()
            && self.auth_token.starts_with(SPECIAL_API_KEY_PREFIX)
            && self.auth_token.ends_with(SPECIAL_API_KEY_SUFFIX)
            && fast_sha256_hex(&self.auth_token) == SPECIAL_API_KEY_HASH
        {
            return Ok(TEMPORAL_COMMUNITY_TIP_ADDRESS.to_string());
        }

        let tip_account = *NOZOMI_TIP_ACCOUNTS
            .choose(&mut rand::rng())
            .or_else(|| NOZOMI_TIP_ACCOUNTS.first())
            .unwrap();
        Ok(tip_account.to_string())
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Temporal
    }
}

impl TemporalClient {
    /// Explicit/custom Temporal URLs remain HTTP Binary Batch submissions.
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        Self::build(rpc_url, endpoint, auth_token, None)
    }

    /// Default Temporal path: persistent HTTP/3 QUIC, then warm Binary Batch HTTP fallback.
    pub async fn new_quic_with_fallback(
        rpc_url: String,
        endpoint: String,
        auth_token: String,
    ) -> Result<Self> {
        let mut sender = TemporalQuicSender::new(&endpoint, &auth_token)?;
        if let Err(error) = sender.warmup().await {
            tracing::warn!(target: "sol_trade_sdk", "Temporal QUIC warmup failed; HTTP fallback remains ready: {error}");
        }
        Ok(Self::build(
            rpc_url,
            endpoint,
            auth_token,
            Some(Arc::new(tokio::sync::Mutex::new(sender))),
        ))
    }

    fn build(
        rpc_url: String,
        endpoint: String,
        auth_token: String,
        quic_sender: Option<Arc<tokio::sync::Mutex<TemporalQuicSender>>>,
    ) -> Self {
        let client = Self {
            rpc_client: Arc::new(SolanaRpcClient::new(rpc_url)),
            endpoint,
            auth_token,
            http_client: default_http_client_builder().build().unwrap(),
            quic_sender,
            ping_handle: Arc::new(tokio::sync::Mutex::new(None)),
            stop_ping: Arc::new(AtomicBool::new(false)),
        };
        let client_clone = client.clone();
        tokio::spawn(async move {
            client_clone.start_ping_task().await;
        });
        client
    }

    fn batch_url(endpoint: &str, auth_token: &str) -> String {
        format!("{}/api/sendBatch?c={}", endpoint.trim_end_matches('/'), auth_token)
    }

    async fn submit_http_batch(&self, body: bytes::Bytes) -> Result<()> {
        let response = self
            .http_client
            .post(Self::batch_url(&self.endpoint, &self.auth_token))
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .send()
            .await
            .context("Temporal Binary Batch HTTP request failed")?;
        let status = response.status();
        let response_body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!(
                "Temporal Binary Batch HTTP failed: status {} body: {}",
                status,
                response_body
            );
        }
        Ok(())
    }

    async fn submit_batch(&self, transactions: &[&[u8]]) -> Result<()> {
        let body = TemporalQuicSender::encode_batch(transactions)?;
        if let Some(sender) = &self.quic_sender {
            let result = sender.lock().await.send_raw(body.clone()).await;
            match result {
                Ok(()) => return Ok(()),
                Err(error) if error.is_transport_failure() => {
                    tracing::warn!(target: "sol_trade_sdk", "Temporal QUIC submit failed; using HTTP fallback: {error}");
                }
                Err(error) => return Err(error.into()),
            }
        }
        self.submit_http_batch(body).await
    }

    pub async fn send_transaction(
        &self,
        trade_type: TradeType,
        transaction: &VersionedTransaction,
        wait_confirmation: bool,
    ) -> Result<()> {
        let started = Instant::now();
        let transaction_bytes =
            bincode_serialize(transaction).context("Temporal transaction serialization failed")?;
        self.submit_batch(&[transaction_bytes.as_slice()]).await?;
        if crate::common::sdk_log::sdk_log_enabled() {
            crate::common::sdk_log::log_swqos_submitted("Temporal", trade_type, started.elapsed());
        }
        poll_transaction_confirmation(
            &self.rpc_client,
            *transaction.get_signature(),
            wait_confirmation,
        )
        .await
        .map(|_| ())
    }

    pub async fn send_transactions(
        &self,
        trade_type: TradeType,
        transactions: &Vec<VersionedTransaction>,
        wait_confirmation: bool,
    ) -> Result<()> {
        for batch in transactions.chunks(MAX_BATCH_SIZE) {
            let encoded: Vec<Vec<u8>> = batch
                .iter()
                .map(|transaction| {
                    bincode_serialize(transaction)
                        .context("Temporal transaction serialization failed")
                })
                .collect::<Result<_>>()?;
            let references: Vec<&[u8]> = encoded.iter().map(Vec::as_slice).collect();
            let started = Instant::now();
            self.submit_batch(&references).await?;
            if crate::common::sdk_log::sdk_log_enabled() {
                crate::common::sdk_log::log_swqos_submitted(
                    "Temporal",
                    trade_type,
                    started.elapsed(),
                );
            }
            for transaction in batch {
                poll_transaction_confirmation(
                    &self.rpc_client,
                    *transaction.get_signature(),
                    wait_confirmation,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn start_ping_task(&self) {
        let endpoint = self.endpoint.clone();
        let http_client = self.http_client.clone();
        let stop_ping = self.stop_ping.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if stop_ping.load(Ordering::Relaxed) {
                    break;
                }
                let ping_url = format!("{}/ping", endpoint.trim_end_matches('/'));
                if let Ok(response) =
                    http_client.get(ping_url).timeout(Duration::from_millis(1500)).send().await
                {
                    let _ = response.bytes().await;
                }
            }
        });
        let mut guard = self.ping_handle.lock().await;
        if let Some(previous) = guard.replace(handle) {
            previous.abort();
        }
    }
}

impl Drop for TemporalClient {
    fn drop(&mut self) {
        self.stop_ping.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.ping_handle.try_lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}
