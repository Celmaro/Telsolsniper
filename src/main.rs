use std::{str::FromStr, sync::Arc};

use sol_trade_sdk::{
    common::{bonding_curve::BondingCurveAccount, AnyResult, PriorityFee, TradeConfig},
    match_event,
    streaming::{
        event_parser::{
            protocols::{
                bonk::{BonkPoolCreateEvent, BonkTradeEvent},
                pumpfun::{PumpFunCreateTokenEvent, PumpFunTradeEvent},
                pumpswap::{
                    PumpSwapBuyEvent, PumpSwapCreatePoolEvent, PumpSwapDepositEvent,
                    PumpSwapSellEvent, PumpSwapWithdrawEvent,
                },
            },
            Protocol, UnifiedEvent,
        },
        ShredStreamGrpc, YellowstoneGrpc,
    },
    swqos::{SwqosConfig, SwqosRegion},
    trading::{
        core::params::PumpFunParams, factory::DexType,
    },
    SolanaTrade,
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    test_create_solana_trade_client().await?;
    test_pumpswap().await?;
    test_bonk().await?;
    test_grpc().await?;
    test_shreds().await?;
    Ok(())
}

/// 创建 SolanaTrade 客户端的示例
async fn test_create_solana_trade_client() -> AnyResult<SolanaTrade> {
    println!("Creating SolanaTrade client...");

    let payer = Keypair::new();
    let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();

    // 配置各种 SWQOS 服务
    let swqos_configs = vec![
        SwqosConfig::Jito(SwqosRegion::Frankfurt),
        SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Default(rpc_url.clone()),
    ];

    // 定义交易配置
    let trade_config = TradeConfig {
        rpc_url: rpc_url.clone(),
        commitment: CommitmentConfig::confirmed(),
        priority_fee: PriorityFee::default(),
        swqos_configs,
        lookup_table_key: None,
    };

    let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    println!("SolanaTrade client created successfully!");

    Ok(solana_trade_client)
}

async fn test_pumpfun_copy_trade_width_grpc(trade_info: PumpFunTradeEvent) -> AnyResult<()> {

    println!("Testing PumpFun trading...");

    let solana_trade_client = test_create_solana_trade_client().await?;
    let creator = Pubkey::from_str("xxxxxx")?; // dev account
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    let mint_pubkey = Pubkey::from_str("xxxxxx")?; // token mint

    println!("Buying tokens from PumpFun...");
    
    let bonding_curve = BondingCurveAccount::from_trade(&trade_info);

    solana_trade_client
        .buy(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            Some(Box::new(PumpFunParams {
                bonding_curve: Some(Arc::new(bonding_curve.clone())),
            })),
        )
        .await?;
    // sell
    println!("Selling tokens from PumpFun...");
    let amount_token = 0; // 写上真实的amount_token
    solana_trade_client
        .sell(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    Ok(())
}

async fn test_pumpfun_sniper_trade_width_shreds(trade_info: PumpFunTradeEvent) -> AnyResult<()> {

    println!("Testing PumpFun trading...");

    // if not dev trade, return
    if !trade_info.is_dev_create_token_trade {
        return Ok(());
    }

    let solana_trade_client = test_create_solana_trade_client().await?;
    let mint_pubkey = trade_info.mint;
    let creator = trade_info.creator;
    let buy_sol_cost = trade_info.max_sol_cost;
    let amount_token = trade_info.token_amount;
    let slippage_basis_points = Some(100);
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    
    println!("Buying tokens from PumpFun...");
    
    let bonding_curve = BondingCurveAccount::from_dev_trade(
        &mint_pubkey,
        amount_token,
        buy_sol_cost,
        creator,
    );
 
    solana_trade_client
        .buy(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            Some(Box::new(PumpFunParams {
                bonding_curve: Some(Arc::new(bonding_curve.clone())),
            })),
        )
        .await?;
    // sell
    println!("Selling tokens from PumpFun...");
    let amount_token = 0; // 写上真实的amount_token
    solana_trade_client
        .sell(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    Ok(())
}

async fn test_pumpswap() -> AnyResult<()> {
    println!("Testing PumpSwap trading...");

    let solana_trade_client = test_create_solana_trade_client().await?;
    let creator = Pubkey::from_str("11111111111111111111111111111111")?; // dev account
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    let mint_pubkey = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?; // token mint

    println!("Buying tokens from PumpSwap...");
    // buy
    solana_trade_client
        .buy(
            DexType::PumpFun,
            mint_pubkey,
            Some(creator),
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    // sell
    println!("Selling tokens from PumpSwap...");
    let amount_token = 0; // 写上真实的amount_token
    solana_trade_client
        .sell(
            DexType::PumpSwap,
            mint_pubkey,
            Some(creator),
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    Ok(())
}

async fn test_bonk() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Bonk trading...");

    let solana_trade_client = test_create_solana_trade_client().await?;
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100); // 1%
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    let mint_pubkey = Pubkey::from_str("xxxxxxx")?;

    println!("Buying tokens from letsbonk.fun...");
    // buy
    solana_trade_client
        .buy(
            DexType::Bonk,
            mint_pubkey,
            None,
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    // sell
    println!("Selling tokens from letsbonk.fun...");
    let amount_token = 0; // 写上真实的amount_token
    solana_trade_client
        .sell(
            DexType::Bonk,
            mint_pubkey,
            None,
            amount_token,
            slippage_basis_points,
            recent_blockhash,
            None,
            false,
            None,
        )
        .await?;
    Ok(())
}

async fn test_grpc() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 GRPC 客户端订阅事件
    println!("正在订阅 GRPC 事件...");

    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    // 定义回调函数处理 PumpSwap 事件
    let callback = |event: Box<dyn UnifiedEvent>| {
        match_event!(event, {
            BonkPoolCreateEvent => |e: BonkPoolCreateEvent| {
                println!("BonkPoolCreateEvent: {:?}", e.base_mint_param.symbol);
            },
            BonkTradeEvent => |e: BonkTradeEvent| {
                println!("BonkTradeEvent: {:?}", e);
            },
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                println!("PumpFunTradeEvent: {:?}", e);
            },
            PumpFunCreateTokenEvent => |e: PumpFunCreateTokenEvent| {
                println!("PumpFunCreateTokenEvent: {:?}", e);
            },
            PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {
                println!("Buy event: {:?}", e);
            },
            PumpSwapSellEvent => |e: PumpSwapSellEvent| {
                println!("Sell event: {:?}", e);
            },
            PumpSwapCreatePoolEvent => |e: PumpSwapCreatePoolEvent| {
                println!("CreatePool event: {:?}", e);
            },
            PumpSwapDepositEvent => |e: PumpSwapDepositEvent| {
                println!("Deposit event: {:?}", e);
            },
            PumpSwapWithdrawEvent => |e: PumpSwapWithdrawEvent| {
                println!("Withdraw event: {:?}", e);
            },
        });
    };

    // 订阅 PumpSwap 事件
    println!("开始监听事件，按 Ctrl+C 停止...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk];
    grpc.subscribe_events(protocols, None, None, None, callback)
        .await?;

    Ok(())
}

async fn test_shreds() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 ShredStream 客户端订阅事件
    println!("正在订阅 ShredStream 事件...");

    let shred_stream = ShredStreamGrpc::new("http://127.0.0.1:10800".to_string()).await?;

    // 定义回调函数处理 PumpSwap 事件
    let callback = |event: Box<dyn UnifiedEvent>| {
        match_event!(event, {
            BonkPoolCreateEvent => |e: BonkPoolCreateEvent| {
                println!("BonkPoolCreateEvent: {:?}", e.base_mint_param.symbol);
            },
            BonkTradeEvent => |e: BonkTradeEvent| {
                println!("BonkTradeEvent: {:?}", e);
            },
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                println!("PumpFunTradeEvent: {:?}", e);
            },
            PumpFunCreateTokenEvent => |e: PumpFunCreateTokenEvent| {
                println!("PumpFunCreateTokenEvent: {:?}", e);
            },
            PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {
                println!("Buy event: {:?}", e);
            },
            PumpSwapSellEvent => |e: PumpSwapSellEvent| {
                println!("Sell event: {:?}", e);
            },
            PumpSwapCreatePoolEvent => |e: PumpSwapCreatePoolEvent| {
                println!("CreatePool event: {:?}", e);
            },
            PumpSwapDepositEvent => |e: PumpSwapDepositEvent| {
                println!("Deposit event: {:?}", e);
            },
            PumpSwapWithdrawEvent => |e: PumpSwapWithdrawEvent| {
                println!("Withdraw event: {:?}", e);
            },
        });
    };

    // 订阅 PumpSwap 事件
    println!("开始监听事件，按 Ctrl+C 停止...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk];
    shred_stream
        .shredstream_subscribe(protocols, None, callback)
        .await?;

    Ok(())
}
