use std::{str::FromStr, sync::Arc};

use sol_trade_sdk::{
    accounts::BondingCurveAccount,
    common::{AnyResult, PriorityFee, TradeConfig},
    constants::{pumpfun::global_constants::TOKEN_TOTAL_SUPPLY, trade_type},
    event_parser::{
        protocols::{
            pumpfun::{PumpFunCreateTokenEvent, PumpFunTradeEvent},
            pumpswap::{
                PumpSwapBuyEvent, PumpSwapCreatePoolEvent, PumpSwapDepositEvent, PumpSwapSellEvent,
                PumpSwapWithdrawEvent,
            },
            bonk::{BonkPoolCreateEvent, BonkTradeEvent},
        },
        Protocol, UnifiedEvent,
    },
    grpc::{ShredStreamGrpc, YellowstoneGrpc},
    match_event,
    pumpfun::common::get_bonding_curve_account_v2,
    swqos::{SwqosConfig, SwqosRegion},
    trading::{
        core::params::{PumpFunParams, PumpFunSellParams, PumpSwapParams, BonkParams},
        BuyParams, SellParams,
    },
    SolanaTrade,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    test_pumpfun().await?;
    // test_pumpswap().await?;
    // test_bonk().await?;
    // test_grpc().await?;
    // test_shreds().await?;
    Ok(())
}

async fn test_pumpfun() -> AnyResult<()> {
    let payer = Keypair::new();
    let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();
    let swqos_configs = vec![
        SwqosConfig::Jito(SwqosRegion::Frankfurt),
        SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Default(rpc_url.clone()),
    ];
    // Define cluster configuration
    let trade_config = TradeConfig {
        rpc_url: rpc_url.clone(),
        commitment: CommitmentConfig::confirmed(),
        priority_fee: PriorityFee::default(),
        swqos_configs,
        lookup_table_key: None,
    };
    let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    let creator = Pubkey::from_str("xxxxxx")?; // dev account
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();
    let mint_pubkey = Pubkey::from_str("xxxxxx")?; // token mint
    println!("Buying tokens from PumpFun...");
    // get bonding curve
    let (bonding_curve, bonding_curve_pda) =
        get_bonding_curve_account_v2(&solana_trade_client.rpc, &mint_pubkey).await?;
    let virtual_token_reserves = bonding_curve.virtual_token_reserves;
    let virtual_sol_reserves = bonding_curve.virtual_sol_reserves;
    let real_token_reserves = bonding_curve.real_token_reserves;
    let real_sol_reserves = bonding_curve.real_sol_reserves;
    let bonding_curve = BondingCurveAccount {
        discriminator: bonding_curve.discriminator,
        account: bonding_curve_pda,
        virtual_token_reserves: virtual_token_reserves,
        virtual_sol_reserves: virtual_sol_reserves,
        real_token_reserves: real_token_reserves,
        real_sol_reserves: real_sol_reserves,
        token_total_supply: TOKEN_TOTAL_SUPPLY,
        complete: false,
        creator: creator,
    };
    // 如果是狙击开发者
    // let bonding_curve =
    //     BondingCurveAccount::new(&mint_pubkey, dev_buy_token, dev_cost_sol, creator);
    // buy
    let buy_protocol_params = PumpFunParams {
        trade_type: trade_type::COPY_BUY.to_string(),
        bonding_curve: Some(Arc::new(bonding_curve)),
    };
    let buy_params = BuyParams {
        rpc: Some(solana_trade_client.rpc.clone()),
        payer: solana_trade_client.payer.clone(),
        mint: mint_pubkey,
        creator: creator,
        amount_sol: buy_sol_cost,
        slippage_basis_points: slippage_basis_points,
        priority_fee: solana_trade_client.trade_config.clone().priority_fee,
        lookup_table_key: solana_trade_client.trade_config.clone().lookup_table_key,
        recent_blockhash,
        data_size_limit: 0,
        protocol_params: Box::new(buy_protocol_params.clone()),
    };
    let buy_with_tip_params = buy_params
        .clone()
        .with_tip(solana_trade_client.swqos_clients.clone());
    solana_trade_client
        .buy_use_buy_params(buy_with_tip_params, None)
        .await?;
    // sell
    println!("Selling tokens from PumpFun...");
    let sell_protocol_params = PumpFunSellParams {};
    let amount_token = 0; // 写上真实的amount_token
    let sell_params = SellParams {
        rpc: Some(solana_trade_client.rpc.clone()),
        payer: solana_trade_client.payer.clone(),
        mint: mint_pubkey,
        creator: creator,
        amount_token: Some(amount_token),
        slippage_basis_points: None,
        priority_fee: solana_trade_client.trade_config.clone().priority_fee,
        lookup_table_key: solana_trade_client.trade_config.clone().lookup_table_key,
        recent_blockhash,
        protocol_params: Box::new(sell_protocol_params.clone()),
    };
    solana_trade_client
        .sell_by_amount_use_sell_params(sell_params)
        .await?;
    Ok(())
}

async fn test_pumpswap() -> AnyResult<()> {
    let payer = Keypair::new();
    let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();
    let swqos_configs = vec![
        SwqosConfig::Jito(SwqosRegion::Frankfurt),
        SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Default(rpc_url.clone()),
    ];
    // Define cluster configuration
    let trade_config = TradeConfig {
        rpc_url: rpc_url.clone(),
        commitment: CommitmentConfig::confirmed(),
        priority_fee: PriorityFee::default(),
        swqos_configs,
        lookup_table_key: None,
    };
    let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    let creator = Pubkey::from_str("11111111111111111111111111111111")?; // dev account
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();
    let mint_pubkey = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?; // token mint
    println!("Buying tokens from PumpSwap...");
    // buy
    let protocol_params = PumpSwapParams {
        pool: None,
        pool_base_token_account: None,
        pool_quote_token_account: None,
        user_base_token_account: None,
        user_quote_token_account: None,
        auto_handle_wsol: true,
    };
    let buy_params = BuyParams {
        rpc: Some(solana_trade_client.rpc.clone()),
        payer: solana_trade_client.payer.clone(),
        mint: mint_pubkey,
        creator: creator,
        amount_sol: buy_sol_cost,
        slippage_basis_points: slippage_basis_points,
        priority_fee: solana_trade_client.trade_config.clone().priority_fee,
        lookup_table_key: solana_trade_client.trade_config.clone().lookup_table_key,
        recent_blockhash,
        data_size_limit: 0,
        protocol_params: Box::new(protocol_params.clone()),
    };
    let buy_with_tip_params = buy_params
        .clone()
        .with_tip(solana_trade_client.swqos_clients.clone());
    solana_trade_client
        .buy_use_buy_params(buy_with_tip_params, None)
        .await?;
    // sell
    println!("Selling tokens from PumpSwap...");
    let amount_token = 0; // 写上真实的amount_token
    let sell_params = SellParams {
        rpc: Some(solana_trade_client.rpc.clone()),
        payer: solana_trade_client.payer.clone(),
        mint: mint_pubkey,
        creator: creator,
        amount_token: Some(amount_token),
        slippage_basis_points: None,
        priority_fee: solana_trade_client.trade_config.clone().priority_fee,
        lookup_table_key: solana_trade_client.trade_config.clone().lookup_table_key,
        recent_blockhash,
        protocol_params: Box::new(protocol_params.clone()),
    };
    solana_trade_client
        .sell_by_amount_use_sell_params(sell_params)
        .await?;
    Ok(())
}

async fn test_bonk() -> Result<(), Box<dyn std::error::Error>> {
    // 创建一个随机账户作为交易者
    let payer = Keypair::new();
    let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();
    let swqos_configs = vec![
        SwqosConfig::Jito(SwqosRegion::Frankfurt),
        SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Default(rpc_url.clone()),
    ];
    // Define cluster configuration
    let trade_config = TradeConfig {
        rpc_url: rpc_url.clone(),
        commitment: CommitmentConfig::confirmed(),
        priority_fee: PriorityFee::default(),
        swqos_configs,
        lookup_table_key: None,
    };
    let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    let amount = 100_000; // 0.0001 SOL
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;
    let mint = Pubkey::from_str("xxxxxxx")?;
    let bonk_params = BonkParams {
        virtual_base: None,
        virtual_quote: None,
        real_base_before: None,
        real_quote_before: None,
        auto_handle_wsol: true,
    };
    println!("Buying tokens from letsbonk.fun...");
    // buy
    let buy_params = BuyParams {
        rpc: Some(solana_trade_client.rpc.clone()),
        payer: solana_trade_client.payer.clone(),
        mint: mint,
        creator: Pubkey::default(),
        amount_sol: amount,
        slippage_basis_points: None,
        priority_fee: solana_trade_client.trade_config.clone().priority_fee,
        lookup_table_key: solana_trade_client.trade_config.clone().lookup_table_key,
        recent_blockhash,
        data_size_limit: 0,
        protocol_params: Box::new(bonk_params.clone()),
    };
    let buy_with_tip_params = buy_params
        .clone()
        .with_tip(solana_trade_client.swqos_clients.clone());
    solana_trade_client
        .buy_use_buy_params(buy_with_tip_params, None)
        .await?;
    // sell
    println!("Selling tokens from letsbonk.fun...");
    let sell_params = SellParams {
        rpc: Some(solana_trade_client.rpc.clone()),
        payer: solana_trade_client.payer.clone(),
        mint: mint,
        creator: Pubkey::default(),
        amount_token: None,
        slippage_basis_points: None,
        priority_fee: solana_trade_client.trade_config.clone().priority_fee,
        lookup_table_key: solana_trade_client.trade_config.clone().lookup_table_key,
        recent_blockhash,
        protocol_params: Box::new(bonk_params.clone()),
    };
    solana_trade_client
        .sell_by_amount_use_sell_params(sell_params)
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
    let protocols = vec![
        Protocol::PumpFun,
        Protocol::PumpSwap,
        Protocol::Bonk,
    ];
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
    let protocols = vec![
        Protocol::PumpFun,
        Protocol::PumpSwap,
        Protocol::Bonk,
    ];
    shred_stream
        .shredstream_subscribe(protocols, None, callback)
        .await?;

    Ok(())
}
