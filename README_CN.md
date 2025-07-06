# Sol Trade SDK

一个全面的Rust SDK，用于与Solana DEX交易程序进行无缝交互。此SDK提供强大的工具和接口集，将PumpFun和PumpSwap功能集成到您的应用程序中。

## 项目特性

1. **PumpFun交易**: 支持`购买`、`卖出`功能
2. **PumpSwap交易**: 支持PumpSwap池的交易操作
3. **Raydium交易**: 支持Raydium DEX的交易操作
4. **日志订阅**: 订阅PumpFun、PumpSwap和Raydium程序的交易日志
5. **Yellowstone gRPC**: 使用Yellowstone gRPC订阅程序日志
6. **ShredStream支持**: 使用ShredStream订阅程序日志
7. **多种MEV保护**: 支持Jito、Nextblock、0slot、Nozomi等服务
8. **并发交易**: 同时使用多个MEV服务发送交易，最快的成功，其他失败
9. **实时价格**: 获取代币实时价格和流动性信息

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

### 1. 日志订阅 - 监听代币交易

```rust
use sol_trade_sdk::{common::pumpfun::logs_events::PumpfunEvent, grpc::YellowstoneGrpc};
use solana_sdk::signature::Keypair;

// 创建Yellowstone gRPC客户端
let grpc_url = "https://solana-yellowstone-grpc.publicnode.com:443";
let x_token = None; // 可选的认证令牌
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// 定义回调函数
let callback = |event: PumpfunEvent| match event {
    PumpfunEvent::NewDevTrade(trade_info) => {
        println!("收到开发者交易事件: {:?}", trade_info);
    }
    PumpfunEvent::NewToken(token_info) => {
        println!("收到新代币事件: {:?}", token_info);
    }
    PumpfunEvent::NewUserTrade(trade_info) => {
        println!("收到用户交易事件: {:?}", trade_info);
    }
    PumpfunEvent::NewBotTrade(trade_info) => {
        println!("收到机器人交易事件: {:?}", trade_info);
    }
    PumpfunEvent::Error(err) => {
        println!("收到错误: {}", err);
    }
};

client.subscribe_pumpfun(callback, None).await?;
```

### 2. 初始化SolanaTrade实例

```rust
use std::{str::FromStr, sync::Arc};

use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig}, 
    swqos::{SwqosConfig, SwqosRegion, SwqosType}, 
    SolanaTrade
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

// 配置优先费用
let priority_fee = PriorityFee {
    unit_limit: 190000,
    unit_price: 1000000,
    rpc_unit_limit: 500000,
    rpc_unit_price: 500000,
    buy_tip_fee: 0.001,
    buy_tip_fees: vec![0.001, 0.002],
    sell_tip_fee: 0.0001,
};

// 单区域配置多个swqos，可同时发送交易
let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();
let swqos_configs = vec![
    SwqosConfig::Jito(SwqosRegion::Frankfurt),
    SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Default(rpc_url.clone()),
];

// 定义sdk配置参数
let trade_config = TradeConfig {
    rpc_url: rpc_url.clone(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee,
    swqos_configs,
    lookup_table_key: None,
};

// 创建SolanaTrade实例
let payer = Keypair::from_base58_string("your_private_key");
let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
```

### 3. 购买代币

### 3.1 购买代币 --- 狙击
```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};
use std::sync::Arc;
use sol_trade_sdk::accounts::BondingCurveAccount;

let mint_pubkey = Pubkey::from_str("代币地址")?;
let creator = Pubkey::from_str("创建者地址")?;
let recent_blockhash = Hash::default(); // 获取最新区块哈希
let buy_sol_cost = 50000; // 0.00005 SOL
let slippage_basis_points = Some(100); // 1%

// 狙击购买（新代币上线时快速购买）
let dev_buy_token = 100_000; // 测试值
let dev_cost_sol = 10_000; // 测试值
let bonding_curve = BondingCurveAccount::new(&mint_pubkey, dev_buy_token, dev_cost_sol, creator);

solana_trade_client.sniper_buy(
    mint_pubkey,
    creator,
    buy_sol_cost,
    slippage_basis_points,
    recent_blockhash,
    Some(Arc::new(bonding_curve)),
).await?;

// 使用小费进行MEV保护的购买
solana_trade_client.sniper_buy_with_tip(
    mint_pubkey,
    creator,
    buy_sol_cost,
    slippage_basis_points,
    recent_blockhash,
    Some(Arc::new(bonding_curve)),
    None, // 自定义小费
).await?;
```

### 3.2 购买代币 --- 跟单
```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};
use std::sync::Arc;
use sol_trade_sdk::accounts::BondingCurveAccount;
use sol_trade_sdk::{constants::{pumpfun::global_constants::TOKEN_TOTAL_SUPPLY, trade_type::COPY_BUY}, pumpfun::common::get_bonding_curve_pda};

let mint_pubkey = Pubkey::from_str("代币地址")?;
let creator = Pubkey::from_str("创建者地址")?;
let recent_blockhash = Hash::default(); // 获取最新区块哈希
let buy_sol_cost = 50000; // 0.00005 SOL
let slippage_basis_points = Some(100); // 1%

// 跟单购买
let dev_buy_token = 100_000; // 测试值
let dev_cost_sol = 10_000; // 测试值
// trade_info来自pumpfun解析出来的数据，可以参考上面 1. 日志订阅
let bonding_curve = Some(Arc::new(BondingCurveAccount {
    discriminator: 0,
    account: get_bonding_curve_pda(&trade_info.mint).unwrap(),
    virtual_token_reserves: trade_info.virtual_token_reserves,
    virtual_sol_reserves: trade_info.virtual_sol_reserves,
    real_token_reserves: trade_info.real_token_reserves,
    real_sol_reserves: trade_info.real_sol_reserves,
    token_total_supply: TOKEN_TOTAL_SUPPLY,
    complete: false,
    creator: Pubkey::from_str(&creator).unwrap(),
}));

solana_trade_client.buy(
    mint_pubkey,
    creator,
    buy_sol_cost,
    slippage_basis_points,
    recent_blockhash,
    Some(Arc::new(bonding_curve)),
    "pumpfun".to_string(), 
).await?;

// 使用小费进行MEV保护的购买
solana_trade_client.buy_with_tip(
    mint_pubkey,
    creator,
    buy_sol_cost,
    slippage_basis_points,
    recent_blockhash,
    Some(Arc::new(bonding_curve)),
    "pumpfun".to_string(), 
    None, // 自定义小费
).await?;
```

### 4. 卖出代币

```rust
// 按数量卖出
solana_trade_client.sell_by_amount_with_tip(
    mint_pubkey,
    creator,
    1000000, // 代币数量
    recent_blockhash,
    "pumpfun".to_string(), // 交易平台
).await?;

// 按百分比卖出
solana_trade_client.sell_by_percent_with_tip(
    mint_pubkey,
    creator,
    50,      // 百分比 (50%)
    2000000, // 总代币数量
    recent_blockhash,
    "pumpfun".to_string(), // 交易平台
).await?;
```

### 5. 获取价格和余额信息

```rust
// 获取代币当前价格
let price = solana_trade_client.get_current_price(&mint_pubkey).await?;
println!("当前价格: {}", price);

// 获取SOL余额
let sol_balance = solana_trade_client.get_payer_sol_balance().await?;
println!("SOL余额: {} lamports", sol_balance);

// 获取代币余额
let token_balance = solana_trade_client.get_payer_token_balance(&mint_pubkey).await?;
println!("代币余额: {}", token_balance);

// 获取流动性信息
let sol_reserves = solana_trade_client.get_real_sol_reserves(&mint_pubkey).await?;
println!("SOL储备: {} lamports", sol_reserves);
```

### 6. PumpSwap订阅 - 监听AMM事件

```rust
use sol_trade_sdk::{common::pumpswap::logs_events::PumpSwapEvent, grpc::YellowstoneGrpc};

// 创建Yellowstone gRPC客户端
let grpc_url = "https://solana-yellowstone-grpc.publicnode.com:443";
let x_token = None;
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// 定义PumpSwap事件的回调函数
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

// 订阅PumpSwap事件
println!("开始监听PumpSwap事件，按Ctrl+C停止...");
client.subscribe_pumpswap(callback).await?;
```

### 7. PumpSwap交易操作

```rust
use std::sync::Arc;
use solana_sdk::{pubkey::Pubkey, hash::Hash, signature::Keypair};
use solana_client::rpc_client::RpcClient;
use sol_trade_sdk::{common::{Cluster, PriorityFee}, SolanaTrade};

// 单区域配置多个swqos，可同时发送交易
let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();
let swqos_configs = vec![
    SwqosConfig::Jito(SwqosRegion::Frankfurt),
    SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Default(rpc_url.clone()),
];

// 定义sdk配置参数
let trade_config = TradeConfig {
    rpc_url: rpc_url.clone(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee,
    swqos_configs,
    lookup_table_key: None,
};

// 创建SolanaTrade实例
let payer = Keypair::from_base58_string("your_private_key");
let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;

let creator = Pubkey::from_str("11111111111111111111111111111111")?; // 开发者账户
let buy_sol_cost = 500_000; // 0.0005 SOL
let slippage_basis_points = Some(100);
let rpc = RpcClient::new(cluster.rpc_url);
let recent_blockhash = rpc.get_latest_blockhash().unwrap();
let trade_platform = "pumpswap".to_string();
let mint_pubkey = Pubkey::from_str("您的代币铸造地址")?; // 代币铸造地址

println!("从PumpSwap购买代币...");
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

// 卖出30%的代币数量
solana_trade_client
    .sell_by_percent(
        mint_pubkey,
        creator,
        30,      // 百分比 (30%)
        100,     // 总代币数量
        recent_blockhash,
        trade_platform.clone(),
    )
    .await?;
```

### 8. PumpSwap池信息

```rust
use solana_sdk::pubkey::Pubkey;

let pool_address = Pubkey::from_str("池地址")?;

// 从PumpSwap池获取当前价格
let price = solana_trade_client.get_current_price_with_pumpswap(&pool_address).await?;
println!("PumpSwap池价格: {}", price);

// 获取PumpSwap池中的SOL储备
let sol_reserves = solana_trade_client.get_real_sol_reserves_with_pumpswap(&pool_address).await?;
println!("PumpSwap SOL储备: {} lamports", sol_reserves);

// 获取PumpSwap池中的代币余额
let token_balance = solana_trade_client.get_payer_token_balance_with_pumpswap(&pool_address).await?;
println!("PumpSwap代币余额: {}", token_balance);
```

## 支持的交易平台

- **PumpFun**: 主要的meme币交易平台
- **PumpSwap**: PumpFun的交换协议
- **Raydium**: 集成Raydium DEX功能

## MEV保护服务

- **Jito**: 高性能区块空间
- **Nextblock**: 快速交易执行
- **0slot**: 零延迟交易
- **Nozomi**: MEV保护服务

## 项目结构

```
src/
├── accounts/     # 账户相关定义
├── common/       # 通用功能和工具
├── constants/    # 常量定义
├── error/        # 错误处理
├── grpc/         # gRPC客户端
├── instruction/  # 指令构建
├── pumpfun/      # PumpFun交易功能
├── pumpswap/     # PumpSwap交易功能
├── swqos/        # MEV服务客户端
├── trading/      # 统一交易引擎
├── lib.rs        # 主库文件
└── main.rs       # 示例程序
```

## 许可证

MIT许可证

## 联系方式

- 项目仓库: https://github.com/0xfnzero/sol-trade-sdk
- Telegram群组: https://t.me/fnzero_group

## 重要注意事项

1. 在主网使用前请充分测试
2. 正确设置私钥和API令牌
3. 注意滑点设置避免交易失败
4. 监控余额和交易费用
5. 遵循相关法律法规

## 语言版本

- [English](README.md)
- [中文](README_CN.md) 