use crate::protos::nextblock_grpc;
use crate::protos::nextblock_grpc::api_client::ApiClient;
use crate::swqos::common::{poll_transaction_confirmation, serialize_smart_transaction_and_encode};
use rand::seq::IndexedRandom;
use rustls::crypto::ring::default_provider;
use rustls::crypto::CryptoProvider;
use tonic::transport::Channel;
use yellowstone_grpc_client::Interceptor;
use std::str::FromStr;
use std::{sync::Arc, time::Instant};

use solana_sdk::{signature::Signature};

use tonic::{service::interceptor::InterceptedService, transport::Uri, Status};         
use std::time::Duration;
use solana_transaction_status::UiTransactionEncoding;
use tonic::transport::ClientTlsConfig;

use anyhow::Result;
use solana_sdk::transaction::VersionedTransaction;
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;

use crate::{common::SolanaRpcClient, constants::pumpfun::accounts::NEXTBLOCK_TIP_ACCOUNTS};


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
impl SwqosClientTrait for NextBlockClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature> {
        self.send_transaction(trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>> {
        self.send_transactions(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *NEXTBLOCK_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| NEXTBLOCK_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::NextBlock
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

    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature> {
        let start_time = Instant::now();
        let (content, signature) = serialize_smart_transaction_and_encode(transaction, UiTransactionEncoding::Base64).await?;
        
        self.client.clone().post_submit_v2(nextblock_grpc::PostSubmitRequest {
            transaction: Some(nextblock_grpc::TransactionMessage {
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

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>> {
        let mut entries = Vec::new();
        let encoding = UiTransactionEncoding::Base64;
        
        let mut signatures = Vec::new();
        for transaction in transactions {
            let (content, signature) = serialize_smart_transaction_and_encode(transaction, encoding).await?;
            entries.push(nextblock_grpc::PostSubmitRequestEntry {
                transaction: Some(nextblock_grpc::TransactionMessage {
                    content,
                    is_cleanup: false,
                }),
                skip_pre_flight: true,
            });
            signatures.push(signature);
        }

        self.client.clone().post_submit_batch_v2(nextblock_grpc::PostSubmitBatchRequest {
            entries,
            submit_strategy: nextblock_grpc::SubmitStrategy::PSubmitAll as i32,
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