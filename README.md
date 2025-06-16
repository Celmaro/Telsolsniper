# Sol Trade SDK

A comprehensive Rust SDK for seamless interaction with Solana DEX trading programs. This SDK provides a robust set of tools and interfaces to integrate PumpFun and PumpSwap functionality into your applications.

## Features

1. **PumpFun Trading**: Support for `create`, `buy`, `sell` operations
2. **PumpSwap Trading**: Support for PumpSwap pool trading operations
3. **Logs Subscription**: Subscribe to PumpFun program transaction logs
4. **Yellowstone gRPC**: Subscribe to program logs using gRPC
5. **Multiple MEV Protection**: Support for Jito, Nextblock, 0slot, Nozomi services
6. **Concurrent Transactions**: Submit transactions using multiple MEV services simultaneously; the fastest succeeds while others fail
7. **IPFS Integration**: Support for token metadata IPFS uploads
8. **Real-time Pricing**: Get real-time token prices and liquidity information

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

### 1. Logs Subscription - Monitor Token Creation and Trading

```rust
use sol_trade_sdk::{common::pumpfun::logs_events::PumpfunEvent, grpc::YellowstoneGrpc};
use solana_sdk::signature::Keypair;

// Create gRPC client
let grpc_url = "http://127.0.0.1:10000";
let x_token = None; // Optional auth token
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// Define callback function
let callback = |event: PumpfunEvent| {
    match event {
        PumpfunEvent::NewToken(token_info) => {
            println!("Received new token event: {:?}", token_info);
        },
        PumpfunEvent::NewDevTrade(trade_info) => {
            println!("Received dev trade event: {:?}", trade_info);
        },
        PumpfunEvent::NewUserTrade(trade_info) => {
            println!("Received new trade event: {:?}", trade_info);
        },
        PumpfunEvent::NewBotTrade(trade_info) => {
            println!("Received new bot trade event: {:?}", trade_info);
        }
        PumpfunEvent::Error(err) => {
            println!("Received error: {}", err);
        }
    }
};

let payer_keypair = Keypair::from_base58_string("your_private_key");
client.subscribe_pumpfun(callback, Some(payer_keypair.pubkey())).await?;
```

### 2. Initialize PumpFun Instance

```rust
use std::sync::Arc;
use sol_trade_sdk::{common::{Cluster, PriorityFee}, PumpFun};
use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair};

// Configure priority fees
let priority_fee = PriorityFee {
    unit_limit: 190000,
    unit_price: 1000000,
    rpc_unit_limit: 500000,
    rpc_unit_price: 500000,
    buy_tip_fee: 0.001,
    buy_tip_fees: vec![0.001, 0.002],
    sell_tip_fee: 0.0001,
};

// Configure cluster
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
    lookup_table_key: None, // Optional lookup table
};

// Create PumpFun instance
let payer = Keypair::from_base58_string("your_private_key");
let pumpfun = PumpFun::new(Arc::new(payer), &cluster).await;
```

### 3. Create Token

```rust
use sol_trade_sdk::{PumpFun, ipfs::CreateTokenMetadata, ipfs::create_token_metadata};
use solana_sdk::signature::Keypair;

// Create token keypair
let mint_keypair = Keypair::new();

// Prepare token metadata
let metadata = CreateTokenMetadata {
    name: "My Token".to_string(),
    symbol: "MTK".to_string(),
    description: "This is a test token".to_string(),
    file: "path/to/image.png".to_string(), // Local file path
    twitter: Some("https://twitter.com/example".to_string()),
    telegram: Some("https://t.me/example".to_string()),
    website: Some("https://example.com".to_string()),
    metadata_uri: None, // Will be generated
};

// Upload metadata to IPFS
let api_token = "your_pinata_api_token";
let ipfs_response = create_token_metadata(metadata, api_token).await?;

// Create token
pumpfun.create(mint_keypair, ipfs_response).await?;
```

### 4. Buy Tokens

```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};

let mint_pubkey = Pubkey::from_str("token_address")?;
let creator = Pubkey::from_str("creator_address")?;
let recent_blockhash = Hash::default(); // Get latest blockhash

// Sniper buy (fast purchase when new token launches)
pumpfun.sniper_buy_with_tip(
    mint_pubkey,
    creator,
    1000000,  // dev_buy_token
    10000,    // dev_sol_cost  
    50000,    // buy_sol_cost (lamports)
    Some(100), // slippage (1%)
    recent_blockhash,
).await?;

// Copy buy (follow other traders)
pumpfun.copy_buy_with_tip(
    mint_pubkey,
    creator,
    1000000,  // dev_buy_token
    10000,    // dev_sol_cost
    50000,    // buy_sol_cost (lamports)
    Some(100), // slippage (1%)
    recent_blockhash,
    "pumpfun".to_string(), // trading platform
).await?;
```

### 5. Sell Tokens

```rust
// Sell by amount
pumpfun.sell_by_amount_with_tip(
    mint_pubkey,
    creator,
    1000000, // token amount
    recent_blockhash,
    "pumpfun".to_string(),
).await?;

// Sell by percentage
pumpfun.sell_by_percent_with_tip(
    mint_pubkey,
    creator,
    50,      // percentage (50%)
    2000000, // total token amount
    recent_blockhash,
    "pumpfun".to_string(),
).await?;
```

### 6. Get Price and Balance Information

```rust
// Get current token price
let price = pumpfun.get_current_price(&mint_pubkey).await?;
println!("Current price: {}", price);

// Get SOL balance
let sol_balance = pumpfun.get_payer_sol_balance().await?;
println!("SOL balance: {} lamports", sol_balance);

// Get token balance
let token_balance = pumpfun.get_payer_token_balance(&mint_pubkey).await?;
println!("Token balance: {}", token_balance);

// Get liquidity information
let sol_reserves = pumpfun.get_real_sol_reserves(&mint_pubkey).await?;
println!("SOL reserves: {} lamports", sol_reserves);
```

### 7. PumpSwap Subscription - Monitor AMM Events

```rust
use sol_trade_sdk::{common::pumpswap::logs_events::PumpSwapEvent, grpc::YellowstoneGrpc};

// Create gRPC client (same as above)
let grpc_url = "http://127.0.0.1:10000";
let x_token = None;
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// Define callback function for PumpSwap events
let callback = |event: PumpSwapEvent| {
    match event {
        PumpSwapEvent::Buy(buy_event) => {
            println!("PumpSwap Buy Event: {:?}", buy_event);
        },
        PumpSwapEvent::Sell(sell_event) => {
            println!("PumpSwap Sell Event: {:?}", sell_event);
        },
        PumpSwapEvent::CreatePool(pool_event) => {
            println!("PumpSwap Pool Created: {:?}", pool_event);
        },
        PumpSwapEvent::Deposit(deposit_event) => {
            println!("PumpSwap Deposit: {:?}", deposit_event);
        },
        PumpSwapEvent::Withdraw(withdraw_event) => {
            println!("PumpSwap Withdraw: {:?}", withdraw_event);
        },
        PumpSwapEvent::Disable(disable_event) => {
            println!("PumpSwap Pool Disabled: {:?}", disable_event);
        },
        PumpSwapEvent::UpdateAdmin(admin_event) => {
            println!("PumpSwap Admin Updated: {:?}", admin_event);
        },
        PumpSwapEvent::UpdateFeeConfig(fee_event) => {
            println!("PumpSwap Fee Config Updated: {:?}", fee_event);
        },
        PumpSwapEvent::Error(err) => {
            println!("PumpSwap Error: {}", err);
        }
    }
};

// Subscribe to PumpSwap events
client.subscribe_pumpswap(callback).await?;
```

### 8. PumpSwap Trading Operations

```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};

let mint_pubkey = Pubkey::from_str("token_address")?;
let creator = Pubkey::from_str("creator_address")?;
let recent_blockhash = Hash::default();

// Buy tokens on PumpSwap
pumpfun.copy_buy_with_tip(
    mint_pubkey,
    creator,
    1000000,  // dev_buy_token
    10000,    // dev_sol_cost
    50000,    // buy_sol_cost (lamports)
    Some(100), // slippage (1%)
    recent_blockhash,
    "pumpswap".to_string(), // Use PumpSwap platform
).await?;

// Sell tokens on PumpSwap by amount
pumpfun.sell_by_amount_with_tip(
    mint_pubkey,
    creator,
    1000000, // token amount
    recent_blockhash,
    "pumpswap".to_string(), // Use PumpSwap platform
).await?;

// Sell tokens on PumpSwap by percentage
pumpfun.sell_by_percent_with_tip(
    mint_pubkey,
    creator,
    50,      // percentage (50%)
    2000000, // total token amount
    recent_blockhash,
    "pumpswap".to_string(), // Use PumpSwap platform
).await?;
```

### 9. PumpSwap Pool Information

```rust
use solana_sdk::pubkey::Pubkey;

let pool_address = Pubkey::from_str("pool_address")?;

// Get current price from PumpSwap pool
let price = pumpfun.get_current_price_with_pumpswap(&pool_address).await?;
println!("PumpSwap pool price: {}", price);

// Get SOL reserves in PumpSwap pool
let sol_reserves = pumpfun.get_real_sol_reserves_with_pumpswap(&pool_address).await?;
println!("PumpSwap SOL reserves: {} lamports", sol_reserves);

// Get token balance in PumpSwap pool
let token_balance = pumpfun.get_payer_token_balance_with_pumpswap(&pool_address).await?;
println!("PumpSwap token balance: {}", token_balance);
```

## Supported Trading Platforms

- **PumpFun**: Primary meme coin trading platform
- **PumpSwap**: PumpFun's swap protocol
- **Raydium**: Integrated Raydium DEX functionality

## MEV Protection Services

- **Jito**: High-performance block space
- **Nextblock**: Fast transaction execution
- **0slot**: Zero-latency transactions
- **Nozomi**: MEV protection service

## Project Structure

```
src/
├── accounts/     # Account-related definitions
├── common/       # Common utilities and tools
├── constants/    # Constant definitions
├── error/        # Error handling
├── grpc/         # gRPC clients
├── instruction/  # Instruction building
├── ipfs/         # IPFS integration
├── pumpfun/      # PumpFun trading functionality
├── pumpswap/     # PumpSwap trading functionality
├── swqos/        # MEV service clients
├── lib.rs        # Main library file
└── main.rs       # Example program
```

## License

MIT License

## Contact

- Repository: https://github.com/0xfnzero/sol-trade-sdk
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
