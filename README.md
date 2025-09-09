# Sol Trade SDK
[中文](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README_CN.md) | [English](https://github.com/0xfnzero/sol-trade-sdk/blob/main/README.md) | [Website](https://fnzero.dev/) | [Telegram](https://t.me/fnzero_group)

A comprehensive Rust SDK for seamless interaction with Solana DEX trading programs. This SDK provides a robust set of tools and interfaces to integrate PumpFun, PumpSwap, and Bonk functionality into your applications.

## Project Features

1. **PumpFun Trading**: Support for `buy` and `sell` operations
2. **PumpSwap Trading**: Support for PumpSwap pool trading operations
3. **Bonk Trading**: Support for Bonk trading operations
4. **Raydium CPMM Trading**: Support for Raydium CPMM (Concentrated Pool Market Maker) trading operations
5. **Raydium AMM V4 Trading**: Support for Raydium AMM V4 (Automated Market Maker) trading operations
6. **Event Subscription**: Subscribe to PumpFun, PumpSwap, Bonk, Raydium CPMM, and Raydium AMM V4 program trading events
7. **Yellowstone gRPC**: Subscribe to program events using Yellowstone gRPC
8. **ShredStream Support**: Subscribe to program events using ShredStream
9. **Multiple MEV Protection**: Support for Jito, Nextblock, ZeroSlot, Temporal, Bloxroute, FlashBlock, BlockRazor, Node1, Astralane and other services
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
sol-trade-sdk = { path = "./sol-trade-sdk", version = "0.6.1" }
```

### Use crates.io

```toml
# Add to your Cargo.toml
sol-trade-sdk = "0.6.1"
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

The `lookup_table_key` parameter is an optional `Pubkey` that specifies an address lookup table for transaction optimization. You need to use `AddressLookupTableCache` to manage the cached address lookup table before using it.

- **Purpose**: Address lookup tables can reduce transaction size and improve execution speed by storing frequently used addresses
- **Usage**: 
  - Can be set globally in `TradeConfig` for all transactions
  - Can be overridden per transaction in `buy()` and `sell()` methods
  - If not provided, defaults to `None`
- **Benefits**:
  - Reduces transaction size by referencing addresses from lookup tables
  - Improves transaction success rate and speed
  - Particularly useful for complex transactions with many account references

#### priority_fee Parameter

The `priority_fee` parameter is an optional `PriorityFee` that allows you to override the default priority fee settings for individual transactions:

- **Purpose**: Provides fine-grained control over transaction priority fees on a per-transaction basis
- **Usage**:
  - Can be passed to `buy()` and `sell()` methods to override the global priority fee settings
  - If not provided, defaults to `None` and uses the priority fee settings from `TradeConfig`
  - When provided, the `buy_tip_fees` array will be automatically padded to match the number of SWQOS clients
- **Benefits**:
  - Allows dynamic adjustment of priority fees based on market conditions
  - Enables different fee strategies for different types of transactions
  - Provides flexibility for high-frequency trading scenarios

#### About ShredStream

When using shred to subscribe to events, due to the nature of shreds, you cannot get complete information about transaction events.
Please ensure that the parameters your trading logic depends on are available in shreds when using them.

### Usage Examples Summary Table

| Feature Type | Package Name | Description | Run Command | Source Code |
|-------------|--------------|-------------|-------------|-------------|
| Event Subscription | `event_subscription` | Monitor token trading events | `cargo run --package event_subscription` | [examples/event_subscription](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/event_subscription/src/main.rs) |
| Trading Client | `trading_client` | Create and configure SolanaTrade instance | `cargo run --package trading_client` | [examples/trading_client](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/trading_client/src/main.rs) |
| PumpFun Sniping | `pumpfun_sniper_trading` | PumpFun token sniping trading | `cargo run --package pumpfun_sniper_trading` | [examples/pumpfun_sniper_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/pumpfun_sniper_trading/src/main.rs) |
| PumpFun Copy Trading | `pumpfun_copy_trading` | PumpFun token copy trading | `cargo run --package pumpfun_copy_trading` | [examples/pumpfun_copy_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/pumpfun_copy_trading/src/main.rs) |
| PumpSwap | `pumpswap_trading` | PumpSwap trading operations | `cargo run --package pumpswap_trading` | [examples/pumpswap_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/pumpswap_trading/src/main.rs) |
| Raydium CPMM | `raydium_cpmm_trading` | Raydium CPMM trading operations | `cargo run --package raydium_cpmm_trading` | [examples/raydium_cpmm_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/raydium_cpmm_trading/src/main.rs) |
| Raydium AMM V4 | `raydium_amm_v4_trading` | Raydium AMM V4 trading operations | `cargo run --package raydium_amm_v4_trading` | [examples/raydium_amm_v4_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/raydium_amm_v4_trading/src/main.rs) |
| Bonk Sniping | `bonk_sniper_trading` | Bonk token sniping trading | `cargo run --package bonk_sniper_trading` | [examples/bonk_sniper_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/bonk_sniper_trading/src/main.rs) |
| Bonk Copy Trading | `bonk_copy_trading` | Bonk token copy trading | `cargo run --package bonk_copy_trading` | [examples/bonk_copy_trading](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/bonk_copy_trading/src/main.rs) |
| Middleware System | `middleware_system` | Custom instruction middleware example | `cargo run --package middleware_system` | [examples/middleware_system](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/middleware_system/src/main.rs) |
| Address Lookup | `address_lookup` | Address lookup table example | `cargo run --package address_lookup` | [examples/address_lookup](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/address_lookup/src/main.rs) |
| Nonce | `nonce_cache` | Nonce example | `cargo run --package nonce_cache` | [examples/nonce_cache](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/nonce_cache/src/main.rs) |
| WSOL Wrapper | `wsol_wrapper` | Wrap/unwrap SOL to/from WSOL example | `cargo run --package wsol_wrapper` | [examples/wsol_wrapper](https://github.com/0xfnzero/sol-trade-sdk/tree/main/examples/wsol_wrapper/src/main.rs) |

### SWQOS Service Configuration

When configuring SWQOS services, note the different parameter requirements for each service:

- **Jito**: The first parameter is UUID, if you don't have a UUID, pass an empty string `""`
- **NextBlock**: The first parameter is API Token
- **Bloxroute**: The first parameter is API Token  
- **ZeroSlot**: The first parameter is API Token
- **Temporal**: The first parameter is API Token
- **FlashBlock**: The first parameter is API Token, Add community TG admin [xyz_0xfnzero](https://t.me/xyz_0xfnzero) to get a free key and instantly speed up your trading (with tip refunds available)!
- **BlockRazor**: The first parameter is API Token, Add official TG support to get a free key and instantly accelerate your trades!
- **Node1**: The first parameter is API Token, Add the official TG support at https://t.me/node1_me to get a free key and instantly accelerate your trades!
- **Astralane**: The first parameter is API Token

#### Custom URL Support

Each SWQOS service now supports an optional custom URL parameter:

```rust
// Using custom URL (third parameter)
let jito_config = SwqosConfig::Jito(
    "your_uuid".to_string(),
    SwqosRegion::Frankfurt, // This parameter is still required but will be ignored
    Some("https://custom-jito-endpoint.com".to_string()) // Custom URL
);

// Using default regional endpoint (third parameter is None)
let nextblock_config = SwqosConfig::NextBlock(
    "your_api_token".to_string(),
    SwqosRegion::NewYork, // Will use the default endpoint for this region
    None // No custom URL, uses SwqosRegion
);
```

**URL Priority Logic**:
- If a custom URL is provided (`Some(url)`), it will be used instead of the regional endpoint
- If no custom URL is provided (`None`), the system will use the default endpoint for the specified `SwqosRegion`
- This allows for maximum flexibility while maintaining backward compatibility 

When using multiple MEV services, you need to use `Durable Nonce`. You need to initialize a `NonceCache` class (or write your own nonce management class), get the latest `nonce` value, and use it as the `blockhash` when trading.

### Middleware System Explanation

The SDK provides a powerful middleware system that allows you to modify, add, or remove instructions before transaction execution. Middleware executes in the order they are added:

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
    tip_unit_limit: 190000,
    tip_unit_price: 1000000,
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
- **FlashBlock**: High-speed transaction execution with API key authentication - [Official Documentation](https://doc.flashblock.trade/)
- **BlockRazor**: High-speed transaction execution with API key authentication - [Official Documentation](https://blockrazor.gitbook.io/blockrazor/)
- **Node1**: High-speed transaction execution with API key authentication - [Official Documentation](https://node1.me/docs.html) 
- **Astralane**: Blockchain network acceleration

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

- Official Website: https://fnzero.dev/
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
