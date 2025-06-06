use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter}
};

use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use futures::StreamExt;
use crate::common::pumpswap::{
    logs_events::PumpSwapEvent,
    logs_filters::LogFilter
};
use crate::constants::pumpswap::accounts;

/// 订阅句柄，包含任务和取消订阅逻辑
pub struct SubscriptionHandle {
    pub task: JoinHandle<()>,
    pub unsub_fn: Box<dyn Fn() + Send>,
}

impl SubscriptionHandle {
    pub async fn shutdown(self) {
        (self.unsub_fn)();
        self.task.abort();
    }
}

/// 创建PubSub客户端
pub async fn create_pubsub_client(ws_url: &str) -> PubsubClient {
    PubsubClient::new(ws_url).await.unwrap()
}

/// 启动PumpSwap代币订阅
pub async fn tokens_subscription<F>(
    ws_url: &str,
    commitment: CommitmentConfig,
    callback: F,
) -> Result<SubscriptionHandle, Box<dyn std::error::Error>>
where
    F: Fn(PumpSwapEvent) + Send + Sync + 'static,
{
    // 使用constants中定义的AMM_PROGRAM
    let program_address = accounts::AMM_PROGRAM.to_string();
    let logs_filter = RpcTransactionLogsFilter::Mentions(vec![program_address]);

    let logs_config = RpcTransactionLogsConfig {
        commitment: Some(commitment),
    };

    // 创建PubsubClient
    let sub_client = Arc::new(PubsubClient::new(ws_url).await.unwrap());

    let sub_client_clone = Arc::clone(&sub_client);

    // 创建用于取消订阅的通道
    let (unsub_tx, _) = mpsc::channel(1);

    // 启动订阅任务
    let task = tokio::spawn(async move {
        let (mut stream, _) = sub_client_clone.logs_subscribe(logs_filter, logs_config).await.unwrap();

        loop {
            let msg = stream.next().await;
            match msg {
                Some(msg) => {
                    if let Some(_err) = msg.value.err {
                        continue;
                    }

                    let events = LogFilter::parse_pumpswap_logs(&msg.value.logs);
                    for mut event in events {
                        // 设置签名和slot
                        match &mut event {
                            PumpSwapEvent::Buy(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            PumpSwapEvent::Sell(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            PumpSwapEvent::CreatePool(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            PumpSwapEvent::Deposit(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            PumpSwapEvent::Withdraw(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            PumpSwapEvent::Disable(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            PumpSwapEvent::UpdateAdmin(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            PumpSwapEvent::UpdateFeeConfig(e) => {
                                e.signature = msg.value.signature.clone();
                                e.slot = msg.context.slot;
                            },
                            _ => {}
                        }
                        callback(event);
                    }
                }
                None => {
                    println!("PumpSwap subscription stream ended");
                }
            }
        }
    });

    // 返回订阅句柄和取消订阅逻辑
    Ok(SubscriptionHandle {
        task,
        unsub_fn: Box::new(move || {
            let _ = unsub_tx.try_send(());
        }),
    })
}



/// 停止订阅
pub async fn stop_subscription(handle: SubscriptionHandle) {
    handle.shutdown().await;
}
