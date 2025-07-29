# Sol Trade SDK
[中文](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README_CN.md) | [English](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README.md) | [Telegram](https://t.me/fnzero_group)

A comprehensive Rust SDK for seamless interaction with Solana DEX trading programs. This SDK provides a robust set of tools and interfaces to integrate PumpFun, PumpSwap, Bonk, and Raydium CPMM functionality into your applications.

## Project Features

1. **PumpFun Trading**: Support for `buy` and `sell` operations
2. **PumpSwap Trading**: Support for PumpSwap pool trading operations
3. **Bonk Trading**: Support for Bonk trading operations
4. **Raydium CPMM Trading**: Support for Raydium CPMM (Concentrated Pool Market Maker) trading operations
5. **Event Subscription**: Subscribe to PumpFun, PumpSwap, Bonk, and Raydium CPMM program trading events
6. **Yellowstone gRPC**: Subscribe to program events using Yellowstone gRPC
7. **ShredStream Support**: Subscribe to program events using ShredStream
8. **Multiple MEV Protection**: Support for Jito, Nextblock, ZeroSlot, Temporal, Bloxroute, and other services
9. **Concurrent Trading**: Send transactions using multiple MEV services simultaneously; the fastest succeeds while others fail
10. **Unified Trading Interface**: Use unified trading protocol enums for trading operations

## Installation

### Direct Clone

Clone this project to your project directory:

```bash
cd your_project_root_directory
git clone https://github.com/0xfnzero/sol-trade-sdk
```

Add the dependency to your `Cargo.toml`:

```toml
# Add to your Cargo.toml
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.2.9" }
```

### Use crates.io

```toml
# Add to your Cargo.toml
sol-trade-sdk = "0.2.9"
```

## Usage Examples

### Important Parameter Description

#### auto_handle_wsol Parameter

In PumpSwap, Bonk, and Raydium CPMM trading, the `auto_handle_wsol` parameter is used to automatically handle wSOL (Wrapped SOL):

- **Mechanism**:
  - When `auto_handle_wsol: true`, the SDK automatically handles the conversion between SOL and wSOL
  - When buying: automatically wraps SOL to wSOL for trading
  - When selling: automatically unwraps the received wSOL to SOL
  - Default value is `true`

### 1. Event Subscription - Monitor Token Trading

#### 1.1 Subscribe to Events Using Yellowstone gRPC

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
        YellowstoneGrpc,
    },
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
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                println!("Raydium CPMM Swap event: {:?}", e);
            },
        });
    };

    // Subscribe to events from multiple protocols
    println!("Starting to listen for events, press Ctrl+C to stop...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk, Protocol::RaydiumCpmm];
    
    // Filter accounts
    let account_include = vec![
        PUMPFUN_PROGRAM_ID.to_string(),      // Listen to pumpfun program ID
        PUMPSWAP_PROGRAM_ID.to_string(),     // Listen to pumpswap program ID
        BONK_PROGRAM_ID.to_string(),         // Listen to bonk program ID
        RAYDIUM_CPMM_PROGRAM_ID.to_string(), // Listen to raydium_cpmm program ID
        RAYDIUM_CLMM_PROGRAM_ID.to_string(), // Listen to raydium_clmm program ID
        "xxxxxxxx".to_string(),              // Listen to xxxxx account
    ];
    let account_exclude = vec![];
    let account_required = vec![];

    grpc.subscribe_events_v2(
        protocols,
        None,
        account_include,
        account_exclude,
        account_required,
        None,
        callback,
    )
    .await?;

    Ok(())
}
```

#### 1.2 Subscribe to Events Using ShredStream

```rust
use sol_trade_sdk::solana_streamer_sdk::streaming::ShredStreamGrpc;

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
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                println!("Raydium CPMM Swap event: {:?}", e);
            },
        });
    };

    // Subscribe to events
    println!("Starting to listen for events, press Ctrl+C to stop...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk, Protocol::RaydiumCpmm];
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
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair};

/// Example of creating a SolanaTrade client
async fn test_create_solana_trade_client() -> AnyResult<SolanaTrade> {
    println!("Creating SolanaTrade client...");

    let payer = Keypair::new();
    let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();

    // Configure various SWQOS services
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

    let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
    println!("SolanaTrade client created successfully!");

    Ok(solana_trade_client)
}
```

### 3. PumpFun Trading Operations

```rust
use sol_trade_sdk::{
    common::bonding_curve::BondingCurveAccount,
    constants::pumpfun::global_constants::TOKEN_TOTAL_SUPPLY,
    trading::{core::params::PumpFunParams, factory::DexType},
};

// pumpfun sniper trade
async fn test_pumpfun_sniper_trade_with_shreds(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    println!("Testing PumpFun trading...");

    // if not dev trade, return
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
    
    // By not using RPC to fetch the bonding curve, transaction time can be saved.
    let bonding_curve = BondingCurveAccount::from_dev_trade(
        &mint_pubkey,
        dev_token_amount,
        dev_sol_amount,
        creator,
    );

    // my trade cost sol amount
    let buy_sol_amount = 100_000;
    trade_client.buy(
        DexType::PumpFun,
        mint_pubkey,
        Some(creator),
        buy_sol_amount,
        slippage_basis_points,
        recent_blockhash,
        None,
        Some(Box::new(PumpFunParams {
            bonding_curve: Some(Arc::new(bonding_curve.clone())),
        })),
    )
    .await?;

    Ok(())
}

// pumpfun copy trade
async fn test_pumpfun_copy_trade_with_grpc(trade_info: PumpFunTradeEvent) -> AnyResult<()> {
    println!("Testing PumpFun trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = trade_info.mint;
    let creator = trade_info.creator;
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;

    println!("Buying tokens from PumpFun...");

    // my trade cost sol amount
    let buy_sol_amount = 100_000;

    // By not using RPC to fetch the bonding curve, transaction time can be saved.
    let bonding_curve = BondingCurveAccount::from_trade(&trade_info);

    trade_client.buy(
        DexType::PumpFun,
        mint_pubkey,
        Some(creator),
        buy_sol_amount,
        slippage_basis_points,
        recent_blockhash,
        None,
        Some(Box::new(PumpFunParams {
            bonding_curve: Some(Arc::new(bonding_curve.clone())),
        })),
    )
    .await?;

    Ok(())
}

// pumpfun sell token
async fn test_pumpfun_sell() -> AnyResult<()> {
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
        None,
    )
    .await?;
}
```

### 4. PumpSwap Trading Operations

```rust
async fn test_pumpswap() -> AnyResult<()> {
    println!("Testing PumpSwap trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = Pubkey::from_str("xxxxxxx")?; 
    let creator = Pubkey::from_str("xxxxxx")?; 
    let buy_sol_amount = 100_000; 
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;
    let pool_address = Pubkey::from_str("xxxxxxx")?;

    println!("Buying tokens from PumpSwap...");
    // buy
    trade_client.buy(
        DexType::PumpSwap,
        mint_pubkey,
        Some(creator),
        buy_sol_amount,
        slippage_basis_points,
        recent_blockhash,
        None,
        Some(Box::new(PumpSwapParams {
            pool: Some(pool_address),
            auto_handle_wsol: true,
        })),
    )
    .await?;
    
    
    // sell
    println!("Selling tokens from PumpSwap...");

    let amount_token = 100_000; 
    trade_client.sell(
        DexType::PumpSwap,
        mint_pubkey,
        Some(creator),
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        Some(Box::new(PumpSwapParams {
            pool: Some(pool_address),
            auto_handle_wsol: true,
        })),
    )
    .await?;

    Ok(())
}
```

### 5. Raydium CPMM Trading Operations

```rust
use sol_trade_sdk::{
    trading::{
        core::params::RaydiumCpmmParams, 
        factory::DexType, 
        raydium_cpmm::common::{get_buy_token_amount, get_sell_sol_amount}
    },
};
use spl_token; // For standard SPL Token
// use spl_token_2022; // For Token 2022 standard (if needed)

async fn test_raydium_cpmm() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Raydium CPMM trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = Pubkey::from_str("xxxxxxxx")?; // Token address
    let buy_sol_cost = 100_000; // 0.0001 SOL (in lamports)
    let slippage_basis_points = Some(100); // 1% slippage
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;
    let pool_state = Pubkey::from_str("xxxxxxx")?; // Pool state address

    // Calculate expected token amount when buying
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
        Some(Box::new(RaydiumCpmmParams {
            pool_state: Some(pool_state), // If not provided, will auto-calculate
            mint_token_program: Some(spl_token::ID), // Support spl_token or spl_token_2022::ID
            mint_token_in_pool_state_index: Some(1), // Index of mint_token in pool_state, default is at index 1
            minimum_amount_out: Some(buy_amount_out), // If not provided, defaults to 0
            auto_handle_wsol: true, // Automatically handle wSOL wrapping/unwrapping
        })),
    ).await?;

    println!("Selling tokens from Raydium CPMM...");
    let amount_token = 100_000_000; // Token amount to sell
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
        Some(Box::new(RaydiumCpmmParams {
            pool_state: Some(pool_state), // If not provided, will auto-calculate
            mint_token_program: Some(spl_token::ID), // Support spl_token or spl_token_2022::ID
            mint_token_in_pool_state_index: Some(1), // Index of mint_token in pool_state, default is at index 1
            minimum_amount_out: Some(sell_sol_amount), // If not provided, defaults to 0
            auto_handle_wsol: true, // Automatically handle wSOL wrapping/unwrapping
        })),
    ).await?;

    Ok(())
}
```

### 6. Bonk Trading Operations

```rust

// bonk sniper trade
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
    
    // Use dev trade info to build BonkParams, can save transaction time
    trade_client.buy(
        DexType::Bonk,
        mint_pubkey,
        None,
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        Some(Box::new(BonkParams::from_dev_trade(trade_info))),
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
        None,
    ).await?;

    Ok(())
}

// bonk copy trade
async fn test_bonk_copy_trade_with_grpc(trade_info: BonkTradeEvent) -> AnyResult<()> {
    println!("Testing Bonk trading...");

    let trade_client = test_create_solana_trade_client().await?;
    let mint_pubkey = Pubkey::from_str("xxxxxxx")?;
    let buy_sol_cost = 100_000;
    let slippage_basis_points = Some(100);
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;

    println!("Buying tokens from letsbonk.fun...");
    
    // Use trade event info to build BonkParams, can save transaction time
    trade_client.buy(
        DexType::Bonk,
        mint_pubkey,
        None,
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        Some(Box::new(BonkParams::from_trade(trade_info))),
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
        None,
    ).await?;

    Ok(())
}

// bonk regular trade
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
        None,
    )
    .await?;

    Ok(())
}
```

### 7. Custom Priority Fee Configuration

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
- **Bonk**: Token launch platform (letsbonk.fun)
- **Raydium CPMM**: Raydium's Concentrated Pool Market Maker protocol

## MEV Protection Services

- **Jito**: High-performance block space
- **NextBlock**: Fast transaction execution
- **ZeroSlot**: Zero-latency transactions
- **Temporal**: Time-sensitive transactions
- **Bloxroute**: Blockchain network acceleration

## New Architecture Features

### Unified Trading Interface

- **TradingProtocol Enum**: Use unified protocol enums (PumpFun, PumpSwap, Bonk, RaydiumCpmm)
- **Unified buy/sell Methods**: All protocols use the same trading method signatures
- **Protocol-specific Parameters**: Each protocol has its own parameter structure (PumpFunParams, RaydiumCpmmParams, etc.)

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
├── common/           # Common functionality and tools
├── constants/        # Constant definitions
├── instruction/      # Instruction building
├── swqos/            # MEV service clients
├── trading/          # Unified trading engine
│   ├── common/       # Common trading tools
│   ├── core/         # Core trading engine
│   ├── bonk/         # Bonk trading implementation
│   ├── pumpfun/      # PumpFun trading implementation
│   ├── pumpswap/     # PumpSwap trading implementation
│   ├── raydium_cpmm/ # Raydium CPMM trading implementation
│   └── factory.rs    # Trading factory
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

