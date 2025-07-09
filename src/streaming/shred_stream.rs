use std::sync::Arc;

use futures::{channel::mpsc, StreamExt};
use solana_entry::entry::Entry;
use tonic::transport::Channel;

use log::error;
use solana_sdk::transaction::VersionedTransaction;

use crate::common::AnyResult;
use crate::streaming::event_parser::{EventParserFactory, Protocol, UnifiedEvent};

use crate::protos::shredstream::shredstream_proxy_client::ShredstreamProxyClient;
use crate::protos::shredstream::SubscribeEntriesRequest;
use solana_sdk::pubkey::Pubkey;

const CHANNEL_SIZE: usize = 1000;

pub struct ShredStreamGrpc {
    shredstream_client: Arc<ShredstreamProxyClient<Channel>>,
}

struct TransactionWithSlot {
    transaction: VersionedTransaction,
    slot: u64,
}

impl ShredStreamGrpc {
    pub async fn new(endpoint: String) -> AnyResult<Self> {
        let shredstream_client = ShredstreamProxyClient::connect(endpoint.clone()).await?;
        Ok(Self {
            shredstream_client: Arc::new(shredstream_client),
        })
    }

    pub async fn shredstream_subscribe<F>(
        &self,
        protocols: Vec<Protocol>,
        bot_wallet: Option<Pubkey>,
        callback: F,
    ) -> AnyResult<()>
    where
        F: Fn(Box<dyn UnifiedEvent>) + Send + Sync + 'static,
    {
        let request = tonic::Request::new(SubscribeEntriesRequest {});
        let mut client = (*self.shredstream_client).clone();
        let mut stream = client.subscribe_entries(request).await?.into_inner();
        let (mut tx, mut rx) = mpsc::channel::<TransactionWithSlot>(CHANNEL_SIZE);
        let callback = Box::new(callback);
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Ok(entries) = bincode::deserialize::<Vec<Entry>>(&msg.entries) {
                            for entry in entries {
                                for transaction in entry.transactions {
                                    let _ = tx.try_send(TransactionWithSlot {
                                        transaction: transaction.clone(),
                                        slot: msg.slot,
                                    });
                                }
                            }
                        }
                    }
                    Err(error) => {
                        error!("Stream error: {error:?}");
                        break;
                    }
                }
            }
        });

        while let Some(transaction_with_slot) = rx.next().await {
            if let Err(e) = Self::process_transaction(
                transaction_with_slot,
                protocols.clone(),
                bot_wallet,
                &*callback,
            )
            .await
            {
                error!("Error processing transaction: {:?}", e);
            }
        }

        Ok(())
    }

    async fn process_transaction<F>(
        transaction_with_slot: TransactionWithSlot,
        protocols: Vec<Protocol>,
        bot_wallet: Option<Pubkey>,
        callback: &F,
    ) -> AnyResult<()>
    where
        F: Fn(Box<dyn UnifiedEvent>) + Send + Sync,
    {
        let slot = transaction_with_slot.slot;
        let versioned_tx = transaction_with_slot.transaction;
        let signature = versioned_tx.signatures[0];

        for protocol in protocols {
            let parser = EventParserFactory::create_parser(protocol.clone());
            let events = parser
                .parse_versioned_transaction(
                    &versioned_tx,
                    &signature.to_string(),
                    Some(slot),
                    bot_wallet.clone(),
                )
                .await
                .unwrap_or_else(|_e| vec![]);
            for event in events {
                callback(event);
            }
        }

        Ok(())
    }
}
