use crate::{common::AnyResult, streaming::yellowstone_grpc::{TransactionPretty, YellowstoneGrpc}};
use solana_program::pubkey;
use solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction};
use futures::{channel::mpsc, StreamExt};
use log::error;
use solana_transaction_status::EncodedTransactionWithStatusMeta;

const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
const CHANNEL_SIZE: usize = 1000;

#[derive(Debug)]
pub enum SystemEvent {
    NewTransfer(TransferInfo),
    Error(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransferInfo {
    pub slot: u64,
    pub signature: String,
    pub tx: Option<VersionedTransaction>,
}

impl YellowstoneGrpc {
    pub async fn subscribe_system<F>(
        &self,
        callback: F,
        account_include: Option<Vec<String>>,
        account_exclude: Option<Vec<String>>,
    ) -> AnyResult<()>
    where
        F: Fn(SystemEvent) + Send + Sync + 'static,
    {
        let addrs = vec![SYSTEM_PROGRAM_ID.to_string()];
        let account_include = account_include.unwrap_or_default();
        let account_exclude = account_exclude.unwrap_or_default();
        let transactions =
            self.get_subscribe_request_filter(account_include, account_exclude, addrs);
        let (mut subscribe_tx, mut stream) = self.subscribe_with_request(transactions).await?;
        let (mut tx, mut rx) = mpsc::channel::<TransactionPretty>(CHANNEL_SIZE);

        let callback = Box::new(callback);

        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) =
                            Self::handle_stream_message(msg, &mut tx, &mut subscribe_tx).await
                        {
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

    async fn process_system_transaction<F>(
        transaction_pretty: TransactionPretty,
        callback: &F,
    ) -> AnyResult<()>
    where
        F: Fn(SystemEvent) + Send + Sync,
    {
        let trade_raw: EncodedTransactionWithStatusMeta = transaction_pretty.tx;
        let meta = trade_raw
            .meta
            .as_ref()
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
