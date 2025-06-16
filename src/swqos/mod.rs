use api::api_client::ApiClient;
use common::{poll_transaction_confirmation, serialize_smart_transaction_and_encode, serialize_transaction_and_encode};
use solana_client::rpc_config::RpcSendTransactionConfig;
use crate::swqos::jito_grpc::searcher::searcher_service_client::SearcherServiceClient;
use reqwest::Client;
use searcher_client::{get_searcher_client_no_auth, send_bundle_with_confirmation};
use serde_json::json;
use tonic::transport::Channel;
use yellowstone_grpc_client::Interceptor;
use std::{sync::Arc, time::Instant};
use tokio::sync::{Mutex, RwLock};

use solana_sdk::{commitment_config::CommitmentLevel, signature::Signature};

use std::str::FromStr;
use rustls::crypto::{ring::default_provider, CryptoProvider};

use tonic::{service::interceptor::InterceptedService, transport::Uri, Status};         
use std::time::Duration;
use solana_transaction_status::UiTransactionEncoding;
use tonic::transport::ClientTlsConfig;

use anyhow::{anyhow, Result};
use rand::{rng, seq::{IndexedRandom, IteratorRandom}};
use solana_sdk::transaction::VersionedTransaction;

use crate::{common::SolanaRpcClient, constants::pumpfun::accounts::{JITO_TIP_ACCOUNTS, NEXTBLOCK_TIP_ACCOUNTS, ZEROSLOT_TIP_ACCOUNTS, NOZOMI_TIP_ACCOUNTS}};

pub mod api;
pub mod common;
pub mod searcher_client;
pub mod jito_grpc;

lazy_static::lazy_static! {
    static ref TIP_ACCOUNT_CACHE: RwLock<Vec<String>> = RwLock::new(Vec::new());
}

#[derive(Debug, Clone, Copy)]
pub enum TradeType {
    Create,
    CreateAndBuy,
    Buy,
    Sell,
}

impl std::fmt::Display for TradeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TradeType::Create => "创建",
            TradeType::CreateAndBuy => "创建并买入",
            TradeType::Buy => "买入",
            TradeType::Sell => "卖出",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClientType {
    Jito,
    NextBlock,
    ZeroSlot,
    Nozomi,
    Rpc,
}

pub type FeeClient = dyn FeeClientTrait + Send + Sync + 'static;

#[async_trait::async_trait]
pub trait FeeClientTrait {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature>;
    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>>;
    fn get_tip_account(&self) -> Result<String>;
    fn get_client_type(&self) -> ClientType;
}

#[derive(Clone)]
pub struct SolRpcClient {
    pub rpc_client: Arc<SolanaRpcClient>,
}

#[async_trait::async_trait]
impl FeeClientTrait for SolRpcClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature> {
        let signature = self.rpc_client.send_transaction_with_config(transaction, RpcSendTransactionConfig{
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Processed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(3),
            min_context_slot: Some(0),
        }).await?;

        let start_time = Instant::now();
        match poll_transaction_confirmation(&self.rpc_client, signature).await {
            Ok(_) => (),
            Err(_) => (),
        }
        println!(" signature: {:?}", signature);
        println!(" rpc{}确认: {:?}", trade_type, start_time.elapsed());

        Ok(signature)
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>> {
        let mut signatures = Vec::new();
        for transaction in transactions {
            let signature = self.send_transaction(trade_type, transaction).await?;
            signatures.push(signature);
        }
        Ok(signatures)
    }

    fn get_tip_account(&self) -> Result<String> {
        Ok("".to_string())
    }

    fn get_client_type(&self) -> ClientType {
        ClientType::Rpc
    }
}

impl SolRpcClient {
    pub fn new(rpc_client: Arc<SolanaRpcClient>) -> Self {
        Self { rpc_client }
    }
}

pub struct JitoClient {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub searcher_client: Arc<Mutex<SearcherServiceClient<Channel>>>,
}

#[async_trait::async_trait]
impl FeeClientTrait for JitoClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature, anyhow::Error> {
        self.send_bundle_with_confirmation(trade_type, &vec![transaction.clone()]).await?.first().cloned().ok_or(anyhow!("Failed to send transaction"))
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>, anyhow::Error> {
        self.send_bundle_with_confirmation(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String, anyhow::Error> {
        if let Some(acc) = JITO_TIP_ACCOUNTS.iter().choose(&mut rng()) {
            Ok(acc.to_string())
        } else {
            Err(anyhow!("no valid tip accounts found"))
        }
    }

    fn get_client_type(&self) -> ClientType {
        ClientType::Jito
    }
}

impl JitoClient {
    pub async fn new(rpc_url: String, block_engine_url: String) -> Result<Self> {
        let rpc_client = SolanaRpcClient::new(rpc_url);
        let searcher_client = get_searcher_client_no_auth(block_engine_url.as_str()).await?;
        Ok(Self { rpc_client: Arc::new(rpc_client), searcher_client: Arc::new(Mutex::new(searcher_client)) })
    }
    
    pub async fn send_bundle_with_confirmation(
        &self,
        trade_type: TradeType,
        transactions: &Vec<VersionedTransaction>,
    ) -> Result<Vec<Signature>, anyhow::Error> {
        send_bundle_with_confirmation(self.rpc_client.clone(), trade_type, &transactions, self.searcher_client.clone()).await
    }

    pub async fn send_bundle_no_wait(
        &self,
        transactions: &Vec<VersionedTransaction>,
    ) -> Result<Vec<Signature>, anyhow::Error> {
        searcher_client::send_bundle_no_wait(&transactions, self.searcher_client.clone()).await
    }
}

#[derive(Clone)]
pub struct MyInterceptor {
    auth_token: String,
}

impl MyInterceptor {
    pub fn new(auth_token: String) -> Self {
        Self { auth_token }
    }
}

impl Interceptor for MyInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        request.metadata_mut().insert(
            "authorization", 
            tonic::metadata::MetadataValue::from_str(&self.auth_token)
                .map_err(|_| Status::invalid_argument("Invalid auth token"))?
        );
        Ok(request)
    }
}

#[derive(Clone)]
pub struct NextBlockClient {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub client: ApiClient<InterceptedService<Channel, MyInterceptor>>,
}

#[async_trait::async_trait]
impl FeeClientTrait for NextBlockClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature, anyhow::Error> {
        self.send_transaction(trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>, anyhow::Error> {
        self.send_transactions(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *NEXTBLOCK_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| NEXTBLOCK_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_client_type(&self) -> ClientType {
        ClientType::NextBlock
    }
}

impl NextBlockClient {
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        if CryptoProvider::get_default().is_none() {
            let _ = default_provider()
                .install_default()
                .map_err(|e| anyhow::anyhow!("Failed to install crypto provider: {:?}", e));
        }

        let endpoint = endpoint.parse::<Uri>().unwrap();
        let tls = ClientTlsConfig::new().with_native_roots();
        let channel = Channel::builder(endpoint)
            .tls_config(tls).expect("Failed to create TLS config")
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .http2_keep_alive_interval(Duration::from_secs(30))
            .keep_alive_while_idle(true)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .connect_lazy();

        let client = ApiClient::with_interceptor(channel, MyInterceptor::new(auth_token));
        let rpc_client = SolanaRpcClient::new(rpc_url);
        Self { rpc_client: Arc::new(rpc_client), client }
    }

    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature, anyhow::Error> {
        let start_time = Instant::now();
        let (content, signature) = serialize_smart_transaction_and_encode(transaction, UiTransactionEncoding::Base64).await?;
        
        self.client.clone().post_submit_v2(api::PostSubmitRequest {
            transaction: Some(api::TransactionMessage {
                content,
                is_cleanup: false,
            }),
            skip_pre_flight: true,
            front_running_protection: Some(true),
            experimental_front_running_protection: Some(true),
            snipe_transaction: Some(true),
        }).await?;

        println!(" nextblock{}提交: {:?}", trade_type, start_time.elapsed());

        let start_time: Instant = Instant::now();
        let timeout: Duration = Duration::from_secs(10);
        while Instant::now().duration_since(start_time) < timeout {
            match poll_transaction_confirmation(&self.rpc_client, signature).await {
                Ok(_) => break,
                Err(_) => continue,
            }
        }

        println!(" nextblock{}确认: {:?}", trade_type, start_time.elapsed());

        Ok(signature)
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>, anyhow::Error> {
        let mut entries = Vec::new();
        let encoding = UiTransactionEncoding::Base64;
        
        let mut signatures = Vec::new();
        for transaction in transactions {
            let (content, signature) = serialize_smart_transaction_and_encode(transaction, encoding).await?;
            entries.push(api::PostSubmitRequestEntry {
                transaction: Some(api::TransactionMessage {
                    content,
                    is_cleanup: false,
                }),
                skip_pre_flight: true,
            });
            signatures.push(signature);
        }

        self.client.clone().post_submit_batch_v2(api::PostSubmitBatchRequest {
            entries,
            submit_strategy: api::SubmitStrategy::PSubmitAll as i32,
            use_bundle: Some(true),
            front_running_protection: Some(true),
        }).await?;

        let start_time: Instant = Instant::now();
        for signature in signatures.clone() {
            match poll_transaction_confirmation(&self.rpc_client, signature).await {
                Ok(_) => continue,
                Err(_) => continue,
            }
        }

        println!(" nextblock{}确认: {:?}", trade_type, start_time.elapsed());
        
        Ok(signatures)
    }
}

#[derive(Clone)]
pub struct ZeroSlotClient {
    pub endpoint: String,
    pub auth_token: String,
    pub rpc_client: Arc<SolanaRpcClient>,
    pub http_client: Client,
}

#[async_trait::async_trait]
impl FeeClientTrait for ZeroSlotClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature, anyhow::Error> {
        self.send_transaction(trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>, anyhow::Error> {
        self.send_transactions(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *ZEROSLOT_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| ZEROSLOT_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_client_type(&self) -> ClientType {
        ClientType::ZeroSlot
    }
}

impl ZeroSlotClient {
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        let rpc_client = SolanaRpcClient::new(rpc_url);
        let http_client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(64)
            .tcp_keepalive(Some(Duration::from_secs(1200)))
            .http2_keep_alive_interval(Duration::from_secs(15))
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        Self { rpc_client: Arc::new(rpc_client), endpoint, auth_token, http_client }
    }

    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature, anyhow::Error> {
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

        let mut url = String::with_capacity(self.endpoint.len() + self.auth_token.len() + 20);
        url.push_str(&self.endpoint);
        url.push_str("/?api-key=");
        url.push_str(&self.auth_token);

        // 4. 直接使用 `text().await?`，避免 `json().await?` 的异步 JSON 解析
        let response_text = self.http_client.post(&url)
            .body(request_body) // 直接传字符串，避免 `json()` 开销
            .header("Content-Type", "application/json") // 显式指定 JSON 头
            .send()
            .await?
            .text()
            .await?;

        // 5. 用 `serde_json::from_str()` 解析 JSON，减少 `.json().await?` 额外等待
        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if response_json.get("result").is_some() {
                println!(" 0slot{}提交: {:?}", trade_type, start_time.elapsed());
            } else if let Some(_error) = response_json.get("error") {
                eprintln!(" 0slot{}提交失败: {:?}", trade_type, _error);
            }
        }

        let start_time: Instant = Instant::now();
        match poll_transaction_confirmation(&self.rpc_client, signature).await {
            Ok(_) => (),
            Err(_) => (),
        }

        println!(" 0slot{}确认: {:?}", trade_type, start_time.elapsed());

        Ok(signature)
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>, anyhow::Error> {
        let mut signatures = Vec::new();
        for transaction in transactions {
            let signature = self.send_transaction(trade_type, transaction).await?;
            signatures.push(signature);
        }
        Ok(signatures)
    }
}

#[derive(Clone)]
pub struct NozomiClient {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub endpoint: String,
    pub auth_token: String,
    pub http_client: Client,
}

#[async_trait::async_trait]
impl FeeClientTrait for NozomiClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature, anyhow::Error> {
        self.send_transaction(trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>, anyhow::Error> {
        self.send_transactions(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *NOZOMI_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| NOZOMI_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_client_type(&self) -> ClientType {
        ClientType::Nozomi
    }
}

impl NozomiClient {
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        let rpc_client = SolanaRpcClient::new(rpc_url);
        let http_client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(64)
            .tcp_keepalive(Some(Duration::from_secs(1200)))
            .http2_keep_alive_interval(Duration::from_secs(15))
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        Self { rpc_client: Arc::new(rpc_client), endpoint, auth_token, http_client }
    }

    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature, anyhow::Error> {
        let start_time = Instant::now();
        let (content, signature) = serialize_transaction_and_encode(transaction, UiTransactionEncoding::Base64).await?;
        println!(" 交易编码base64: {:?}", start_time.elapsed());

        // 按照 Nozomi 文档要求构建请求体
        let request_body = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                content,
                { "encoding": "base64" }
            ]
        }))?;

        let mut url = String::with_capacity(self.endpoint.len() + self.auth_token.len() + 20);
        url.push_str(&self.endpoint);
        url.push_str("/?c=");
        url.push_str(&self.auth_token);

        let response_text = self.http_client.post(&url)
            .body(request_body)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .text()
            .await?;

        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if response_json.get("result").is_some() {
                println!(" nozomi{}提交: {:?}", trade_type, start_time.elapsed());
            } else if let Some(_error) = response_json.get("error") {
                // eprintln!("nozomi交易提交失败: {:?}", _error);
            }
        }

        let start_time: Instant = Instant::now();
        match poll_transaction_confirmation(&self.rpc_client, signature).await {
            Ok(_) => (),
            Err(_) => (),
        }

        println!(" nozomi{}确认: {:?}", trade_type, start_time.elapsed());

        Ok(signature)
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>, anyhow::Error> {
        let mut signatures = Vec::new();
        for transaction in transactions {
            let signature = self.send_transaction(trade_type, transaction).await?;
            signatures.push(signature);
        }
        Ok(signatures)
    }
}