use std::{collections::HashMap, fmt, time::Duration};

use futures::{channel::mpsc, sink::Sink, Stream, StreamExt, SinkExt};
use rustls::crypto::{ring::default_provider, CryptoProvider};
use tonic::{transport::channel::ClientTlsConfig, Status};
use yellowstone_grpc_client::{GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions, SubscribeUpdate,
    SubscribeUpdateTransaction, subscribe_update::UpdateOneof, SubscribeRequestPing,
};
use log::{error, info};
use chrono::Local;
use solana_sdk::{pubkey, pubkey::Pubkey, signature::Signature};
use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedTransactionWithStatusMeta, UiTransactionEncoding,
};

use crate::common::pumpfun::logs_data::{DexInstruction, TransferInfo};
use crate::common::pumpfun::logs_events::{PumpfunEvent, SystemEvent};
use crate::common::pumpfun::logs_filters::LogFilter;
use crate::common::AnyResult;

type TransactionsFilterMap = HashMap<String, SubscribeRequestFilterTransactions>;

const PUMP_PROGRAM_ID: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
const CONNECT_TIMEOUT: u64 = 10;
const REQUEST_TIMEOUT: u64 = 60;
const CHANNEL_SIZE: usize = 1000;
const MAX_DECODING_MESSAGE_SIZE: usize = 1024 * 1024 * 10;

#[derive(Clone)]
pub struct TransactionPretty {
    pub slot: u64,
    pub signature: Signature,
    pub is_vote: bool,
    pub tx: EncodedTransactionWithStatusMeta,
}

impl fmt::Debug for TransactionPretty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct TxWrap<'a>(&'a EncodedTransactionWithStatusMeta);
        impl<'a> fmt::Debug for TxWrap<'a> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let serialized = serde_json::to_string(self.0).expect("failed to serialize");
                fmt::Display::fmt(&serialized, f)
            }
        }

        f.debug_struct("TransactionPretty")
            .field("slot", &self.slot)
            .field("signature", &self.signature)
            .field("is_vote", &self.is_vote)
            .field("tx", &TxWrap(&self.tx))
            .finish()
    }
}

impl From<SubscribeUpdateTransaction> for TransactionPretty {
    fn from(SubscribeUpdateTransaction { transaction, slot }: SubscribeUpdateTransaction) -> Self {
        let tx = transaction.expect("should be defined");
        Self {
            slot,
            signature: Signature::try_from(tx.signature.as_slice()).expect("valid signature"),
            is_vote: tx.is_vote,
            tx: yellowstone_grpc_proto::convert_from::create_tx_with_meta(tx)
                .expect("valid tx with meta")
                .encode(UiTransactionEncoding::Base64, Some(u8::MAX), true)
                .expect("failed to encode"),
        }
    }
}

pub struct YellowstoneGrpc {
    endpoint: String,
    x_token: Option<String>,
}

impl YellowstoneGrpc {
    pub fn new(endpoint: String, x_token: Option<String>) -> AnyResult<Self> {
        if CryptoProvider::get_default().is_none() {
            default_provider()
                .install_default()
                .map_err(|e| anyhow::anyhow!("Failed to install crypto provider: {:?}", e))?;
        }

        Ok(Self { 
            endpoint, 
            x_token,
        })
    }

    pub async fn connect(
        &self,
    ) -> AnyResult<GeyserGrpcClient<impl Interceptor>>
    {
        let builder = GeyserGrpcClient::build_from_shared(self.endpoint.clone())?
            .x_token(self.x_token.clone())?
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT));

        Ok(builder.connect().await?)
    }

    pub async fn subscribe_with_request(
        &self,
        transactions: TransactionsFilterMap,
    ) -> AnyResult<(
        impl Sink<SubscribeRequest, Error = mpsc::SendError>,
        impl Stream<Item = Result<SubscribeUpdate, Status>>,
    )> {
        let subscribe_request = SubscribeRequest {
            transactions,
            commitment: Some(CommitmentLevel::Processed.into()),
            ..Default::default()
        };

        let mut client = self.connect().await?;
        let (sink, stream) = client.subscribe_with_request(Some(subscribe_request)).await?;
        Ok((sink, stream))
    }

    pub fn get_subscribe_request_filter(
        &self,
        account_include: Vec<String>,
        account_exclude: Vec<String>,
        account_required: Vec<String>,
    ) -> TransactionsFilterMap {
        let mut transactions = HashMap::new();
        transactions.insert(
            "client".to_string(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                signature: None,
                account_include,
                account_exclude,
                account_required,
            },
        );
        transactions
    }

    async fn handle_stream_message(
        msg: SubscribeUpdate,
        tx: &mut mpsc::Sender<TransactionPretty>,
        subscribe_tx: &mut (impl Sink<SubscribeRequest, Error = mpsc::SendError> + Unpin),
    ) -> AnyResult<()> {
        match msg.update_oneof {
            Some(UpdateOneof::Transaction(sut)) => {
                let transaction_pretty = TransactionPretty::from(sut);
                tx.try_send(transaction_pretty)?;
            }
            Some(UpdateOneof::Ping(_)) => {
                subscribe_tx
                    .send(SubscribeRequest {
                        ping: Some(SubscribeRequestPing { id: 1 }),
                        ..Default::default()
                    })
                    .await?;
                info!("service is ping: {}", Local::now());
            }
            Some(UpdateOneof::Pong(_)) => {
                info!("service is pong: {}", Local::now());
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn subscribe_pumpfun<F>(&self, callback: F, bot_wallet: Option<Pubkey>) -> AnyResult<()> 
    where
        F: Fn(PumpfunEvent) + Send + Sync + 'static,
    {
        let addrs = vec![PUMP_PROGRAM_ID.to_string()];
        let transactions = self.get_subscribe_request_filter(addrs, vec![], vec![]);
        let (mut subscribe_tx, mut stream) = self.subscribe_with_request(transactions).await?;
        let (mut tx, mut rx) = mpsc::channel::<TransactionPretty>(CHANNEL_SIZE);

        let callback = Box::new(callback);

        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) = Self::handle_stream_message(msg, &mut tx, &mut subscribe_tx).await {
                            error!("Error handling message: {:?}", e);
                            break;
                        }
                    }
                    Err(error) => {
                        error!("Stream error: {error:?}");
                        break;
                    }
                }
            }
        });

        while let Some(transaction_pretty) = rx.next().await {
            if let Err(e) = Self::process_pumpfun_transaction(transaction_pretty, &*callback, bot_wallet).await {
                error!("Error processing transaction: {:?}", e);
            }
        }
        Ok(())
    }

    pub async fn subscribe_pumpfun_with_filter<F>(&self, callback: F, bot_wallet: Option<Pubkey>, account_include: Option<Vec<String>>, account_exclude: Option<Vec<String>>) -> AnyResult<()> 
    where
        F: Fn(PumpfunEvent) + Send + Sync + 'static,
    {
        let addrs = vec![PUMP_PROGRAM_ID.to_string()];
        let account_include = account_include.unwrap_or_default();
        let account_exclude = account_exclude.unwrap_or_default();
        let transactions = self.get_subscribe_request_filter(account_include, account_exclude, addrs);
        let (mut subscribe_tx, mut stream) = self.subscribe_with_request(transactions).await?;
        let (mut tx, mut rx) = mpsc::channel::<TransactionPretty>(CHANNEL_SIZE);

        let callback = Box::new(callback);

        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) = Self::handle_stream_message(msg, &mut tx, &mut subscribe_tx).await {
                            error!("Error handling message: {:?}", e);
                            break;
                        }
                    }
                    Err(error) => {
                        error!("Stream error: {error:?}");
                        break;
                    }
                }
            }
        });

        while let Some(transaction_pretty) = rx.next().await {
            if let Err(e) = Self::process_pumpfun_transaction(transaction_pretty, &*callback, bot_wallet).await {
                error!("Error processing transaction: {:?}", e);
            }
        }
        Ok(())
    }

    async fn process_pumpfun_transaction<F>(transaction_pretty: TransactionPretty, callback: &F, bot_wallet: Option<Pubkey>) -> AnyResult<()> 
    where
        F: Fn(PumpfunEvent) + Send + Sync,
    {
        let slot = transaction_pretty.slot;
        let trade_raw: EncodedTransactionWithStatusMeta = transaction_pretty.tx;
        let meta = trade_raw.meta.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing transaction metadata"))?;
            
        if meta.err.is_some() {
            return Ok(());
        }

        let logs = if let OptionSerializer::Some(logs) = &meta.log_messages {
            logs
        } else {
            &vec![]
        };

        let mut dev_address: Option<Pubkey> = None;
        let instructions = LogFilter::parse_instruction(logs, bot_wallet).unwrap();
        for instruction in instructions {
            match instruction {
                DexInstruction::CreateToken(mut token_info) => {
                    token_info.slot = slot;
                    dev_address = Some(token_info.user);
                    callback(PumpfunEvent::NewToken(token_info));
                }
                DexInstruction::UserTrade(mut trade_info) => {
                    trade_info.slot = slot;
                    if Some(trade_info.user) == dev_address {
                        callback(PumpfunEvent::NewDevTrade(trade_info));
                    } else {
                        callback(PumpfunEvent::NewUserTrade(trade_info));
                    }
                }
                DexInstruction::BotTrade(mut trade_info) => {
                    trade_info.slot = slot;
                    callback(PumpfunEvent::NewBotTrade(trade_info));
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn subscribe_system<F>(&self, callback: F, account_include: Option<Vec<String>>, account_exclude: Option<Vec<String>>) -> AnyResult<()> 
    where
        F: Fn(SystemEvent) + Send + Sync + 'static,
    {
        let addrs = vec![SYSTEM_PROGRAM_ID.to_string()];
        let account_include = account_include.unwrap_or_default();
        let account_exclude = account_exclude.unwrap_or_default();
        let transactions = self.get_subscribe_request_filter(account_include, account_exclude, addrs);
        let (mut subscribe_tx, mut stream) = self.subscribe_with_request(transactions).await?;
        let (mut tx, mut rx) = mpsc::channel::<TransactionPretty>(CHANNEL_SIZE);

        let callback = Box::new(callback);

        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) = Self::handle_stream_message(msg, &mut tx, &mut subscribe_tx).await {
                            error!("Error handling message: {:?}", e);
                            break;
                        }
                    }
                    Err(error) => {
                        error!("Stream error: {error:?}");
                        break;
                    }
                }
            }
        });

        while let Some(transaction_pretty) = rx.next().await {
            if let Err(e) = Self::process_system_transaction(transaction_pretty, &*callback).await {
                error!("Error processing transaction: {:?}", e);
            }
        }
        Ok(())
    }

    async fn process_system_transaction<F>(transaction_pretty: TransactionPretty, callback: &F) -> AnyResult<()> 
    where
        F: Fn(SystemEvent) + Send + Sync,
    {        
        let trade_raw: EncodedTransactionWithStatusMeta = transaction_pretty.tx;
        let meta = trade_raw.meta.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing transaction metadata"))?;
            
        if meta.err.is_some() {
            return Ok(());
        }
        
        callback(SystemEvent::NewTransfer(TransferInfo {
            slot: transaction_pretty.slot,
            signature: transaction_pretty.signature.to_string(),
            tx: trade_raw.transaction.decode(),
        }));

        Ok(())
    }
}
