# Sol Trade SDK
[中文](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README_CN.md) | [English](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README.md) | [Telegram](https://t.me/fnzero_group)

A comprehensive Rust SDK for seamless interaction with Solana DEX trading programs. This SDK provides a robust set of tools and interfaces to integrate PumpFun, PumpSwap, Bonk, and Raydium CPMM functionality into your applications.

## Project Features

1. **PumpFun Trading**: Support for `buy` and `sell` operations
2. **PumpSwap Trading**: Support for PumpSwap pool trading operations
3. **Bonk Trading**: Support for Bonk trading operations
4. **Raydium CPMM Trading**: Support for Raydium CPMM (Concentrated Pool Market Maker) trading operations
5. **Raydium AMM V4 Trading**: Support for Raydium AMM V4 (Automated Market Maker) trading operations
6. **Event Subscription**: Subscribe to PumpFun, PumpSwap, Bonk, Raydium CPMM, and Raydium AMM V4 program trading events
7. **Yellowstone gRPC**: Subscribe to program events using Yellowstone gRPC
8. **ShredStream Support**: Subscribe to program events using ShredStream
9. **Multiple MEV Protection**: Support for Jito, Nextblock, ZeroSlot, Temporal, Bloxroute, Node1, and other services
10. **Concurrent Trading**: Send transactions using multiple MEV services simultaneously; the fastest succeeds while others fail
11. **Unified Trading Interface**: Use unified trading protocol enums for trading operations
12. **Middleware System**: Support for custom instruction middleware to modify, add, or remove instructions before transaction execution

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
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.4.7" }
```

### Use crates.io

```toml
# Add to your Cargo.toml
sol-trade-sdk = "0.4.7"
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

#### lookup_table_key Parameter

The `lookup_table_key` parameter is an optional `Pubkey` that specifies an address lookup table for transaction optimization:

- **Purpose**: Address lookup tables can reduce transaction size and improve execution speed by storing frequently used addresses
- **Usage**: 
  - Can be set globally in `TradeConfig` for all transactions
  - Can be overridden per transaction in `buy()` and `sell()` methods
  - If not provided, defaults to `None`
- **Benefits**:
  - Reduces transaction size by referencing addresses from lookup tables
  - Improves transaction success rate and speed
  - Particularly useful for complex transactions with many account references

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
            // ..... 
            // For more events and documentation, please refer to https://github.com/0xfnzero/solana-streamer
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
        RAYDIUM_AMM_V4_PROGRAM_ID.to_string(), // Listen to raydium_amm_v4 program ID
        "xxxxxxxx".to_string(),              // Listen to xxxxx account
    ];
    let account_exclude = vec![];
    let account_required = vec![];

    // Transaction filter for monitoring transaction data
    let transaction_filter = TransactionFilter {
        account_include: account_include.clone(),
        account_exclude,
        account_required,
    };

    // Account filter for monitoring account data owned by programs
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
            // ..... 
            // For more events and documentation, please refer to https://github.com/0xfnzero/solana-streamer
        });
    };

    // Subscribe to events
    println!("Starting to listen for events, press Ctrl+C to stop...");
    let protocols = vec![Protocol::PumpFun, Protocol::PumpSwap, Protocol::Bonk, Protocol::RaydiumCpmm];
    shred_stream
        .shredstream_subscribe(protocols, None, None, callback)
        .await?;

    Ok(())
}
```

### 2. Initialize SolanaTrade Instance

#### 2.1 SWQOS Service Configuration

When configuring SWQOS services, note the different parameter requirements for each service:

- **Jito**: The first parameter is UUID, if you don't have a UUID, pass an empty string `""`
- **NextBlock**: The first parameter is API Token
- **Bloxroute**: The first parameter is API Token  
- **ZeroSlot**: The first parameter is API Token
- **Temporal**: The first parameter is API Token
- **FlashBlock**: The first parameter is API Token, Add the official TG support at https://t.me/FlashBlock_Official to get a free key and instantly accelerate your trades! Official docs: https://doc.flashblock.trade/
- **Node1**: The first parameter is API Token, Add the official TG support at https://t.me/node1_me
 to get a free key and instantly accelerate your trades! Official docs: https://node1.me/docs.html

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
        // First parameter is UUID, pass empty string if no UUID
        SwqosConfig::Jito("your uuid".to_string(), SwqosRegion::Frankfurt), 
        SwqosConfig::NextBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Bloxroute("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::ZeroSlot("your api_token".to_string(), SwqosRegion::Frankfurt),
        SwqosConfig::Temporal("your api_token".to_string(), SwqosRegion::Frankfurt),
        // Add tg official customer https://t.me/FlashBlock_Official to get free FlashBlock key
        SwqosConfig::FlashBlock("your api_token".to_string(), SwqosRegion::Frankfurt),
        // Add tg official customer https://t.me/node1_me to get free Node1 key
        SwqosConfig::Node1("your api_token".to_string(), SwqosRegion::Frankfurt),
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
        None,
    )
    .await?;
}
```

### 4. PumpSwap Trading Operations

```rust
use sol_trade_sdk::trading::core::params::PumpSwapParams;

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
    let pool_base_token_reserves = 0; // Input the correct value
    let pool_quote_token_reserves = 0; // Input the correct value

    // Buy tokens
    println!("Buying tokens from PumpSwap...");
    client.buy(
        DexType::PumpSwap,
        mint_pubkey,
        Some(creator),
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        // Through RPC call, adds latency. Can optimize by using from_buy_trade or manually initializing PumpSwapParams
        Box::new(PumpSwapParams::from_pool_address_by_rpc(&client.rpc, &pool_address).await?),
        None,
    ).await?;

    // Sell tokens
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
        // Through RPC call, adds latency. Can optimize by using from_sell_trade or manually initializing PumpSwapParams
        Box::new(PumpSwapParams::from_pool_address_by_rpc(&client.rpc, &pool_address).await?),
        None,
    ).await?;

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
        // Through RPC call, adds latency, or manually initialize RaydiumCpmmParams
        Box::new(
            RaydiumCpmmParams::from_pool_address_by_rpc(&trade_client.rpc, &pool_state).await?,
        ),
        None,
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
        // Through RPC call, adds latency, or manually initialize RaydiumCpmmParams
        Box::new(
            RaydiumCpmmParams::from_pool_address_by_rpc(&trade_client.rpc, &pool_state).await?,
        ),
        None,
    ).await?;

    Ok(())
}
```

### 6. Raydium AMM V4 Trading Operations

```rust
use sol_trade_sdk::trading::core::params::RaydiumAmmV4Params;

async fn test_raydium_amm_v4() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Raydium AMM V4 trading...");

    let trade_client = test_create_solana_trade_client().await?;

    let mint_pubkey = Pubkey::from_str("xxxxxxx")?; // Token address
    let buy_sol_cost = 100_000; // 0.0001 SOL (in lamports)
    let slippage_basis_points = Some(100); // 1% slippage
    let recent_blockhash = trade_client.rpc.get_latest_blockhash().await?;
    let amm_address = Pubkey::from_str("xxxxxx")?; // AMM pool address

    println!("Buying tokens from Raydium AMM V4...");
    trade_client.buy(
        DexType::RaydiumAmmV4,
        mint_pubkey,
        None,
        buy_sol_cost,
        slippage_basis_points,
        recent_blockhash,
        None,
        // Through RPC call, adds latency, or from_amm_info_and_reserves or manually initialize RaydiumAmmV4Params
        Box::new(
            RaydiumAmmV4Params::from_amm_address_by_rpc(&trade_client.rpc, amm_address).await?,
        ),
        None,
    ).await?;

    println!("Selling tokens from Raydium AMM V4...");
    let amount_token = 100_000_000; // Token amount to sell
    
    trade_client.sell(
        DexType::RaydiumAmmV4,
        mint_pubkey,
        None,
        amount_token,
        slippage_basis_points,
        recent_blockhash,
        None,
        false,
        // Through RPC call, adds latency, or from_amm_info_and_reserves or manually initialize RaydiumAmmV4Params
        Box::new(
            RaydiumAmmV4Params::from_amm_address_by_rpc(&trade_client.rpc, amm_address).await?,
        ),
        None,
    ).await?;

    Ok(())
}
```

### 7. Bonk Trading Operations

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
        // Through RPC call, adds latency. Can optimize by using from_trade or manually initializing BonkParams
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
        // Through RPC call, adds latency. Can optimize by using from_trade or manually initializing BonkParams
        Box::new(BonkParams::from_mint_by_rpc(&trade_client.rpc, &mint_pubkey).await?),
        None,
    )
    .await?;

    Ok(())
}
```

### 8. Middleware System

The SDK provides a powerful middleware system that allows you to modify, add, or remove instructions before transaction execution. This gives you tremendous flexibility to customize trading behavior.

#### 8.1 Using Built-in Logging Middleware

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
    
    // SDK example middleware that prints instruction information
    // You can reference LoggingMiddleware to implement the InstructionMiddleware trait for your own middleware
    let middleware_manager = MiddlewareManager::new()
        .add_middleware(Box::new(LoggingMiddleware));
    
    client = client.with_middleware_manager(middleware_manager);
    
    let creator = Pubkey::from_str("11111111111111111111111111111111")?;
    let mint_pubkey = Pubkey::from_str("xxxxx")?;
    let buy_sol_cost = 100_000;
    let slippage_basis_points = Some(100);
    let recent_blockhash = client.rpc.get_latest_blockhash().await?;
    let pool_address = Pubkey::from_str("xxxx")?;
    
    // Buy tokens
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

#### 8.2 Creating Custom Middleware

You can create custom middleware by implementing the `InstructionMiddleware` trait:

```rust
use sol_trade_sdk::trading::middleware::traits::InstructionMiddleware;
use anyhow::Result;
use solana_sdk::instruction::Instruction;

/// Custom middleware example - Add additional instructions
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
        println!("Custom middleware processing, protocol: {}", protocol_name);
        
        // Here you can:
        // 1. Modify existing instructions
        // 2. Add new instructions
        // 3. Remove specific instructions
        
        // Example: Add a custom instruction at the beginning
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
        println!("Custom middleware processing, instruction count: {}", full_instructions.len());
        Ok(full_instructions)
    }

    fn clone_box(&self) -> Box<dyn InstructionMiddleware> {
        Box::new(self.clone())
    }
}

// Using custom middleware
async fn test_custom_middleware() -> AnyResult<()> {
    let mut client = test_create_solana_trade_client().await?;
    
    let middleware_manager = MiddlewareManager::new()
        .add_middleware(Box::new(LoggingMiddleware))           // Logging middleware
        .add_middleware(Box::new(CustomMiddleware));
    
    client = client.with_middleware_manager(middleware_manager);
    
    // Now all transactions will be processed through your middleware
    // ...
    Ok(())
}
```

#### 8.3 Middleware Execution Order

Middleware executes in the order they are added:

```rust
let middleware_manager = MiddlewareManager::new()
    .add_middleware(Box::new(FirstMiddleware))   // Executes first
    .add_middleware(Box::new(SecondMiddleware))  // Executes second
    .add_middleware(Box::new(ThirdMiddleware));  // Executes last
```

### 9. Custom Priority Fee Configuration

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
- **Raydium AMM V4**: Raydium's Automated Market Maker V4 protocol

## MEV Protection Services

- **Jito**: High-performance block space
- **NextBlock**: Fast transaction execution
- **ZeroSlot**: Zero-latency transactions
- **Temporal**: Time-sensitive transactions
- **Bloxroute**: Blockchain network acceleration
- **FlashBlock**: High-speed transaction execution with API key authentication - [Official Docs](https://doc.flashblock.trade/)
- **Node1**: High-speed transaction execution with API key authentication - [Official Docs](https://node1.me/docs.html)

## New Architecture Features

### Unified Trading Interface

- **TradingProtocol Enum**: Use unified protocol enums (PumpFun, PumpSwap, Bonk, RaydiumCpmm, RaydiumAmmV4)
- **Unified buy/sell Methods**: All protocols use the same trading method signatures
- **Protocol-specific Parameters**: Each protocol has its own parameter structure (PumpFunParams, RaydiumCpmmParams, RaydiumAmmV4Params, etc.)

### Event Parsing System

- **Unified Event Interface**: All protocol events implement the UnifiedEvent trait
- **Protocol-specific Events**: Each protocol has its own event types
- **Event Factory**: Automatically identifies and parses events from different protocols

### Trading Engine

- **Unified Trading Interface**: All trading operations use the same methods
- **Protocol Abstraction**: Supports trading operations across multiple protocols
- **Concurrent Execution**: Supports sending transactions to multiple MEV services simultaneously

## Price Calculation Utilities

The SDK includes price calculation utilities for all supported protocols in `src/utils/price/`.

## Amount Calculation Utilities

The SDK provides trading amount calculation functionality for various protocols, located in `src/utils/calc/`:

- **Common Calculation Functions**: Provides general fee calculation and division utilities
- **Protocol-Specific Calculations**: Specialized calculation logic for each protocol
  - **PumpFun**: Token buy/sell amount calculations based on bonding curves
  - **PumpSwap**: Amount calculations for multiple trading pairs
  - **Raydium AMM V4**: Amount and fee calculations for automated market maker pools
  - **Raydium CPMM**: Amount calculations for constant product market makers
  - **Bonk**: Specialized calculation logic for Bonk tokens

Key features include:
- Calculate output amounts based on input amounts
- Fee calculation and distribution
- Slippage protection calculations
- Liquidity pool state calculations

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
│   ├── middleware/   # Middleware system
│   │   ├── builtin.rs    # Built-in middleware implementations
│   │   ├── traits.rs     # Middleware trait definitions
│   │   └── mod.rs        # Middleware module
│   ├── bonk/         # Bonk trading implementation
│   ├── pumpfun/      # PumpFun trading implementation
│   ├── pumpswap/     # PumpSwap trading implementation
│   ├── raydium_cpmm/ # Raydium CPMM trading implementation
│   ├── raydium_amm_v4/ # Raydium AMM V4 trading implementation
│   └── factory.rs    # Trading factory
├── utils/            # Utility functions
│   ├── price/        # Price calculation utilities
│   │   ├── common.rs       # Common price functions
│   │   ├── bonk.rs         # Bonk price calculations
│   │   ├── pumpfun.rs      # PumpFun price calculations
│   │   ├── pumpswap.rs     # PumpSwap price calculations
│   │   ├── raydium_cpmm.rs # Raydium CPMM price calculations
│   │   ├── raydium_clmm.rs # Raydium CLMM price calculations
│   │   └── raydium_amm_v4.rs # Raydium AMM V4 price calculations
│   └── calc/         # Amount calculation utilities
│       ├── common.rs       # Common calculation functions
│       ├── bonk.rs         # Bonk amount calculations
│       ├── pumpfun.rs      # PumpFun amount calculations
│       ├── pumpswap.rs     # PumpSwap amount calculations
│       ├── raydium_cpmm.rs # Raydium CPMM amount calculations
│       └── raydium_amm_v4.rs # Raydium AMM V4 amount calculations
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

