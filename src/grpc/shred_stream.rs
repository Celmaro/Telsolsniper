use std::sync::Arc;

use futures::{channel::mpsc, StreamExt};
use solana_entry::entry::Entry;
use tonic::transport::Channel;

use log::error;
use solana_sdk::transaction::VersionedTransaction;

use crate::common::pumpswap::PumpSwapInstruction;
use crate::common::raydium::{RaydiumEvent, RaydiumInstruction};
use crate::common::AnyResult;

use solana_sdk::pubkey::Pubkey;
use crate::common::pumpfun::logs_data::DexInstruction;
use crate::common::pumpfun::logs_events::PumpfunEvent;
use crate::common::pumpswap::logs_events::PumpSwapEvent;
use crate::common::pumpfun::logs_filters::LogFilter;
use crate::common::pumpswap::logs_filters::LogFilter as PumpswapLogFilter;
use crate::common::raydium::logs_filters::LogFilter as RaydiumLogFilter;
use crate::swqos::jito_grpc::shredstream::shredstream_proxy_client::ShredstreamProxyClient;
use crate::swqos::jito_grpc::shredstream::SubscribeEntriesRequest;

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
            shredstream_client: Arc::new(shredstream_client)
        })
    }

    pub async fn shredstream_subscribe<F>(&self, callback: F, bot_wallet: Option<Pubkey>) -> AnyResult<()> 
    where
        F: Fn(PumpfunEvent) + Send + Sync + 'static,
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
            if let Err(e) = Self::process_pumpfun_transaction(transaction_with_slot, &*callback, bot_wallet).await {
                error!("Error processing transaction: {:?}", e);
            }
        }
    
        Ok(())
    }

    pub async fn shredstream_subscribe_pumpswap<F>(&self, callback: F) -> AnyResult<()> 
    where
        F: Fn(PumpSwapEvent) + Send + Sync + 'static,
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
            if let Err(e) = Self::process_pumpswap_transaction(transaction_with_slot, &*callback).await {
                error!("Error processing transaction: {:?}", e);
            }
        }
    
        Ok(())
    }

    pub async fn shredstream_subscribe_raydium<F>(&self, callback: F) -> AnyResult<()> 
    where
        F: Fn(RaydiumEvent) + Send + Sync + 'static,
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
            if let Err(e) = Self::process_raydium_transaction(transaction_with_slot, &*callback).await {
                error!("Error processing transaction: {:?}", e);
            }
        }
    
        Ok(())
    }

    async fn process_pumpfun_transaction<F>(transaction_with_slot: TransactionWithSlot, callback: &F, bot_wallet: Option<Pubkey>) -> AnyResult<()> 
    where
        F: Fn(PumpfunEvent) + Send + Sync,
    {
        let slot = transaction_with_slot.slot;
        let versioned_tx = transaction_with_slot.transaction;
        let mut dev_address: Option<Pubkey> = None;
        // let hash = versioned_tx.signatures[0].to_string();
        let instructions = LogFilter::parse_compiled_instruction(versioned_tx, bot_wallet).unwrap();
        for instruction in instructions {
            // println!("hash: {}\ninstruction: {:?}\n\n", hash, instruction);
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

    async fn process_pumpswap_transaction<F>(transaction_with_slot: TransactionWithSlot, callback: &F) -> AnyResult<()> 
    where
        F: Fn(PumpSwapEvent) + Send + Sync,
    {
        let slot = transaction_with_slot.slot;
        let versioned_tx = transaction_with_slot.transaction;
        let signature = versioned_tx.signatures[0].to_string();
        let instructions = PumpswapLogFilter::parse_pumpswap_compiled_instruction(versioned_tx).unwrap();
        for instruction in instructions {
            match instruction {
                PumpSwapInstruction::CreatePool(mut create_event) => {
                    create_event.slot = slot;
                    create_event.signature = signature.clone();
                    callback(PumpSwapEvent::CreatePool(create_event));
                }
                PumpSwapInstruction::Deposit(mut deposit_event) => {
                    deposit_event.slot = slot;
                    deposit_event.signature = signature.clone();
                    callback(PumpSwapEvent::Deposit(deposit_event));
                }
                PumpSwapInstruction::Withdraw(mut withdraw_event) => {
                    withdraw_event.slot = slot;
                    withdraw_event.signature = signature.clone();
                    callback(PumpSwapEvent::Withdraw(withdraw_event));
                }
                PumpSwapInstruction::Buy(mut buy_event) => {
                    buy_event.slot = slot;
                    buy_event.signature = signature.clone();
                    callback(PumpSwapEvent::Buy(buy_event));
                }
                PumpSwapInstruction::Sell(mut sell_event) => {
                    sell_event.slot = slot;
                    sell_event.signature = signature.clone();
                    callback(PumpSwapEvent::Sell(sell_event));
                }
                PumpSwapInstruction::UpdateFeeConfig(mut update_fee_event) => {
                    update_fee_event.slot = slot;
                    update_fee_event.signature = signature.clone();
                    callback(PumpSwapEvent::UpdateFeeConfig(update_fee_event));
                }
                PumpSwapInstruction::UpdateAdmin(mut update_admin_event) => {
                    update_admin_event.slot = slot;
                    update_admin_event.signature = signature.clone();
                    callback(PumpSwapEvent::UpdateAdmin(update_admin_event));
                }
                PumpSwapInstruction::Disable(mut disable_event) => {
                    disable_event.slot = slot;
                    disable_event.signature = signature.clone();
                    callback(PumpSwapEvent::Disable(disable_event));
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn process_raydium_transaction<F>(transaction_with_slot: TransactionWithSlot, callback: &F) -> AnyResult<()> 
    where
        F: Fn(RaydiumEvent) + Send + Sync,
    {
        let slot = transaction_with_slot.slot;
        let versioned_tx = transaction_with_slot.transaction;
        let signature = versioned_tx.signatures[0].to_string();
        let instructions: Vec<RaydiumInstruction> = RaydiumLogFilter::parse_raydium_compiled_instruction(versioned_tx).unwrap();
        for instruction in instructions {
            match instruction {
                RaydiumInstruction::V4Swap(mut v4_swap_event) => {
                    v4_swap_event.slot = slot;
                    v4_swap_event.signature = signature.clone();
                    callback(RaydiumEvent::V4Swap(v4_swap_event));
                }
                RaydiumInstruction::SwapBaseInput(mut swap_base_input_event) => {
                    swap_base_input_event.slot = slot;
                    swap_base_input_event.signature = signature.clone();
                    callback(RaydiumEvent::SwapBaseInput(swap_base_input_event));
                }
                RaydiumInstruction::SwapBaseOutput(mut swap_base_output_event) => {
                    swap_base_output_event.slot = slot;
                    swap_base_output_event.signature = signature.clone();
                    callback(RaydiumEvent::SwapBaseOutput(swap_base_output_event));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
