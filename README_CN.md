# Sol Trade SDK

一个全面的Rust SDK，用于与Solana DEX交易程序进行无缝交互。此SDK提供强大的工具和接口集，将PumpFun和PumpSwap功能集成到您的应用程序中。

## 项目特性

1. **PumpFun交易**: 支持`创建代币`、`购买`、`卖出`功能
2. **PumpSwap交易**: 支持PumpSwap池的交易操作
3. **日志订阅**: 订阅PumpFun程序的交易日志
4. **Yellowstone gRPC**: 使用gRPC订阅程序日志
5. **多种MEV保护**: 支持Jito、Nextblock、0slot、Nozomi等服务
6. **并发交易**: 同时使用多个MEV服务发送交易，最快的成功，其他失败
7. **IPFS集成**: 支持代币元数据的IPFS上传
8. **实时价格**: 获取代币实时价格和流动性信息

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

### 1. 日志订阅 - 监听代币创建和交易

```rust
use sol_trade_sdk::{common::pumpfun::logs_events::PumpfunEvent, grpc::YellowstoneGrpc};
use solana_sdk::signature::Keypair;

// 创建gRPC客户端
let grpc_url = "http://127.0.0.1:10000";
let x_token = None; // 可选的认证令牌
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// 定义回调函数
let callback = |event: PumpfunEvent| {
    match event {
        PumpfunEvent::NewToken(token_info) => {
            println!("收到新代币事件: {:?}", token_info);
        },
        PumpfunEvent::NewDevTrade(trade_info) => {
            println!("收到开发者交易事件: {:?}", trade_info);
        },
        PumpfunEvent::NewUserTrade(trade_info) => {
            println!("收到用户交易事件: {:?}", trade_info);
        },
        PumpfunEvent::NewBotTrade(trade_info) => {
            println!("收到机器人交易事件: {:?}", trade_info);
        }
        PumpfunEvent::Error(err) => {
            println!("收到错误: {}", err);
        }
    }
};

let payer_keypair = Keypair::from_base58_string("your_private_key");
client.subscribe_pumpfun(callback, Some(payer_keypair.pubkey())).await?;
```

### 2. 初始化PumpFun实例

```rust
use std::sync::Arc;
use sol_trade_sdk::{common::{Cluster, PriorityFee}, PumpFun};
use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair};

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

// 配置集群
let cluster = Cluster {
    rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
    block_engine_url: "https://block-engine.example.com".to_string(),
    nextblock_url: "https://nextblock.example.com".to_string(),
    nextblock_auth_token: "nextblock_api_token".to_string(),
    zeroslot_url: "https://zeroslot.example.com".to_string(),
    zeroslot_auth_token: "zeroslot_api_token".to_string(),
    nozomi_url: "https://nozomi.example.com".to_string(),
    nozomi_auth_token: "nozomi_api_token".to_string(),
    use_jito: true,
    use_nextblock: false,
    use_zeroslot: false,
    use_nozomi: false,
    use_rpc: true,
    priority_fee,
    commitment: CommitmentConfig::processed(),
    lookup_table_key: None, // 可选的查找表
};

// 创建PumpFun实例
let payer = Keypair::from_base58_string("your_private_key");
let pumpfun = PumpFun::new(Arc::new(payer), &cluster).await;
```

### 3. 创建代币

```rust
use sol_trade_sdk::{PumpFun, ipfs::CreateTokenMetadata, ipfs::create_token_metadata};
use solana_sdk::signature::Keypair;

// 创建代币密钥对
let mint_keypair = Keypair::new();

// 准备代币元数据
let metadata = CreateTokenMetadata {
    name: "我的代币".to_string(),
    symbol: "MTK".to_string(),
    description: "这是一个测试代币".to_string(),
    file: "path/to/image.png".to_string(), // 本地文件路径
    twitter: Some("https://twitter.com/example".to_string()),
    telegram: Some("https://t.me/example".to_string()),
    website: Some("https://example.com".to_string()),
    metadata_uri: None, // 将自动生成
};

// 上传元数据到IPFS
let api_token = "your_pinata_api_token";
let ipfs_response = create_token_metadata(metadata, api_token).await?;

// 创建代币
pumpfun.create(mint_keypair, ipfs_response).await?;
```

### 4. 购买代币

```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};

let mint_pubkey = Pubkey::from_str("代币地址")?;
let creator = Pubkey::from_str("创建者地址")?;
let recent_blockhash = Hash::default(); // 获取最新区块哈希

// 狙击购买（新代币上线时快速购买）
pumpfun.sniper_buy_with_tip(
    mint_pubkey,
    creator,
    1000000,  // dev_buy_token
    10000,    // dev_sol_cost  
    50000,    // buy_sol_cost (lamports)
    Some(100), // slippage (1%)
    recent_blockhash,
).await?;

// 复制购买（跟随其他交易者）
pumpfun.copy_buy_with_tip(
    mint_pubkey,
    creator,
    1000000,  // dev_buy_token
    10000,    // dev_sol_cost
    50000,    // buy_sol_cost (lamports)
    Some(100), // slippage (1%)
    recent_blockhash,
    "pumpfun".to_string(), // 交易平台
).await?;
```

### 5. 卖出代币

```rust
// 按数量卖出
pumpfun.sell_by_amount_with_tip(
    mint_pubkey,
    creator,
    1000000, // 代币数量
    recent_blockhash,
    "pumpfun".to_string(),
).await?;

// 按百分比卖出
pumpfun.sell_by_percent_with_tip(
    mint_pubkey,
    creator,
    50,      // 百分比 (50%)
    2000000, // 总代币数量
    recent_blockhash,
    "pumpfun".to_string(),
).await?;
```

### 6. 获取价格和余额信息

```rust
// 获取代币当前价格
let price = pumpfun.get_current_price(&mint_pubkey).await?;
println!("当前价格: {}", price);

// 获取SOL余额
let sol_balance = pumpfun.get_payer_sol_balance().await?;
println!("SOL余额: {} lamports", sol_balance);

// 获取代币余额
let token_balance = pumpfun.get_payer_token_balance(&mint_pubkey).await?;
println!("代币余额: {}", token_balance);

// 获取流动性信息
let sol_reserves = pumpfun.get_real_sol_reserves(&mint_pubkey).await?;
println!("SOL储备: {} lamports", sol_reserves);
```

### 7. PumpSwap订阅 - 监听AMM事件

```rust
use sol_trade_sdk::{common::pumpswap::logs_events::PumpSwapEvent, grpc::YellowstoneGrpc};

// 创建gRPC客户端（与上面相同）
let grpc_url = "http://127.0.0.1:10000";
let x_token = None;
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// 定义PumpSwap事件的回调函数
let callback = |event: PumpSwapEvent| {
    match event {
        PumpSwapEvent::Buy(buy_event) => {
            println!("PumpSwap购买事件: {:?}", buy_event);
        },
        PumpSwapEvent::Sell(sell_event) => {
            println!("PumpSwap卖出事件: {:?}", sell_event);
        },
        PumpSwapEvent::CreatePool(pool_event) => {
            println!("PumpSwap池创建: {:?}", pool_event);
        },
        PumpSwapEvent::Deposit(deposit_event) => {
            println!("PumpSwap存款: {:?}", deposit_event);
        },
        PumpSwapEvent::Withdraw(withdraw_event) => {
            println!("PumpSwap提款: {:?}", withdraw_event);
        },
        PumpSwapEvent::Disable(disable_event) => {
            println!("PumpSwap池禁用: {:?}", disable_event);
        },
        PumpSwapEvent::UpdateAdmin(admin_event) => {
            println!("PumpSwap管理员更新: {:?}", admin_event);
        },
        PumpSwapEvent::UpdateFeeConfig(fee_event) => {
            println!("PumpSwap费用配置更新: {:?}", fee_event);
        },
        PumpSwapEvent::Error(err) => {
            println!("PumpSwap错误: {}", err);
        }
    }
};

// 订阅PumpSwap事件
client.subscribe_pumpswap(callback).await?;
```

### 8. PumpSwap交易操作

```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};

let mint_pubkey = Pubkey::from_str("代币地址")?;
let creator = Pubkey::from_str("创建者地址")?;
let recent_blockhash = Hash::default();

// 在PumpSwap上购买代币
pumpfun.copy_buy_with_tip(
    mint_pubkey,
    creator,
    1000000,  // dev_buy_token
    10000,    // dev_sol_cost
    50000,    // buy_sol_cost (lamports)
    Some(100), // slippage (1%)
    recent_blockhash,
    "pumpswap".to_string(), // 使用PumpSwap平台
).await?;

// 在PumpSwap上按数量卖出代币
pumpfun.sell_by_amount_with_tip(
    mint_pubkey,
    creator,
    1000000, // 代币数量
    recent_blockhash,
    "pumpswap".to_string(), // 使用PumpSwap平台
).await?;

// 在PumpSwap上按百分比卖出代币
pumpfun.sell_by_percent_with_tip(
    mint_pubkey,
    creator,
    50,      // 百分比 (50%)
    2000000, // 总代币数量
    recent_blockhash,
    "pumpswap".to_string(), // 使用PumpSwap平台
).await?;
```

### 9. PumpSwap池信息

```rust
use solana_sdk::pubkey::Pubkey;

let pool_address = Pubkey::from_str("池地址")?;

// 从PumpSwap池获取当前价格
let price = pumpfun.get_current_price_with_pumpswap(&pool_address).await?;
println!("PumpSwap池价格: {}", price);

// 获取PumpSwap池中的SOL储备
let sol_reserves = pumpfun.get_real_sol_reserves_with_pumpswap(&pool_address).await?;
println!("PumpSwap SOL储备: {} lamports", sol_reserves);

// 获取PumpSwap池中的代币余额
let token_balance = pumpfun.get_payer_token_balance_with_pumpswap(&pool_address).await?;
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
├── ipfs/         # IPFS集成
├── pumpfun/      # PumpFun交易功能
├── pumpswap/     # PumpSwap交易功能
├── swqos/        # MEV服务客户端
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