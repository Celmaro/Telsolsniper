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
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.5.2" }
```

### 使用 crates.io

```toml
# 添加到您的 Cargo.toml
sol-trade-sdk = "0.5.2"
```

## 使用示例

### 重要说明

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

#### 关于shredstream

当你使用 shred 订阅事件时，由于 shred 的特性，你无法获取到交易事件的完整信息。
请你在使用时，确保你的交易逻辑依赖的参数，在shred中都能获取到。

### 1. 事件订阅 - 监听代币交易

查看[examples/event_subscription](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/event_subscription/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package event_subscription
```

### 2. 初始化 SolanaTrade 实例

#### 2.1 SWQOS 服务配置说明

在配置 SWQOS 服务时，需要注意不同服务的参数要求：

- **Jito**: 第一个参数是 UUID，如果没有 UUID 则传空字符串 `""`
- **NextBlock**: 第一个参数是 API Token
- **Bloxroute**: 第一个参数是 API Token  
- **ZeroSlot**: 第一个参数是 API Token
- **Temporal**: 第一个参数是 API Token
- **FlashBlock**: 第一个参数是 API Token, 添加tg官方客服https://t.me/FlashBlock_Official 获取免费key立即加速你的交易！官方文档: https://doc.flashblock.trade/
- **Node1**: 第一个参数是 API Token, 添加tg官方客服https://t.me/node1_me 获取免费key立即加速你的交易！官方文档: https://node1.me/docs.html

当使用多个MEV服务时，需要使用`Durable Nonce`。你需要初始化`NonceCache`类（或者自行写一个管理nonce的类），获取最新的`nonce`值，并在交易的时候作为`blockhash`使用。

#### 2.2 创建 SolanaTrade 实例

查看[examples/trading_client](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/trading_client/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package trading_client
```

### 3. PumpFun 交易操作

#### 3.1 狙击

查看[examples/pumpfun_sniper_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/pumpfun_sniper_trading/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package pumpfun_sniper_trading
```

#### 3.2 跟单

查看[examples/pumpfun_copy_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/pumpfun_copy_trading/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package pumpfun_copy_trading
```

### 4. PumpSwap 交易操作

查看[examples/pumpswap_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/pumpswap_trading/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package pumpswap_trading
```

### 5. Raydium CPMM 交易操作

查看[examples/raydium_cpmm_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/raydium_cpmm_trading/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package raydium_cpmm_trading
```


### 6. Raydium AMM V4 交易操作

查看[examples/raydium_amm_v4_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/raydium_amm_v4_trading/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package raydium_amm_v4_trading
```

### 7. Bonk 交易操作

#### 7.1 狙击

查看[examples/bonk_sniper_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/bonk_sniper_trading/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package bonk_sniper_trading
```

#### 7.2 跟单

查看[examples/bonk_copy_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/bonk_copy_trading/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package bonk_copy_trading
```

### 8. 中间件系统

SDK 提供了强大的中间件系统，允许您在交易执行前对指令进行修改、添加或移除。这为您提供了极大的灵活性来自定义交易行为。

查看[examples/middleware_system](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/middleware_system/src/main.rs) 中的示例代码。

运行示例代码：
```bash
cargo run --package middleware_system
```

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
    tip_unit_limit: 190000,
    tip_unit_price: 1000000,
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
