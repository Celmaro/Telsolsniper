# Sol Trade SDK
[中文](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README_CN.md) | [English](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README.md) | [Telegram](https://t.me/fnzero_group)

一个全面的 Rust SDK，用于与 Solana DEX 交易程序进行无缝交互。此 SDK 提供强大的工具和接口集，将 PumpFun、PumpSwap 和 Bonk 功能集成到您的应用程序中。

## 项目特性

1. **PumpFun 交易**: 支持`购买`、`卖出`功能
2. **PumpSwap 交易**: 支持 PumpSwap 池的交易操作
3. **Bonk 交易**: 支持 Bonk 的交易操作
4. **Raydium CPMM 交易**: 支持 Raydium CPMM (Concentrated Pool Market Maker) 的交易操作
5. **Raydium AMM V4 交易**: 支持 Raydium AMM V4 (Automated Market Maker) 的交易操作
6. **事件订阅**: 订阅 PumpFun、PumpSwap、Bonk、Raydium CPMM 和 Raydium AMM V4 程序的交易事件
7. **Yellowstone gRPC**: 使用 Yellowstone gRPC 订阅程序事件
8. **ShredStream 支持**: 使用 ShredStream 订阅程序事件
9. **多种 MEV 保护**: 支持 Jito、Nextblock、ZeroSlot、Temporal、Bloxroute、Node1 等服务
10. **并发交易**: 同时使用多个 MEV 服务发送交易，最快的成功，其他失败
11. **统一交易接口**: 使用统一的交易协议枚举进行交易操作
12. **中间件系统**: 支持自定义指令中间件，可在交易执行前对指令进行修改、添加或移除

## 安装

### 直接克隆

将此项目克隆到您的项目目录：

```bash
cd your_project_root_directory
git clone https://github.com/0xfnzero/sol-trade-sdk
```

在您的`Cargo.toml`中添加依赖：

```toml
# 添加到您的 Cargo.toml
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.4.5" }
```

### 使用 crates.io

```toml
# 添加到您的 Cargo.toml
sol-trade-sdk = "0.4.5"
```

## 使用示例

### 重要参数说明

#### auto_handle_wsol 参数

在 PumpSwap、Bonk、Raydium CPMM 交易中，`auto_handle_wsol` 参数用于自动处理 wSOL（Wrapped SOL）：

- **作用机制**：
  - 当 `auto_handle_wsol: true` 时，SDK 会自动处理 SOL 与 wSOL 之间的转换
  - 买入时：自动将 SOL 包装为 wSOL 进行交易
  - 卖出时：自动将获得的 wSOL 解包装为 SOL
  - 默认值为 `true`

#### lookup_table_key 参数

`lookup_table_key` 参数是一个可选的 `Pubkey`，用于指定地址查找表以优化交易：

- **用途**：地址查找表可以通过存储常用地址来减少交易大小并提高执行速度
- **使用方法**：
  - 可以在 `TradeConfig` 中全局设置，用于所有交易
  - 可以在 `buy()` 和 `sell()` 方法中按交易覆盖
  - 如果不提供，默认为 `None`
- **优势**：
  - 通过从查找表引用地址来减少交易大小
  - 提高交易成功率和速度
  - 特别适用于具有许多账户引用的复杂交易

### 1. 事件订阅 - 监听代币交易

#### 1.1 使用 Yellowstone gRPC 订阅事件

```rust
use sol_trade_sdk::solana_streamer_sdk::{
    streaming::{
        event_parser::{
            protocols::{
                bonk::{BonkPoolCreateEvent, BonkTradeEvent},
                pumpfun::{PumpFunCreateTokenEvent, PumpFunTradeEvent},
                pumpswap::{
                    PumpSwapBuyEvent, PumpSwapCreatePoolEvent, PumpSwapDepositEvent,
                    PumpSwapSellEvent, PumpSwapWithdrawEvent,
                },
                raydium_cpmm::RaydiumCpmmSwapEvent,
            },
            Protocol, UnifiedEvent,
        },
        yellowstone_grpc::{AccountFilter, TransactionFilter},
        YellowstoneGrpc,
    },
    match_event,
};

use solana_streamer_sdk::streaming::event_parser::protocols::{
    bonk::parser::BONK_PROGRAM_ID, 
    pumpfun::parser::PUMPFUN_PROGRAM_ID, 
    pumpswap::parser::PUMPSWAP_PROGRAM_ID, 
    raydium_amm_v4::parser::RAYDIUM_AMM_V4_PROGRAM_ID, 
    raydium_clmm::parser::RAYDIUM_CLMM_PROGRAM_ID, 
    raydium_cpmm::parser::RAYDIUM_CPMM_PROGRAM_ID
};

async fn test_grpc() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 GRPC 客户端订阅事件
    println!("正在订阅 GRPC 事件...");

    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    // 定义回调函数处理事件
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
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                println!("Raydium CPMM Swap event: {:?}", e);
            },
            // ..... 
            // 更多的事件和说明请参考 https://github.com/0xfnzero/solana-streamer
        });
    };

    // 订阅多个协议的事件
    println!("开始监听事件，按 Ctrl+C 停止...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk, Protocol::RaydiumCpmm];
    
    // 过滤账户
    let account_include = vec![
        PUMPFUN_PROGRAM_ID.to_string(),      // 监听 pumpfun 程序 ID
        PUMPSWAP_PROGRAM_ID.to_string(),     // 监听 pumpswap 程序 ID
        BONK_PROGRAM_ID.to_string(),         // 监听 bonk 程序 ID
        RAYDIUM_CPMM_PROGRAM_ID.to_string(), // 监听 raydium_cpmm 程序 ID
        RAYDIUM_CLMM_PROGRAM_ID.to_string(), // 监听 raydium_clmm 程序 ID
        RAYDIUM_AMM_V4_PROGRAM_ID.to_string(), // 监听 raydium_amm_v4 程序 ID
        "xxxxxxxx".to_string(),              // 监听特定账户
    ];
    let account_exclude = vec![];
    let account_required = vec![];

    // 监听交易数据
    let transaction_filter = TransactionFilter {
        account_include: account_include.clone(),
        account_exclude,
        account_required,
    };

    // 监听属于owner程序的账号数据 -> 账号事件监听
    let account_filter = AccountFilter { 
        account: vec![], 
        owner: account_include.clone() 
    };

    grpc.subscribe_events_immediate(
        protocols,
        None,
        transaction_filter,
        account_filter,
        None,
        None,
        callback,
    )
    .await?;

    Ok(())
}
```

#### 1.2 使用 ShredStream 订阅事件

```rust
use sol_trade_sdk::solana_streamer_sdk::streaming::ShredStreamGrpc;

async fn test_shreds() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 ShredStream 客户端订阅事件
    println!("正在订阅 ShredStream 事件...");

    let shred_stream = ShredStreamGrpc::new("http://127.0.0.1:10800".to_string()).await?;

    // 定义回调函数处理事件（与上面相同）
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
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                println!("Raydium CPMM Swap event: {:?}", e);
            },
            // ..... 
            // 更多的事件和说明请参考 https://github.com/0xfnzero/solana-streamer
        });
    };

    // 订阅事件
    println!("开始监听事件，按 Ctrl+C 停止...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk, Protocol::RaydiumCpmm];
    shred_stream
        .shredstream_subscribe(protocols, None, None, callback)
        .await?;

    Ok(())
}
```

### 2. 初始化 SolanaTrade 实例

#### 2.1 SWQOS 服务配置说明

在配置 SWQOS 服务时，需要注意不同服务的参数要求：

- **Jito**: 第一个参数是 UUID，如果没有 UUID 则传空字符串 `""`
- **NextBlock**: 第一个参数是 API Token, 点击这里 https://nextblock.io/auth?refCode=fnzero 获取免费key立即加速你的交易！
- **Bloxroute**: 第一个参数是 API Token  
- **ZeroSlot**: 第一个参数是 API Token
- **Temporal**: 第一个参数是 API Token
- **FlashBlock**: 第一个参数是 API Token, 添加tg官方客服https://t.me/FlashBlock_Official 获取免费key立即加速你的交易！官方文档: https://doc.flashblock.trade/
- **Node1**: 第一个参数是 API Token, 添加tg官方客服https://t.me/node1_me 获取免费key立即加速你的交易！官方文档: https://node1.me/docs.html

```rust
use std::{str::FromStr, sync::Arc};
use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig},
    swqos::{SwqosConfig, SwqosRegion},
    SolanaTrade
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

/// 创建 SolanaTrade 客户端的示例
async fn test_create_solana_trade_client() -> AnyResult<SolanaTrade> {
    println!("Creating SolanaTrade client...");

    let payer = Keypair::new();
    let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();

    // 配置各种 SWQOS 服务
    let swqos_configs = vec![
        // 第一个参数是 uuid，如果没有 uuid 则传空字符串
        SwqosConfig::Jito("your uuid".to_string(), SwqosRegion::Frankfurt), 
        // 点击这里 https://nextblock.io/auth?refCode=fnzero 获取免费key立即加速你的交易！
        SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
        // 添加tg官方客服 https://t.me/FlashBlock_Official 获取免费 FlashBlock key
        SwqosConfig::FlashBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        // 添加tg官方客服 https://t.me/node1_me 获取免费 Node1 key
        SwqosConfig::Node1("your api_token".to_string(), SwqosRegion::Frankfurt),
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
```

### 3. PumpFun 交易操作

```rust
use sol_trade_sdk::{
    common::{bonding_curve::BondingCurveAccount, AnyResult},
    constants::pumpfun::global_constants::TOKEN_TOTAL_SUPPLY,
    trading::{core::params::PumpFunParams, factory::DexType},
};
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::PumpFunTradeEvent;

// pumpfun 狙击者交易
async fn test_pumpfun_sniper_trade_with_shreds(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    println!("Testing PumpFun trading...");

    // 如果不是开发者购买，则返回
    if !trade_info.is_dev_create_token_trade {
        return Ok(());
    }

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = trade_info.mint;
    let creator = trade_info.creator;
    let dev_sol_amount = trade_info.max_sol_cost;
    let dev_token_amount = trade_info.token_amount;
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;
    
    println!("Buying tokens from PumpFun...");
    
    // 我本次交易所花的的sol金额
    let buy_sol_amount = 100_000;
    trade_client.buy(
        DexType::PumpFun,
        mint_pubkey,
        Some(creator),
        buy_sol_amount,
        slippage_basis_points,
        recent_blockhash,
        None,
        Box::new(PumpFunParams::from_dev_trade(
            &mint_pubkey,
            dev_token_amount,
            dev_sol_amount,
            creator,
            None,
        )),
        None,
    )
    .await?;

    Ok(())
}

// pumpfun 跟单交易
async fn test_pumpfun_copy_trade_with_grpc(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    println!("Testing PumpFun trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = trade_info.mint;
    let creator = trade_info.creator;
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;

    println!("Buying tokens from PumpFun...");

    // 我本次交易所花的的sol金额
    let buy_sol_amount = 100_000;

    trade_client.buy(
        DexType::PumpFun,
        mint_pubkey,
        Some(creator),
        buy_sol_amount,
        slippage_basis_points,
        recent_blockhash,
        None,
        Box::new(PumpFunParams::from_trade(&trade_info, None)),
        None,
    )
    .await?;

    Ok(())
}

// pumpfun 卖出token
async fn test_pumpfun_sell(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    let trade_client = test_create_solana_trade_client().await?;
    let mint_pubkey = trade_info.mint;
    let creator = trade_info.creator;
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;
    
    let amount_token = 100_000_000; 
    trade_client.sell(
        DexType::PumpFun,
        mint_pubkey,
        Some(creator),
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        Box::new(PumpFunParams::from_trade(&trade_info, None)),
        None,
    )
    .await?;

    Ok(())
}
```

### 4. PumpSwap 交易操作

```rust
use sol_trade_sdk::{
    common::AnyResult,
    trading::{core::params::PumpSwapParams, factory::DexType},
};
use solana_sdk::{pubkey::Pubkey, str::FromStr};

async fn test_pumpswap() -> AnyResult<()> {
    println!("Testing PumpSwap trading...");

    let client = test_create_solana_trade_client().await?;
    let creator = Pubkey::from_str("11111111111111111111111111111111")?;
    let mint_pubkey = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?;
    let buy_sol_cost = 100_000;
    let slippage_basis_points = Some(100);
    let recent_blockhash = client.rpc.get_latest_blockhash().await?;
    let pool_address = Pubkey::from_str("xxxxxxx")?;
    let base_mint = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?;
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112")?;
    let pool_base_token_reserves = 0; // 输入正确的值
    let pool_quote_token_reserves = 0; // 输入正确的值

    // 买入代币
    println!("Buying tokens from PumpSwap...");
    client.buy(
        DexType::PumpSwap,
        mint_pubkey,
        Some(creator),
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        // 通过 RPC 调用，会增加延迟。可以通过使用 from_buy_trade 或手动初始化 PumpSwapParams 来优化
        Box::new(PumpSwapParams::from_pool_address_by_rpc(&client.rpc, &pool_address).await?),
        None,
    ).await?;

    // 卖出代币
    println!("Selling tokens from PumpSwap...");
    let amount_token = 0;
    client.sell(
        DexType::PumpSwap,
        mint_pubkey,
        Some(creator),
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        // 通过 RPC 调用，会增加延迟。可以通过使用 from_sell_trade 或手动初始化 PumpSwapParams 来优化
        Box::new(PumpSwapParams::from_pool_address_by_rpc(&client.rpc, &pool_address).await?),
        None,
    ).await?;

    Ok(())
}
```

### 5. Raydium CPMM 交易操作

```rust
use sol_trade_sdk::{
    trading::{
        core::params::RaydiumCpmmParams, 
        factory::DexType, 
        raydium_cpmm::common::{get_buy_token_amount, get_sell_sol_amount}
    },
};
use solana_sdk::{pubkey::Pubkey, str::FromStr};
use spl_token; // 用于标准 SPL Token
// use spl_token_2022; // 用于 Token 2022 标准（如果需要）

async fn test_raydium_cpmm() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Raydium CPMM trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = Pubkey::from_str("xxxxxxxx")?; // 代币地址
    let buy_sol_cost = 100_000; // 0.0001 SOL（以lamports为单位）
    let slippage_basis_points = Some(100); // 1% 滑点
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;
    let pool_state = Pubkey::from_str("xxxxxxx")?; // 池状态地址

    // 计算买入时预期获得的代币数量
    let buy_amount_out = get_buy_token_amount(&trade_client.rpc, &pool_state, buy_sol_cost).await?;

    println!("Buying tokens from Raydium CPMM...");
    trade_client.buy(
        DexType::RaydiumCpmm,
        mint_pubkey,
        None,
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        // 通过 RPC 调用，会增加延迟，或手动初始化 RaydiumCpmmParams
        Box::new(
            RaydiumCpmmParams::from_pool_address_by_rpc(&trade_client.rpc, &pool_state).await?,
        ),
        None,
    ).await?;

    println!("Selling tokens from Raydium CPMM...");
    let amount_token = 100_000_000; // 卖出代币数量
    let sell_sol_amount = get_sell_sol_amount(&trade_client.rpc, &pool_state, amount_token).await?;
    
    trade_client.sell(
        DexType::RaydiumCpmm,
        mint_pubkey,
        None,
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        // 通过 RPC 调用，会增加延迟，或手动初始化 RaydiumCpmmParams
        Box::new(
            RaydiumCpmmParams::from_pool_address_by_rpc(&trade_client.rpc, &pool_state).await?,
        ),
        None,
    ).await?;

    Ok(())
}
```

### 6. Raydium AMM V4 交易操作

```rust
use sol_trade_sdk::trading::core::params::RaydiumAmmV4Params;

async fn test_raydium_amm_v4() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Raydium AMM V4 trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = Pubkey::from_str("xxxxxxx")?; // 代币地址
    let buy_sol_cost = 100_000; // 0.0001 SOL（以lamports为单位）
    let slippage_basis_points = Some(100); // 1% 滑点
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;
    let amm_address = Pubkey::from_str("xxxxxx")?; // AMM 池地址

    println!("Buying tokens from Raydium AMM V4...");
    trade_client.buy(
        DexType::RaydiumAmmV4,
        mint_pubkey,
        None,
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        // 通过 RPC 调用，会增加延迟，或使用 from_amm_info_and_reserves 或手动初始化 RaydiumAmmV4Params
        Box::new(
            RaydiumAmmV4Params::from_amm_address_by_rpc(&trade_client.rpc, amm_address).await?,
        ),
        None,
    ).await?;

    println!("Selling tokens from Raydium AMM V4...");
    let amount_token = 100_000_000; // 卖出代币数量
    
    trade_client.sell(
        DexType::RaydiumAmmV4,
        mint_pubkey,
        None,
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        // 通过 RPC 调用，会增加延迟，或使用 from_amm_info_and_reserves 或手动初始化 RaydiumAmmV4Params
        Box::new(
            RaydiumAmmV4Params::from_amm_address_by_rpc(&trade_client.rpc, amm_address).await?,
        ),
        None,
    ).await?;

    Ok(())
}
```

### 7. Bonk 交易操作

```rust
use sol_trade_sdk::{
    common::AnyResult,
    trading::{core::params::BonkParams, factory::DexType},
};
use sol_trade_sdk::solana_streamer_sdk::streaming::event_parser::protocols::bonk::BonkTradeEvent;
use solana_sdk::{pubkey::Pubkey, str::FromStr};

// bonk 狙击者交易
async fn test_bonk_sniper_trade_with_shreds(trade_info: BonkTradeEvent) -> AnyResult<()> {
    println!("Testing Bonk trading...");

    if !trade_info.is_dev_create_token_trade {
        return Ok(());
    }

    let trade_client = test_create_solana_trade_client().await?;
    let mint_pubkey = Pubkey::from_str("xxxxxxx")?;
    let buy_sol_cost = 100_000;
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;

    println!("Buying tokens from letsbonk.fun...");
    
    // 使用开发者交易信息构建 BonkParams，可以节约交易时间
    trade_client.buy(
        DexType::Bonk,
        mint_pubkey,
        None,
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        Box::new(BonkParams::from_dev_trade(trade_info.clone())),
        None,
    ).await?;

    println!("Selling tokens from letsbonk.fun...");
    let amount_token = 0;
    trade_client.sell(
        DexType::Bonk,
        mint_pubkey,
        None,
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        Box::new(BonkParams::from_dev_trade(trade_info)),
        None,
    ).await?;

    Ok(())
}

// bonk 跟单交易
async fn test_bonk_copy_trade_with_grpc(trade_info: BonkTradeEvent) -> AnyResult<()> {
    println!("Testing Bonk trading...");

    let trade_client = test_create_solana_trade_client().await?;
    let mint_pubkey = Pubkey::from_str("xxxxxxx")?;
    let buy_sol_cost = 100_000;
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;

    println!("Buying tokens from letsbonk.fun...");
    
    // 使用交易事件信息构建 BonkParams，可以节约交易时间
    trade_client.buy(
        DexType::Bonk,
        mint_pubkey,
        None,
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        Box::new(BonkParams::from_trade(trade_info.clone())),
        None,
    ).await?;

    println!("Selling tokens from letsbonk.fun...");
    let amount_token = 0;
    trade_client.sell(
        DexType::Bonk,
        mint_pubkey,
        None,
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        Box::new(BonkParams::from_trade(trade_info)),
        None,
    ).await?;

    Ok(())
}

// bonk 普通交易
async fn test_bonk() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Bonk trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = Pubkey::from_str("xxxxxxx")?;
    let buy_sol_amount = 100_000; 
    let slippage_basis_points = Some(100); // 1%
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;

    println!("Buying tokens from letsbonk.fun...");

    trade_client.buy(
        DexType::Bonk,
        mint_pubkey,
        None,
        buy_sol_amount,
        slippage_basis_points,
        recent_blockhash,
        None,
        // 通过 RPC 调用，会增加延迟。可以通过使用 from_trade 或手动初始化 BonkParams 来优化
        Box::new(BonkParams::from_mint_by_rpc(&trade_client.rpc, &mint_pubkey).await?),
        None,
    )
    .await?;
    
    println!("Selling tokens from letsbonk.fun...");

    let amount_token = 100_000; 
    trade_client.sell(
        DexType::Bonk,
        mint_pubkey,
        None,
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        // 通过 RPC 调用，会增加延迟。可以通过使用 from_trade 或手动初始化 BonkParams 来优化
        Box::new(BonkParams::from_mint_by_rpc(&trade_client.rpc, &mint_pubkey).await?),
        None,
    )
    .await?;

    Ok(())
}
```

### 8. 中间件系统

SDK 提供了强大的中间件系统，允许您在交易执行前对指令进行修改、添加或移除。这为您提供了极大的灵活性来自定义交易行为。

#### 8.1 使用内置的日志中间件

```rust
use sol_trade_sdk::{
    trading::{
        factory::DexType,
        middleware::builtin::LoggingMiddleware,
        MiddlewareManager,
    },
};

async fn test_middleware() -> AnyResult<()> {
    let mut client = test_create_solana_trade_client().await?;
    
    // SDK 内置的示例中间件，打印指令信息
    // 您可以参考 LoggingMiddleware 来实现 InstructionMiddleware trait 来实现自己的中间件
    let middleware_manager = MiddlewareManager::new()
        .add_middleware(Box::new(LoggingMiddleware));
    
    client = client.with_middleware_manager(middleware_manager);
    
    let creator = Pubkey::from_str("11111111111111111111111111111111")?;
    let mint_pubkey = Pubkey::from_str("xxxxx")?;
    let buy_sol_cost = 100_000;
    let slippage_basis_points = Some(100);
    let recent_blockhash = client.rpc.get_latest_blockhash().await?;
    let pool_address = Pubkey::from_str("xxxx")?;
    
    // 购买代币
    println!("Buying tokens from PumpSwap...");
    client
        .buy(
            DexType::PumpSwap,
            mint_pubkey,
            Some(creator),
            buy_sol_cost,
            slippage_basis_points,
            recent_blockhash,
            None,
            Box::new(PumpSwapParams::from_pool_address_by_rpc(&client.rpc, &pool_address).await?),
            None,
        )
        .await?;
    Ok(())
}
```

#### 8.2 创建自定义中间件

您可以通过实现 `InstructionMiddleware` trait 来创建自定义中间件：

```rust
use sol_trade_sdk::trading::middleware::traits::InstructionMiddleware;
use anyhow::Result;
use solana_sdk::instruction::Instruction;

/// 自定义中间件示例 - 添加额外指令
#[derive(Clone)]
pub struct CustomMiddleware;

impl InstructionMiddleware for CustomMiddleware {
    fn name(&self) -> &'static str {
        "CustomMiddleware"
    }

    fn process_protocol_instructions(
        &self,
        protocol_instructions: Vec<Instruction>,
        protocol_name: String,
        is_buy: bool,
    ) -> Result<Vec<Instruction>> {
        println!("自定义中间件处理中，协议: {}", protocol_name);
        
        // 在这里您可以：
        // 1. 修改现有指令
        // 2. 添加新指令
        // 3. 移除特定指令
        
        // 示例：在指令开始前添加一个自定义指令
        // let custom_instruction = create_your_custom_instruction();
        // instructions.insert(0, custom_instruction);
        
        Ok(protocol_instructions)
    }

    fn process_full_instructions(
        &self,
        full_instructions: Vec<Instruction>,
        protocol_name: String,
        is_buy: bool,
    ) -> Result<Vec<Instruction>> {
        println!("自定义中间件处理中，指令数量: {}", full_instructions.len());
        Ok(full_instructions)
    }

    fn clone_box(&self) -> Box<dyn InstructionMiddleware> {
        Box::new(self.clone())
    }
}

// 使用自定义中间件
async fn test_custom_middleware() -> AnyResult<()> {
    let mut client = test_create_solana_trade_client().await?;
    
    let middleware_manager = MiddlewareManager::new()
        .add_middleware(Box::new(LoggingMiddleware))           // 日志中间件
        .add_middleware(Box::new(CustomMiddleware));
    
    client = client.with_middleware_manager(middleware_manager);
    
    // 现在所有交易都会通过您的中间件处理
    // ...
    Ok(())
}
```

#### 8.3 中间件执行顺序

中间件按照添加顺序依次执行：

```rust
let middleware_manager = MiddlewareManager::new()
    .add_middleware(Box::new(FirstMiddleware))   // 第一个执行
    .add_middleware(Box::new(SecondMiddleware))  // 第二个执行
    .add_middleware(Box::new(ThirdMiddleware));  // 最后执行
```

### 9. 自定义优先费用配置

```rust
use sol_trade_sdk::common::PriorityFee;

// 自定义优先费用配置
let priority_fee = PriorityFee {
    unit_limit: 190000,
    unit_price: 1000000,
    rpc_unit_limit: 500000,
    rpc_unit_price: 500000,
    buy_tip_fee: 0.001,
    buy_tip_fees: vec![0.001, 0.002],
    sell_tip_fee: 0.0001,
};

// 在TradeConfig中使用自定义优先费用
let trade_config = TradeConfig {
    rpc_url: rpc_url.clone(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee, // 使用自定义优先费用
    swqos_configs,
    lookup_table_key: None,
};
```

## 支持的交易平台

- **PumpFun**: 主要的 meme 币交易平台
- **PumpSwap**: PumpFun 的交换协议
- **Bonk**: 代币发行平台（letsbonk.fun）
- **Raydium CPMM**: Raydium 的集中流动性做市商协议
- **Raydium AMM V4**: Raydium 的自动做市商 V4 协议

## MEV 保护服务

- **Jito**: 高性能区块空间
- **NextBlock**: 快速交易执行
- **ZeroSlot**: 零延迟交易
- **Temporal**: 时间敏感交易
- **Bloxroute**: 区块链网络加速
- **FlashBlock**: 高速交易执行，支持 API 密钥认证 - [官方文档](https://doc.flashblock.trade/)
- **Node1**: 高速交易执行，支持 API 密钥认证 - [官方文档](https://node1.me/docs.html)

## 新架构特性

### 统一交易接口

- **TradingProtocol 枚举**: 使用统一的协议枚举（PumpFun、PumpSwap、Bonk、RaydiumCpmm、RaydiumAmmV4）
- **统一的 buy/sell 方法**: 所有协议都使用相同的交易方法签名
- **协议特定参数**: 每个协议都有自己的参数结构（PumpFunParams、RaydiumCpmmParams、RaydiumAmmV4Params 等）

### 事件解析系统

- **统一事件接口**: 所有协议事件都实现 UnifiedEvent 特征
- **协议特定事件**: 每个协议都有自己的事件类型
- **事件工厂**: 自动识别和解析不同协议的事件

### 交易引擎

- **统一交易接口**: 所有交易操作都使用相同的方法
- **协议抽象**: 支持多个协议的交易操作
- **并发执行**: 支持同时向多个 MEV 服务发送交易

## 价格计算工具

SDK 包含所有支持协议的价格计算工具，位于 `src/utils/price/` 目录。

## 数量计算工具

SDK 提供各种协议的交易数量计算功能，位于 `src/utils/calc/` 目录：

- **通用计算函数**: 提供通用的手续费计算和除法运算工具
- **协议特定计算**: 针对每个协议的特定计算逻辑
  - **PumpFun**: 基于联合曲线的代币购买/销售数量计算
  - **PumpSwap**: 支持多种交易对的数量计算
  - **Raydium AMM V4**: 自动做市商池的数量和手续费计算
  - **Raydium CPMM**: 恒定乘积做市商的数量计算
  - **Bonk**: 专门的 Bonk 代币计算逻辑

主要功能包括：
- 根据输入金额计算输出数量
- 手续费计算和分配
- 滑点保护计算
- 流动性池状态计算

## 项目结构

```
src/
├── common/           # 通用功能和工具
├── constants/        # 常量定义
├── instruction/      # 指令构建
├── swqos/            # MEV服务客户端
├── trading/          # 统一交易引擎
│   ├── common/       # 通用交易工具
│   ├── core/         # 核心交易引擎
│   ├── middleware/   # 中间件系统
│   │   ├── builtin.rs    # 内置中间件实现
│   │   ├── traits.rs     # 中间件 trait 定义
│   │   └── mod.rs        # 中间件模块
│   ├── bonk/         # Bonk交易实现
│   ├── pumpfun/      # PumpFun交易实现
│   ├── pumpswap/     # PumpSwap交易实现
│   ├── raydium_cpmm/ # Raydium CPMM交易实现
│   ├── raydium_amm_v4/ # Raydium AMM V4交易实现
│   └── factory.rs    # 交易工厂
├── utils/            # 工具函数
│   ├── price/        # 价格计算工具
│   │   ├── common.rs       # 通用价格函数
│   │   ├── bonk.rs         # Bonk 价格计算
│   │   ├── pumpfun.rs      # PumpFun 价格计算
│   │   ├── pumpswap.rs     # PumpSwap 价格计算
│   │   ├── raydium_cpmm.rs # Raydium CPMM 价格计算
│   │   ├── raydium_clmm.rs # Raydium CLMM 价格计算
│   │   └── raydium_amm_v4.rs # Raydium AMM V4 价格计算
│   └── calc/         # 数量计算工具
│       ├── common.rs       # 通用计算函数
│       ├── bonk.rs         # Bonk 数量计算
│       ├── pumpfun.rs      # PumpFun 数量计算
│       ├── pumpswap.rs     # PumpSwap 数量计算
│       ├── raydium_cpmm.rs # Raydium CPMM 数量计算
│       └── raydium_amm_v4.rs # Raydium AMM V4 数量计算
├── lib.rs            # 主库文件
└── main.rs           # 示例程序
```

## 许可证

MIT 许可证

## 联系方式

- 项目仓库: https://github.com/0xfnzero/sol-trade-sdk
- Telegram 群组: https://t.me/fnzero_group

## 重要注意事项

1. 在主网使用前请充分测试
2. 正确设置私钥和 API 令牌
3. 注意滑点设置避免交易失败
4. 监控余额和交易费用
5. 遵循相关法律法规

## 语言版本

- [English](README.md)
- [中文](README_CN.md)
