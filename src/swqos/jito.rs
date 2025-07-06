
use tonic::transport::Channel;
use tokio::sync::Mutex;

use rand::{rng, seq::IteratorRandom};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use solana_sdk::{transaction::VersionedTransaction, signature::Signature};
use crate::protos::searcher::searcher_service_client::SearcherServiceClient;
use crate::protos::searcher_client::{self, get_searcher_client_no_auth, send_bundle_with_confirmation};
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;

use crate::{common::SolanaRpcClient, constants::pumpfun::accounts::JITO_TIP_ACCOUNTS};


pub struct JitoClient {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub searcher_client: Arc<Mutex<SearcherServiceClient<Channel>>>,
}

#[async_trait::async_trait]
impl SwqosClientTrait for JitoClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature> {
        self.send_bundle_with_confirmation(trade_type, &vec![transaction.clone()]).await?.first().cloned().ok_or(anyhow!("Failed to send transaction"))
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>> {
        self.send_bundle_with_confirmation(trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        if let Some(acc) = JITO_TIP_ACCOUNTS.iter().choose(&mut rng()) {
            Ok(acc.to_string())
        } else {
            Err(anyhow!("no valid tip accounts found"))
        }
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Jito
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
    ) -> Result<Vec<Signature>> {
        send_bundle_with_confirmation(self.rpc_client.clone(), trade_type, &transactions, self.searcher_client.clone()).await
    }

    pub async fn send_bundle_no_wait(
        &self,
        transactions: &Vec<VersionedTransaction>,
    ) -> Result<Vec<Signature>> {
        searcher_client::send_bundle_no_wait(&transactions, self.searcher_client.clone()).await
    }
}