use crate::swqos::common::{default_http_client_builder, poll_transaction_confirmation};
use rand::seq::IndexedRandom;
use reqwest::Client;
use std::{sync::Arc, time::Instant};

use std::time::Duration;
use anyhow::Result;
use bincode::serialize as bincode_serialize;
use solana_client::rpc_client::SerializableTransaction;
use solana_sdk::transaction::VersionedTransaction;
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;

use crate::{common::SolanaRpcClient, constants::swqos::ASTRALANE_TIP_ACCOUNTS};

use tokio::task::JoinHandle;
use std::sync::atomic::{AtomicBool, Ordering};

/// Empty body for getHealth POST; avoid per-request allocation.
static PING_BODY: &[u8] = &[];

#[derive(Clone)]
pub struct AstralaneClient {
    pub endpoint: String,
    pub auth_token: String,
    pub rpc_client: Arc<SolanaRpcClient>,
    pub http_client: Client,
    pub ping_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
    pub stop_ping: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl SwqosClientTrait for AstralaneClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction, wait_confirmation: bool) -> Result<()> {
        self.send_transaction(trade_type, transaction, wait_confirmation).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>, wait_confirmation: bool) -> Result<()> {
        self.send_transactions(trade_type, transactions, wait_confirmation).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *ASTRALANE_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| ASTRALANE_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Astralane
    }
}

impl AstralaneClient {
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        let rpc_client = SolanaRpcClient::new(rpc_url);
        let http_client = default_http_client_builder().build().unwrap();
        
        let client = Self { 
            rpc_client: Arc::new(rpc_client), 
            endpoint, 
            auth_token, 
            http_client,
            ping_handle: Arc::new(tokio::sync::Mutex::new(None)),
            stop_ping: Arc::new(AtomicBool::new(false)),
        };
        
        // Start ping task
        let client_clone = client.clone();
        tokio::spawn(async move {
            client_clone.start_ping_task().await;
        });
        
        client
    }

    /// Start periodic ping task to keep connections active
    async fn start_ping_task(&self) {
        let endpoint = self.endpoint.clone();
        let auth_token = self.auth_token.clone();
        let http_client = self.http_client.clone();
        let stop_ping = self.stop_ping.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await; // first tick completes immediately → one ping at start
                if stop_ping.load(Ordering::Relaxed) {
                    break;
                }
                if let Err(e) = Self::send_ping_request(&http_client, &endpoint, &auth_token).await {
                    eprintln!("Astralane ping request failed: {}", e);
                }
            }
        });
        
        // Update ping_handle - use Mutex to safely update
        {
            let mut ping_guard = self.ping_handle.lock().await;
            if let Some(old_handle) = ping_guard.as_ref() {
                old_handle.abort();
            }
            *ping_guard = Some(handle);
        }
    }

    /// Send ping request: POST endpoint?api-key=...&method=getHealth (endpoint is irisb from constants).
    async fn send_ping_request(http_client: &Client, endpoint: &str, auth_token: &str) -> Result<()> {
        let response = http_client
            .post(endpoint)
            .query(&[("api-key", auth_token), ("method", "getHealth")])
            .timeout(Duration::from_millis(1500))
            .body(PING_BODY)
            .send()
            .await?;
        let status = response.status();
        let _ = response.bytes().await; // consume body so connection returns to pool
        if !status.is_success() {
            eprintln!("Astralane ping request returned non-success status: {}", status);
        }
        Ok(())
    }

    /// Send transaction via /irisb binary API (no Base64; lower latency).
    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction, wait_confirmation: bool) -> Result<()> {
        let start_time = Instant::now();
        let signature = transaction.get_signature();

        let body_bytes = bincode_serialize(transaction).map_err(|e| anyhow::anyhow!("Astralane binary serialize failed: {}", e))?;

        let response = self.http_client
            .post(&self.endpoint)
            .query(&[("api-key", self.auth_token.as_str()), ("method", "sendTransaction")])
            .header("Content-Type", "application/octet-stream")
            .body(body_bytes)
            .send()
            .await?;

        let status = response.status();
        let _ = response.bytes().await;
        if status.is_success() {
            println!(" [astralane] {} submitted: {:?}", trade_type, start_time.elapsed());
        } else {
            eprintln!(" [astralane] {} submission failed: status {}", trade_type, status);
            return Err(anyhow::anyhow!("Astralane sendTransaction failed: {}", status));
        }

        let start_time = Instant::now();
        match poll_transaction_confirmation(&self.rpc_client, *signature, wait_confirmation).await {
            Ok(_) => (),
            Err(e) => {
                println!(" signature: {:?}", signature);
                println!(" [astralane] {} confirmation failed: {:?}", trade_type, start_time.elapsed());
                return Err(e);
            },
        }
        if wait_confirmation {
            println!(" signature: {:?}", signature);
            println!(" [astralane] {} confirmed: {:?}", trade_type, start_time.elapsed());
        }

        Ok(())
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>, wait_confirmation: bool) -> Result<()> {
        for transaction in transactions {
            self.send_transaction(trade_type, transaction, wait_confirmation).await?;
        }
        Ok(())
    }
}

impl Drop for AstralaneClient {
    fn drop(&mut self) {
        // Ensure ping task stops when client is destroyed
        self.stop_ping.store(true, Ordering::Relaxed);
        
        // Try to stop ping task immediately
        // Use tokio::spawn to avoid blocking Drop
        let ping_handle = self.ping_handle.clone();
        tokio::spawn(async move {
            let mut ping_guard = ping_handle.lock().await;
            if let Some(handle) = ping_guard.as_ref() {
                handle.abort();
            }
            *ping_guard = None;
        });
    }
}
