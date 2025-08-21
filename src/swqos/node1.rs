use crate::swqos::common::{poll_transaction_confirmation, serialize_transaction_and_encode};
use rand::seq::IndexedRandom;
use reqwest::Client;
use serde_json::json;
use std::{sync::Arc, time::Instant};

use std::time::Duration;
use solana_transaction_status::UiTransactionEncoding;

use anyhow::Result;
use solana_sdk::transaction::VersionedTransaction;
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;

use crate::{common::SolanaRpcClient, constants::swqos::NODE1_TIP_ACCOUNTS};

use tokio::task::JoinHandle;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
pub struct Node1Client {
    pub endpoint: String,
    pub auth_token: String,
    pub rpc_client: Arc<SolanaRpcClient>,
    pub http_client: Client,
    pub ping_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
    pub stop_ping: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl SwqosClientTrait for Node1Client {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<()> {
        self.send_transaction(trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<()> {
        self.send_transactions(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *NODE1_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| NODE1_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Node1
    }
}

impl Node1Client {
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        let rpc_client = SolanaRpcClient::new(rpc_url);
        let http_client = Client::builder()
            // 由于有 ping 机制，可以延长连接池空闲超时
            .pool_idle_timeout(Duration::from_secs(300)) // 5分钟，比 ping 间隔更长
            .pool_max_idle_per_host(32) // 减少连接数，因为连接会更稳定
            // TCP keepalive 可以设置得更长，因为 ping 会主动保持连接
            .tcp_keepalive(Some(Duration::from_secs(300))) // 5分钟
            // HTTP/2 keepalive 间隔可以更长
            .http2_keep_alive_interval(Duration::from_secs(30)) // 30秒
            // 请求超时可以适当延长，因为连接更稳定
            .timeout(Duration::from_secs(15)) // 15秒
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        
        let client = Self { 
            rpc_client: Arc::new(rpc_client), 
            endpoint, 
            auth_token, 
            http_client,
            ping_handle: Arc::new(tokio::sync::Mutex::new(None)),
            stop_ping: Arc::new(AtomicBool::new(false)),
        };
        
        // 启动 ping 任务
        let client_clone = client.clone();
        tokio::spawn(async move {
            client_clone.start_ping_task().await;
        });
        
        client
    }

    /// 启动定期 ping 任务以保持连接活跃
    async fn start_ping_task(&self) {
        let endpoint = self.endpoint.clone();
        let auth_token = self.auth_token.clone();
        let http_client = self.http_client.clone();
        let stop_ping = self.stop_ping.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // 每60秒ping一次
            
            loop {
                interval.tick().await;
                
                if stop_ping.load(Ordering::Relaxed) {
                    break;
                }
                
                // 发送 ping 请求
                if let Err(e) = Self::send_ping_request(&http_client, &endpoint, &auth_token).await {
                    eprintln!("Node1 ping 请求失败: {}", e);
                }
            }
        });
        
        // 更新 ping_handle - 使用 Mutex 来安全地更新
        {
            let mut ping_guard = self.ping_handle.lock().await;
            if let Some(old_handle) = ping_guard.as_ref() {
                old_handle.abort();
            }
            *ping_guard = Some(handle);
        }
    }

    /// 发送 ping 请求到 /ping 端点
    async fn send_ping_request(http_client: &Client, endpoint: &str, _auth_token: &str) -> Result<()> {
        // 构建 ping URL
        let ping_url = if endpoint.ends_with('/') {
            format!("{}ping", endpoint)
        } else {
            format!("{}/ping", endpoint)
        };

        // 发送 GET 请求到 /ping 端点（不需要 api-key）
        let response = http_client.get(&ping_url)
            .send()
            .await?;
        
        if response.status().is_success() {
            // ping 成功，连接保持活跃
            // 可以选择性地记录日志，但为了减少噪音，这里不打印
        } else {
            eprintln!("Node1 ping 请求返回非成功状态: {}", response.status());
        }
        
        Ok(())
    }

    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<()> {
        let start_time = Instant::now();
        let (content, signature) = serialize_transaction_and_encode(transaction, UiTransactionEncoding::Base64).await?;
        println!(" 交易编码base64: {:?}", start_time.elapsed());

        let request_body = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                content,
                { "encoding": "base64", "skipPreflight": true }
            ]
        }))?;

        // Node1使用api-key header而不是URL参数
        let response_text = self.http_client.post(&self.endpoint)
            .body(request_body)
            .header("Content-Type", "application/json")
            .header("api-key", &self.auth_token)
            .send()
            .await?
            .text()
            .await?;

        // 解析JSON响应
        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if response_json.get("result").is_some() {
                println!(" node1{}提交: {:?}", trade_type, start_time.elapsed());
            } else if let Some(_error) = response_json.get("error") {
                eprintln!(" node1{}提交失败: {:?}", trade_type, _error);
            }
        }

        let start_time: Instant = Instant::now();
        match poll_transaction_confirmation(&self.rpc_client, signature).await {
            Ok(_) => (),
            Err(_) => (),
        }

        println!(" node1{}确认: {:?}", trade_type, start_time.elapsed());

        Ok(())
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<()> {
        for transaction in transactions {
            self.send_transaction(trade_type, transaction).await?;
        }
        Ok(())
    }
}

impl Drop for Node1Client {
    fn drop(&mut self) {
        // 确保在客户端被销毁时停止 ping 任务
        self.stop_ping.store(true, Ordering::Relaxed);
        
        // 尝试立即停止 ping 任务
        // 使用 tokio::spawn 来避免阻塞 Drop
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
