use std::{str::FromStr, sync::Arc};

use sol_trade_sdk::{
    common::{
        pumpfun::{
            self,
            logs_events::PumpfunEvent,
            logs_subscribe::{stop_subscription, tokens_subscription},
        },
        pumpswap::{self, PumpSwapEvent},
        AnyResult, Cluster, PriorityFee,
    },
    grpc::{ShredStreamGrpc, YellowstoneGrpc},
    PumpFun,
};
use solana_hash::Hash;
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair,
    transaction::VersionedTransaction,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // test_pumpfun_with_shreds().await?;
    // test_pumpfun_with_grpc().await?;
    // test_pumpswap_with_shreds().await?;
    // test_pumpswap_with_grpc().await?;
    test_sell().await?;
    Ok(())
}

async fn test_pumpfun_with_shreds() -> Result<(), Box<dyn std::error::Error>> {
    let grpc = ShredStreamGrpc::new("http://127.0.0.1:10000".to_string()).await?;

    let callback = |event: PumpfunEvent| {
        // TradeInfo 的 sol_amount 不是真实线上消费/获取的数量
        // 当 is_buy 为 true 时，sol_amount = max_sol_cost，代表用户愿意支付的最大金额
        // 当 is_buy 为 false 时，sol_amount = min_sol_output，代表用户愿意接受的最小金额
        //
        // timestamp 不是真实交易发生的时间，取的值为当前系统的时间
        //
        // 无法获取下面4个值
        // virtual_sol_reserves: 0,
        // virtual_token_reserves: 0,
        // real_sol_reserves: 0,
        // real_token_reserves: 0,
        match event {
            PumpfunEvent::NewDevTrade(trade_info) => {
                println!("Received new dev trade event: {:?}", trade_info);
            }
            PumpfunEvent::NewToken(token_info) => {
                println!("Received new token event: {:?}", token_info);
            }
            PumpfunEvent::NewUserTrade(trade_info) => {
                println!("Received new trade event: {:?}", trade_info);
            }
            PumpfunEvent::NewBotTrade(trade_info) => {
                println!("Received new bot trade event: {:?}", trade_info);
            }
            PumpfunEvent::Error(err) => {
                println!("Received error: {}", err);
            }
        }
    };

    grpc.shredstream_subscribe(callback, None).await?;

    Ok(())
}

async fn test_pumpfun_with_grpc() -> Result<(), Box<dyn std::error::Error>> {
    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    let callback = |event: PumpfunEvent| match event {
        PumpfunEvent::NewDevTrade(trade_info) => {
            println!("Received new dev trade event: {:?}", trade_info);
        }
        PumpfunEvent::NewToken(token_info) => {
            println!("Received new token event: {:?}", token_info);
        }
        PumpfunEvent::NewUserTrade(trade_info) => {
            println!("Received new trade event: {:?}", trade_info);
        }
        PumpfunEvent::NewBotTrade(trade_info) => {
            println!("Received new bot trade event: {:?}", trade_info);
        }
        PumpfunEvent::Error(err) => {
            println!("Received error: {}", err);
        }
    };

    grpc.subscribe_pumpfun(callback, None).await?;

    Ok(())
}

async fn test_pumpswap_with_shreds() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 ShredStream 客户端订阅 PumpSwap 事件
    println!("正在订阅 PumpSwap ShredStream 事件...");

    let grpc_client = ShredStreamGrpc::new("http://127.0.0.1:10800".to_string()).await?;

    // 定义回调函数处理 PumpSwap 事件
    let callback = |event: PumpSwapEvent| {
        match event {
            PumpSwapEvent::Buy(buy_event) => {
                // println!("buy_event: {:?}", buy_event);
            }
            PumpSwapEvent::Sell(sell_event) => {
                println!("sell_event: {:?}", sell_event);
            }
            PumpSwapEvent::CreatePool(create_event) => {
                // println!("create_event: {:?}", create_event);
            }
            PumpSwapEvent::Deposit(deposit_event) => {
                // println!("deposit_event: {:?}", deposit_event);
            }
            PumpSwapEvent::Withdraw(withdraw_event) => {
                // println!("withdraw_event: {:?}", withdraw_event);
            }
            PumpSwapEvent::Disable(disable_event) => {
                // println!("disable_event: {:?}", disable_event);
            }
            PumpSwapEvent::UpdateAdmin(update_admin_event) => {
                // println!("update_admin_event: {:?}", update_admin_event);
            }
            PumpSwapEvent::UpdateFeeConfig(update_fee_event) => {
                // println!("update_fee_event: {:?}", update_fee_event);
            }
            PumpSwapEvent::Error(err) => {
                println!("error: {}", err);
            }
        }
    };
    // 订阅 PumpSwap 事件
    println!("开始监听 PumpSwap 事件，按 Ctrl+C 停止...");

    grpc_client.shredstream_subscribe_pumpswap(callback).await?;

    Ok(())
}

async fn test_pumpswap_with_grpc() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 GRPC 客户端订阅 PumpSwap 事件
    println!("正在订阅 PumpSwap GRPC 事件...");

    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    // 定义回调函数处理 PumpSwap 事件
    let callback = |event: PumpSwapEvent| match event {
        PumpSwapEvent::Buy(buy_event) => {
            println!("buy_event: {:?}", buy_event);
        }
        PumpSwapEvent::Sell(sell_event) => {
            println!("sell_event: {:?}", sell_event);
        }
        PumpSwapEvent::CreatePool(create_event) => {
            println!("create_event: {:?}", create_event);
        }
        PumpSwapEvent::Deposit(deposit_event) => {
            println!("deposit_event: {:?}", deposit_event);
        }
        PumpSwapEvent::Withdraw(withdraw_event) => {
            println!("withdraw_event: {:?}", withdraw_event);
        }
        PumpSwapEvent::Disable(disable_event) => {
            println!("disable_event: {:?}", disable_event);
        }
        PumpSwapEvent::UpdateAdmin(update_admin_event) => {
            println!("update_admin_event: {:?}", update_admin_event);
        }
        PumpSwapEvent::UpdateFeeConfig(update_fee_event) => {
            println!("update_fee_event: {:?}", update_fee_event);
        }
        PumpSwapEvent::Error(err) => {
            println!("error: {}", err);
        }
    };
    // 订阅 PumpSwap 事件
    println!("开始监听 PumpSwap 事件，按 Ctrl+C 停止...");

    grpc.subscribe_pumpswap(callback).await?;

    Ok(())
}

async fn test_sell() -> AnyResult<()> {
    let payer = Keypair::new();
    // Define cluster configuration
    let cluster = Cluster {
        rpc_url: "https://mainnet.helius-rpc.com/?api-key=f2f194bb-6bd6-4f20-9a94-7fe0799ade0b"
            .to_string(),
        commitment: CommitmentConfig::confirmed(),
        priority_fee: PriorityFee::default(),
        use_jito: false,
        use_zeroslot: false,
        use_nozomi: false,
        use_nextblock: false,
        block_engine_url: "".to_string(),
        zeroslot_url: "".to_string(),
        zeroslot_auth_token: "".to_string(),
        nozomi_url: "".to_string(),
        nozomi_auth_token: "".to_string(),
        nextblock_url: "".to_string(),
        nextblock_auth_token: "".to_string(),
        lookup_table_key: None,
        use_rpc: true,
    };

    let pumpswap = PumpFun::new(Arc::new(payer), &cluster).await;
    let creator = Pubkey::from_str("8BtoThi2ZoXnF7QQK1Wjmh2JuBw9FjVvhnGMVZ2vpump")?;
    let dev_buy_token = 0;
    let dev_sol_cost = 0;
    let buy_sol_cost = 100_000_000;
    let slippage_basis_points = Some(100);
    let recent_blockhash = Hash::default();
    let trade_platform = "pumpswap".to_string();
    let mint_pubkey = Pubkey::from_str("8BtoThi2ZoXnF7QQK1Wjmh2JuBw9FjVvhnGMVZ2vpump")?;
    println!("Buying tokens from PumpSwap...");
    pumpswap
        .copy_buy(
            mint_pubkey,
            creator,
            dev_buy_token,
            dev_sol_cost,
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            trade_platform,
        )
        .await?;
    // 需要先转sol到wsol才能buy
    // pumpswap
    //     .buy(
    //         mint_pubkey,
    //         10_000_000, // 0.01 SOL
    //         Some(100),  // 1% slippage
    //     )
    //     .await?;
    // println!("Selling tokens to PumpSwap...");
    // pumpswap
    //     .sell_by_percent(
    //         mint_pubkey,
    //         100,       // Sell 100% of tokens
    //         Some(500), // 5% slippage
    //     )
    //     .await?;

    Ok(())
}

async fn test_wss() -> AnyResult<()> {
    println!("Starting token subscription\n");

    let ws_url = "wss://api.mainnet-beta.solana.com";

    // Set commitment
    let commitment = CommitmentConfig::confirmed();

    // Define callback function
    let callback = |event: PumpfunEvent| match event {
        PumpfunEvent::NewDevTrade(trade_info) => {
            println!("Received new dev trade event: {:?}", trade_info);
        }
        PumpfunEvent::NewToken(token_info) => {
            println!("Received new token event: {:?}", token_info);
        }
        PumpfunEvent::NewUserTrade(trade_info) => {
            println!("Received new trade event: {:?}", trade_info);
        }
        PumpfunEvent::NewBotTrade(trade_info) => {
            println!("Received new bot trade event: {:?}", trade_info);
        }
        PumpfunEvent::Error(err) => {
            println!("Received error: {}", err);
        }
    };

    // Start subscription
    let subscription = tokens_subscription(ws_url, commitment, callback, None)
        .await
        .unwrap();

    // Wait for a while to receive events
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

    // Stop subscription
    stop_subscription(subscription).await;

    Ok(())
}
