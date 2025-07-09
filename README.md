# Sol Trade SDK

A comprehensive Rust SDK for seamless interaction with Solana DEX trading programs. This SDK provides a robust set of tools and interfaces to integrate PumpFun, PumpSwap, and Bonk functionality into your applications.

## Project Features

1. **PumpFun Trading**: Support for `buy` and `sell` operations
2. **PumpSwap Trading**: Support for PumpSwap pool trading operations
3. **Bonk Trading**: Support for Bonk trading operations
4. **Event Subscription**: Subscribe to PumpFun, PumpSwap, and Bonk program trading events
5. **Yellowstone gRPC**: Subscribe to program events using Yellowstone gRPC
6. **ShredStream Support**: Subscribe to program events using ShredStream
7. **Multiple MEV Protection**: Support for Jito, Nextblock, ZeroSlot, Temporal, Bloxroute, and other services
8. **Concurrent Trading**: Send transactions using multiple MEV services simultaneously; the fastest succeeds while others fail
9. **Unified Trading Interface**: Use unified parameter structures for trading operations

## Installation

Clone this project to your project directory:

```bash
cd your_project_root_directory
git clone https://github.com/0xfnzero/sol-trade-sdk
```

Add the dependency to your `Cargo.toml`:

```toml
# Add to your Cargo.toml
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.1.0" }
```

## Usage Examples

### 1. Event Subscription - Monitor Token Trading

#### 1.1 Subscribe to Events Using Yellowstone gRPC

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
    // Subscribe to events using GRPC client
    println!("Subscribing to GRPC events...");

    let grpc = YellowstoneGrpc::new(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
    )?;

    // Define callback function to handle events
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

    // Subscribe to events from multiple protocols
    println!("Starting to listen for events, press Ctrl+C to stop...");
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

#### 1.2 Subscribe to Events Using ShredStream

```rust
use sol_trade_sdk::grpc::ShredStreamGrpc;

async fn test_shreds() -> Result<(), Box<dyn std::error::Error>> {
    // Subscribe to events using ShredStream client
    println!("Subscribing to ShredStream events...");

    let shred_stream = ShredStreamGrpc::new("http://127.0.0.1:10800".to_string()).await?;

    // Define callback function to handle events (same as above)
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

    // Subscribe to events
    println!("Starting to listen for events, press Ctrl+C to stop...");
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

### 2. Initialize SolanaTrade Instance

```rust
use std::{str::FromStr, sync::Arc};
use sol_trade_sdk::{
    common::{AnyResult, PriorityFee, TradeConfig},
    swqos::{SwqosConfig, SwqosRegion},
    SolanaTrade
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

// Create trader account
let payer = Keypair::new();
let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();

// Configure multiple MEV services to support concurrent trading
let swqos_configs = vec![
    SwqosConfig::Jito(SwqosRegion::Frankfurt),
    SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
    SwqosConfig::Default(rpc_url.clone()),
];

// Define trading configuration
let trade_config = TradeConfig {
    rpc_url: rpc_url.clone(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee: PriorityFee::default(),
    swqos_configs,
    lookup_table_key: None,
};

// Create SolanaTrade instance
let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
```

### 3. PumpFun Trading Operations

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
    // Basic parameter setup
    let creator = Pubkey::from_str("xxxxxx")?; // Developer account
    let mint_pubkey = Pubkey::from_str("xxxxxx")?; // Token address
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();

    println!("Buying tokens from PumpFun...");

    // Get bonding curve information
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

    // Buy operation
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

    // Buy with MEV protection
    let buy_with_tip_params = buy_params
        .clone()
        .with_tip(solana_trade_client.swqos_clients.clone());

    solana_trade_client
        .buy_use_buy_params(buy_with_tip_params, None)
        .await?;

    // Sell operation
    println!("Selling tokens from PumpFun...");
    let sell_protocol_params = PumpFunSellParams {};
    let amount_token = 1000000; // Enter the actual token amount

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

### 4. PumpSwap Trading Operations

```rust
use sol_trade_sdk::trading::core::params::PumpSwapParams;

async fn test_pumpswap() -> AnyResult<()> {
    // Basic parameter setup
    let creator = Pubkey::from_str("11111111111111111111111111111111")?; // Developer account
    let mint_pubkey = Pubkey::from_str("2zMMhcVQEXDtdE6vsFS7S7D5oUodfJHE8vd1gnBouauv")?; // Token address
    let buy_sol_cost = 100_000; // 0.0001 SOL
    let slippage_basis_points = Some(100);
    let rpc = RpcClient::new(rpc_url);
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();

    println!("Buying tokens from PumpSwap...");

    // PumpSwap parameter configuration
    let protocol_params = PumpSwapParams {
        pool: None,
        pool_base_token_account: None,
        pool_quote_token_account: None,
        user_base_token_account: None,
        user_quote_token_account: None,
        auto_handle_wsol: true,
    };

    // Buy operation
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

    // Sell operation
    println!("Selling tokens from PumpSwap...");
    let amount_token = 1000000; // Enter the actual token amount

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

### 5. Bonk Trading Operations

```rust
use sol_trade_sdk::trading::core::params::BonkParams;

async fn test_bonk() -> Result<(), Box<dyn std::error::Error>> {
    // Basic parameter setup
    let amount = 100_000; // 0.0001 SOL
    let mint = Pubkey::from_str("xxxxxxx")?;
    let recent_blockhash = solana_trade_client.rpc.get_latest_blockhash().await?;

    // Bonk parameter configuration
    let bonk_params = BonkParams {
        virtual_base: None,
        virtual_quote: None,
        real_base_before: None,
        real_quote_before: None,
        auto_handle_wsol: true,
    };

    println!("Buying tokens from letsbonk.fun...");

    // Buy operation
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

    // Sell operation
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

### 6. Custom Priority Fee Configuration

```rust
use sol_trade_sdk::common::PriorityFee;

// Custom priority fee configuration
let priority_fee = PriorityFee {
    unit_limit: 190000,
    unit_price: 1000000,
    rpc_unit_limit: 500000,
    rpc_unit_price: 500000,
    buy_tip_fee: 0.001,
    buy_tip_fees: vec![0.001, 0.002],
    sell_tip_fee: 0.0001,
};

// Use custom priority fee in TradeConfig
let trade_config = TradeConfig {
    rpc_url: rpc_url.clone(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee, // Use custom priority fee
    swqos_configs,
    lookup_table_key: None,
};
```

## Supported Trading Platforms

- **PumpFun**: Primary meme coin trading platform
- **PumpSwap**: PumpFun's swap protocol
- **Bonk**: token launch platform (letsbonk.fun)

## MEV Protection Services

- **Jito**: High-performance block space
- **NextBlock**: Fast transaction execution
- **ZeroSlot**: Zero-latency transactions
- **Temporal**: Time-sensitive transactions
- **Bloxroute**: Blockchain network acceleration

## New Architecture Features

### Unified Parameter Structure

- **BuyParams**: Unified buy parameter structure
- **SellParams**: Unified sell parameter structure
- **Protocol-specific Parameters**: Each protocol has its own parameter structure (PumpFunParams, PumpSwapParams, BonkParams)

### Event Parsing System

- **Unified Event Interface**: All protocol events implement the UnifiedEvent trait
- **Protocol-specific Events**: Each protocol has its own event types
- **Event Factory**: Automatically identifies and parses events from different protocols

### Trading Engine

- **Unified Trading Interface**: All trading operations use the same methods
- **Protocol Abstraction**: Supports trading operations across multiple protocols
- **Concurrent Execution**: Supports sending transactions to multiple MEV services simultaneously

## Project Structure

```
src/
├── accounts/         # Account-related definitions
├── common/           # Common functionality and tools
├── constants/        # Constant definitions
├── error/            # Error handling
├── event_parser/     # Event parsing system
│   ├── common/       # Common event parsing tools
│   ├── core/         # Core parsing traits and interfaces
│   ├── protocols/    # Protocol-specific parsers
│   │   ├── pumpfun/  # PumpFun event parsing
│   │   ├── pumpswap/ # PumpSwap event parsing
│   │   └── bonk/     # Bonk event parsing
│   └── factory.rs    # Parser factory
├── grpc/             # gRPC clients
├── instruction/      # Instruction building
├── protos/           # Protocol buffer definitions
├── pumpfun/          # PumpFun trading functionality
├── pumpswap/         # PumpSwap trading functionality
├── bonk/             # Bonk trading functionality
├── swqos/            # MEV service clients
├── trading/          # Unified trading engine
│   ├── common/       # Common trading tools
│   ├── core/         # Core trading engine
│   └── protocols/    # Protocol-specific trading implementations
├── lib.rs            # Main library file
└── main.rs           # Example program
```

## License

MIT License

## Contact

- Project Repository: https://github.com/0xfnzero/sol-trade-sdk
- Telegram Group: https://t.me/fnzero_group

## Important Notes

1. Test thoroughly before using on mainnet
2. Properly configure private keys and API tokens
3. Pay attention to slippage settings to avoid transaction failures
4. Monitor balances and transaction fees
5. Comply with relevant laws and regulations

## Language Versions

- [English](README.md)
- [中文](README_CN.md)

