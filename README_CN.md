# Sol Trade SDK

一个全面的 Rust SDK，用于与 Solana DEX 交易程序进行无缝交互。此 SDK 提供强大的工具和接口集，将 PumpFun、PumpSwap 和 Bonk 功能集成到您的应用程序中。

## 项目特性

1. **PumpFun 交易**: 支持`购买`、`卖出`功能
2. **PumpSwap 交易**: 支持 PumpSwap 池的交易操作
3. **Bonk 交易**: 支持 Bonk 的交易操作
4. **事件订阅**: 订阅 PumpFun、PumpSwap 和 Bonk 程序的交易事件
5. **Yellowstone gRPC**: 使用 Yellowstone gRPC 订阅程序事件
6. **ShredStream 支持**: 使用 ShredStream 订阅程序事件
7. **多种 MEV 保护**: 支持 Jito、Nextblock、ZeroSlot、Temporal、Bloxroute 等服务
8. **并发交易**: 同时使用多个 MEV 服务发送交易，最快的成功，其他失败
9. **统一交易接口**: 使用统一的参数结构进行交易操作

## 安装

将此项目克隆到您的项目目录：

```bash
cd your_project_root_directory
git clone https://github.com/0xfnzero/sol-trade-sdk
```

在您的`Cargo.toml`中添加依赖：

```toml
# 添加到您的 Cargo.toml
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.1.0" }
```

## 使用示例

### 1. 事件订阅 - 监听代币交易

#### 1.1 使用 Yellowstone gRPC 订阅事件

```rust
use sol_trade_sdk::{
    event_parser::{
        protocols::{
            pumpfun::{PumpFunCreateTokenEvent, PumpFunTradeEvent},
            pumpswap::{
                PumpSwapBuyEvent, PumpSwapCreatePoolEvent, PumpSwapDepositEvent,
                PumpSwapSellEvent, PumpSwapWithdrawEvent,
            },
            bonk::{BonkPoolCreateEvent, BonkTradeEvent},
        },
        Protocol, UnifiedEvent,
    },
    grpc::YellowstoneGrpc,
    match_event,
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
        });
    };

    // 订阅多个协议的事件
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
```

#### 1.2 使用 ShredStream 订阅事件

```rust
use sol_trade_sdk::grpc::ShredStreamGrpc;

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
        });
    };

    // 订阅事件
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
```

### 2. 初始化 SolanaTrade 实例

```rust
use std::{str::FromStr, sync::Arc};
use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig},
    swqos::{SwqosConfig, SwqosRegion},
    SolanaTrade
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

// 创建交易者账户
let payer = Keypair::new();
let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();

// 配置多个MEV服务，支持并发交易
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

// 创建SolanaTrade实例
let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
```

### 3. PumpFun 交易操作

```rust
use sol_trade_sdk::{
    accounts::BondingCurveAccount,
    constants::{pumpfun::global_constants::TOKEN_TOTAL_SUPPLY, trade_type},
    pumpfun::common::get_bonding_curve_account_v2,
    trading::{
        core::params::{PumpFunParams, PumpFunSellParams},
        BuyParams, SellParams,
    },
};

async fn test_pumpfun() -> AnyResult<()> {
    // 基本参数设置
    let creator = Pubkey::from_str("xxxxxx")?; // 开发者账户
    let mint_pubkey = Pubkey::from_str("xxxxxx")?; // 代币地址
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();

    println!("Buying tokens from PumpFun...");

    // 获取bonding curve信息
    let (bonding_curve, bonding_curve_pda) =
        get_bonding_curve_account_v2(&solana_trade_client.rpc, &mint_pubkey).await?;

    let bonding_curve = BondingCurveAccount {
        discriminator: bonding_curve.discriminator,
        account: bonding_curve_pda,
        virtual_token_reserves: bonding_curve.virtual_token_reserves,
        virtual_sol_reserves: bonding_curve.virtual_sol_reserves,
        real_token_reserves: bonding_curve.real_token_reserves,
        real_sol_reserves: bonding_curve.real_sol_reserves,
        token_total_supply: TOKEN_TOTAL_SUPPLY,
        complete: false,
        creator: creator,
    };

    // 购买操作
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

    // 使用MEV保护的购买
    let buy_with_tip_params = buy_params
        .clone()
        .with_tip(solana_trade_client.swqos_clients.clone());

    solana_trade_client
        .buy_use_buy_params(buy_with_tip_params, None)
        .await?;

    // 卖出操作
    println!("Selling tokens from PumpFun...");
    let sell_protocol_params = PumpFunSellParams {};
    let amount_token = 1000000; // 写上真实的代币数量

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
```

### 4. PumpSwap 交易操作

```rust
use sol_trade_sdk::trading::core::params::PumpSwapParams;

async fn test_pumpswap() -> AnyResult<()> {
    // 基本参数设置
    let creator = Pubkey::from_str("11111111111111111111111111111111")?; // 开发者账户
    let mint_pubkey = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?; // 代币地址
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();

    println!("Buying tokens from PumpSwap...");

    // PumpSwap参数配置
    let protocol_params = PumpSwapParams {
        pool: None,
        pool_base_token_account: None,
        pool_quote_token_account: None,
        user_base_token_account: None,
        user_quote_token_account: None,
        auto_handle_wsol: true,
    };

    // 购买操作
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

    // 卖出操作
    println!("Selling tokens from PumpSwap...");
    let amount_token = 1000000; // 写上真实的代币数量

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
```

### 5. Bonk 交易操作

```rust
use sol_trade_sdk::trading::core::params::BonkParams;

async fn test_bonk() -> Result<(), Box<dyn std::error::Error>> {
    // 基本参数设置
    let amount = 100_000; // 0.0001 SOL
    let mint = Pubkey::from_str("xxxxxxx")?;
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;

    // Bonk参数配置
    let bonk_params = BonkParams {
        virtual_base: None,
        virtual_quote: None,
        real_base_before: None,
        real_quote_before: None,
        auto_handle_wsol: true,
    };

    println!("Buying tokens from letsbonk.fun...");

    // 购买操作
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

    // 卖出操作
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
```

### 6. 自定义优先费用配置

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

## MEV 保护服务

- **Jito**: 高性能区块空间
- **NextBlock**: 快速交易执行
- **ZeroSlot**: 零延迟交易
- **Temporal**: 时间敏感交易
- **Bloxroute**: 区块链网络加速

## 新架构特性

### 统一参数结构

- **BuyParams**: 统一的购买参数结构
- **SellParams**: 统一的卖出参数结构
- **协议特定参数**: 每个协议都有自己的参数结构（PumpFunParams、PumpSwapParams、BonkParams）

### 事件解析系统

- **统一事件接口**: 所有协议事件都实现 UnifiedEvent 特征
- **协议特定事件**: 每个协议都有自己的事件类型
- **事件工厂**: 自动识别和解析不同协议的事件

### 交易引擎

- **统一交易接口**: 所有交易操作都使用相同的方法
- **协议抽象**: 支持多个协议的交易操作
- **并发执行**: 支持同时向多个 MEV 服务发送交易

## 项目结构

```
src/
├── accounts/         # 账户相关定义
├── common/           # 通用功能和工具
├── constants/        # 常量定义
├── error/            # 错误处理
├── event_parser/     # 事件解析系统
│   ├── common/       # 通用事件解析工具
│   ├── core/         # 核心解析特征和接口
│   ├── protocols/    # 协议特定解析器
│   │   ├── pumpfun/  # PumpFun事件解析
│   │   ├── pumpswap/ # PumpSwap事件解析
│   │   └── bonk/     # Bonk事件解析
│   └── factory.rs    # 解析器工厂
├── grpc/             # gRPC客户端
├── instruction/      # 指令构建
├── protos/           # 协议缓冲区定义
├── pumpfun/          # PumpFun交易功能
├── pumpswap/         # PumpSwap交易功能
├── bonk/             # Bonk交易功能
├── swqos/            # MEV服务客户端
├── trading/          # 统一交易引擎
│   ├── common/       # 通用交易工具
│   ├── core/         # 核心交易引擎
│   └── protocols/    # 协议特定交易实现
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
