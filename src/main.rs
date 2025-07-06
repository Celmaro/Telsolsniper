use std::{str::FromStr, sync::Arc};

use sol_trade_sdk::{accounts::BondingCurveAccount, common::{pumpfun::PumpfunEvent, pumpswap::PumpSwapEvent, raydium::RaydiumEvent, AnyResult, PriorityFee, TradeConfig}, constants::pumpfun::global_constants::TOKEN_TOTAL_SUPPLY, grpc::{ShredStreamGrpc, YellowstoneGrpc}, pumpfun::common::get_bonding_curve_pda, swqos::{SwqosConfig, SwqosRegion, SwqosType}, SolanaTrade};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // test_pumpfun_with_shreds().await?;
    // test_pumpfun_with_grpc().await?;
    // test_pumpswap_with_shreds().await?;
    // test_pumpswap_with_grpc().await?;
    // test_raydium_with_shreds().await?;
    // test_raydium_with_grpc().await?;
    // test_pumpfun_sniper().await?;
    // test_pumpfun().await?;
    test_pumpswap().await?;
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

async fn test_raydium_with_shreds() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 ShredStream 客户端订阅 Raydium 事件
    println!("正在订阅 Raydium ShredStream 事件...");

    let grpc_client = ShredStreamGrpc::new("http://127.0.0.1:10800".to_string()).await?;

    // 定义回调函数处理 Raydium 事件
    let callback = |event: RaydiumEvent| {
        match event {
            RaydiumEvent::V4Swap(v4_swap_event) => {
                println!("v4_swap_event: {:?}", v4_swap_event);
            }
            RaydiumEvent::SwapBaseInput(swap_base_input_event) => {
                println!("swap_base_input_event: {:?}", swap_base_input_event);
            }
            RaydiumEvent::SwapBaseOutput(swap_base_output_event) => {
                println!("swap_base_output_event: {:?}", swap_base_output_event);
            }
            RaydiumEvent::Error(err) => {
                println!("error: {}", err);
            }
        }
    };
    // 订阅 Raydium 事件
    println!("开始监听 Raydium 事件，按 Ctrl+C 停止...");

    grpc_client.shredstream_subscribe_raydium(callback).await?;

    Ok(())
}

async fn test_raydium_with_grpc() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 GRPC 客户端订阅 Raydium 事件
    println!("正在订阅 Raydium GRPC 事件...");

    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    // 定义回调函数处理 PumpSwap 事件
    let callback = |event: RaydiumEvent| match event {
        RaydiumEvent::V4Swap(v4_swap_event) => {
            println!("v4_swap_event: {:?}", v4_swap_event);
        }
        RaydiumEvent::SwapBaseInput(swap_base_input_event) => {
            println!("swap_base_input_event: {:?}", swap_base_input_event);
        }
        RaydiumEvent::SwapBaseOutput(swap_base_output_event) => {
            println!("swap_base_output_event: {:?}", swap_base_output_event);
        }
        RaydiumEvent::Error(err) => {
            println!("error: {}", err);
        }
    };
    // 订阅 Raydium 事件
    println!("开始监听 Raydium 事件，按 Ctrl+C 停止...");

    grpc.subscribe_raydium(callback).await?;

    Ok(())
}

async fn test_pumpfun_sniper() -> AnyResult<()> {
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


    let creator = Pubkey::from_str("xxx")?; // dev account
    let buy_sol_cost = 500_000; // 0.0005 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();
    let mint_pubkey = Pubkey::from_str("xxx")?; // token mint
    println!("Sniping buy tokens from PumpFun...");
    // get bonding curve
    let dev_buy_token = 100_000; // test value
    let dev_cost_sol = 100_000; // test value
    let bonding_curve = BondingCurveAccount::new(&mint_pubkey, dev_buy_token, dev_cost_sol, creator);
    solana_trade_client
        .sniper_buy(
            mint_pubkey,
            creator,
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            Some(Arc::new(bonding_curve)),
        )
        .await?;
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
    let creator = Pubkey::from_str("xxx")?; // dev account
    let buy_sol_cost = 500_000; // 0.0005 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();
    let trade_platform = "pumpfun".to_string();
    let mint_pubkey = Pubkey::from_str("xxx")?; // token mint
    println!("Buying tokens from PumpFun...");
    // get bonding curve
    // Relevant on-chain information can be obtained from rpc/grpc
    let virtual_token_reserves = 0;
    let virtual_sol_reserves = 0;
    let real_token_reserves = 0;
    let real_sol_reserves = 0;
    let bonding_curve = BondingCurveAccount {
        discriminator: 0,
        account: get_bonding_curve_pda(&mint_pubkey).unwrap(),
        virtual_token_reserves: virtual_token_reserves,
        virtual_sol_reserves: virtual_sol_reserves,
        real_token_reserves: real_token_reserves,
        real_sol_reserves: real_sol_reserves,
        token_total_supply: TOKEN_TOTAL_SUPPLY,
        complete: false,
        creator: creator,
    };
    solana_trade_client
        .buy(
            mint_pubkey,
            creator,
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            Some(Arc::new(bonding_curve)),
            trade_platform.clone(),
        )
        .await?;
    // Sell 30% * amount_token quantity
    // solana_trade_client
    //     .sell_by_percent(
    //         mint_pubkey,
    //         creator,
    //         30,
    //         100,
    //         recent_blockhash,
    //         trade_platform.clone(),
    //     )
    //     .await?;
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
    let buy_sol_cost = 500_000; // 0.0005 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();
    let trade_platform = "pumpswap".to_string();
    let mint_pubkey = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?; // token mint
    println!("Buying tokens from PumpSwap...");
    solana_trade_client
        .buy(
            mint_pubkey,
            creator,
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            trade_platform.clone(),
        )
        .await?;
    // Sell 30% * amount_token quantity
    // solana_trade_client
    //     .sell_by_percent(
    //         mint_pubkey,
    //         creator,
    //         30,
    //         100,
    //         recent_blockhash,
    //         trade_platform.clone(),
    //     )
    //     .await?;
    Ok(())
}
