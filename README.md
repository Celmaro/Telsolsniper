# Sol Trade SDK

A comprehensive Rust SDK for seamless interaction with Solana DEX trading programs. This SDK provides a robust set of tools and interfaces to integrate PumpFun and PumpSwap functionality into your applications.

## Features

1. **PumpFun Trading**: Support for `buy`, `sell` operations
2. **PumpSwap Trading**: Support for PumpSwap pool trading operations
3. **Raydium Trading**: Support for Raydium DEX trading operations
4. **Logs Subscription**: Subscribe to PumpFun, PumpSwap, and Raydium program transaction logs
5. **Yellowstone gRPC**: Subscribe to program logs using Yellowstone gRPC
6. **ShredStream Support**: Subscribe to program logs using ShredStream
7. **Multiple MEV Protection**: Support for Jito, Nextblock, 0slot, Nozomi services
8. **Concurrent Transactions**: Submit transactions using multiple MEV services simultaneously; the fastest succeeds while others fail
9. **Real-time Pricing**: Get real-time token prices and liquidity information

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

### 1. Logs Subscription - Monitor Token Trading

```rust
use sol_trade_sdk::{common::pumpfun::logs_events::PumpfunEvent, grpc::YellowstoneGrpc};
use solana_sdk::signature::Keypair;

// Create gRPC client with Yellowstone
let grpc_url = "https://solana-yellowstone-grpc.publicnode.com:443";
let x_token = None; // Optional auth token
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// Define callback function
let callback = |event: PumpfunEvent| match event {
    PumpfunEvent::NewDevTrade(trade_info) => {
        println!("Received new dev trade event: {:?}", trade_info);
    }
    PumpfunEvent::NewToken(token_info) => {
        println!("Received new token event: {:?}", token_info);
    }
    PumpfunEvent::NewUserTrade(trade_info) => {
        println!("Received new trade event: {:?}", trade_info);
    }
    PumpfunEvent::NewBotTrade(trade_info) => {
        println!("Received new bot trade event: {:?}", trade_info);
    }
    PumpfunEvent::Error(err) => {
        println!("Received error: {}", err);
    }
};

client.subscribe_pumpfun(callback, None).await?;
```

### 2. Initialize SolanaTrade Instance

```rust
use std::sync::Arc;
use sol_trade_sdk::{common::{Cluster, PriorityFee}, SolanaTrade};
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

// Configure multiple swqos in single region, can send transactions concurrently
let swqos_configs = vec![
    SwqosConfig::new(None, None, SwqosType::Jito, SwqosRegion::Frankfurt),
    SwqosConfig::new(None, Some("your auth_token".to_string()), SwqosType::ZeroSlot, SwqosRegion::Frankfurt),
    SwqosConfig::new(None, Some("your auth_token".to_string()), SwqosType::Temporal, SwqosRegion::Frankfurt),
];

let rpc_url = "https://mainnet.helius-rpc.com/?api-key=xxxxxx".to_string();

// Define sdk configuration
let trade_config = TradeConfig {
    rpc_url: rpc_url.clone(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee,
    swqos_configs,
    lookup_table_key: None,
};

// Create SolanaTrade instance
let payer = Keypair::from_base58_string("your_private_key");
let solana_trade_client = SolanaTrade::new(Arc::new(payer), trade_config).await;
```

### 3. Buy Tokens

### 3.1 Buy Tokens --- Sniping
```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};
use std::sync::Arc;
use sol_trade_sdk::accounts::BondingCurveAccount;

let mint_pubkey = Pubkey::from_str("token_address")?;
let creator = Pubkey::from_str("creator_address")?;
let recent_blockhash = Hash::default(); // Get latest blockhash
let buy_sol_cost = 50000; // 0.00005 SOL
let slippage_basis_points = Some(100); // 1%

// Sniping buy (quick purchase when new token launches)
let dev_buy_token = 100_000; // Test value
let dev_cost_sol = 10_000; // Test value
let bonding_curve = BondingCurveAccount::new(&mint_pubkey, dev_buy_token, dev_cost_sol, creator);

solana_trade_client.sniper_buy(
    mint_pubkey,
    creator,
    buy_sol_cost,
    slippage_basis_points,
    recent_blockhash,
    Some(Arc::new(bonding_curve)),
).await?;

// Buy with MEV protection using tips
solana_trade_client.sniper_buy_with_tip(
    mint_pubkey,
    creator,
    buy_sol_cost,
    slippage_basis_points,
    recent_blockhash,
    Some(Arc::new(bonding_curve)),
    None, // Custom tip
).await?;
```

### 3.2 Buy Tokens --- Copy Trading
```rust
use solana_sdk::{pubkey::Pubkey, hash::Hash};
use std::sync::Arc;
use sol_trade_sdk::accounts::BondingCurveAccount;
use sol_trade_sdk::{constants::{pumpfun::global_constants::TOKEN_TOTAL_SUPPLY, trade_type::COPY_BUY}, pumpfun::common::get_bonding_curve_pda};

let mint_pubkey = Pubkey::from_str("token_address")?;
let creator = Pubkey::from_str("creator_address")?;
let recent_blockhash = Hash::default(); // Get latest blockhash
let buy_sol_cost = 50000; // 0.00005 SOL
let slippage_basis_points = Some(100); // 1%

// Copy trading buy
let dev_buy_token = 100_000; // Test value
let dev_cost_sol = 10_000; // Test value
// trade_info comes from pumpfun parsed data, refer to section 1. Logs Subscription above
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

// Buy with MEV protection using tips
solana_trade_client.buy_with_tip(
    mint_pubkey,
    creator,
    buy_sol_cost,
    slippage_basis_points,
    recent_blockhash,
    Some(Arc::new(bonding_curve)),
    "pumpfun".to_string(), 
    None, // Custom tip
).await?;
```

### 4. Sell Tokens

```rust
// Sell by amount
solana_trade_client.sell_by_amount_with_tip(
    mint_pubkey,
    creator,
    1000000, // token amount
    recent_blockhash,
    "pumpfun".to_string(), // trading platform
).await?;

// Sell by percentage
solana_trade_client.sell_by_percent_with_tip(
    mint_pubkey,
    creator,
    50,      // percentage (50%)
    2000000, // total token amount
    recent_blockhash,
    "pumpfun".to_string(), // trading platform
).await?;
```

### 5. Get Price and Balance Information

```rust
// Get current token price
let price = solana_trade_client.get_current_price(&mint_pubkey).await?;
println!("Current price: {}", price);

// Get SOL balance
let sol_balance = solana_trade_client.get_payer_sol_balance().await?;
println!("SOL balance: {} lamports", sol_balance);

// Get token balance
let token_balance = solana_trade_client.get_payer_token_balance(&mint_pubkey).await?;
println!("Token balance: {}", token_balance);

// Get liquidity information
let sol_reserves = solana_trade_client.get_real_sol_reserves(&mint_pubkey).await?;
println!("SOL reserves: {} lamports", sol_reserves);
```

### 6. PumpSwap Subscription - Monitor AMM Events

```rust
use sol_trade_sdk::{common::pumpswap::logs_events::PumpSwapEvent, grpc::YellowstoneGrpc};

// Create gRPC client with Yellowstone
let grpc_url = "https://solana-yellowstone-grpc.publicnode.com:443";
let x_token = None;
let client = YellowstoneGrpc::new(grpc_url.to_string(), x_token)?;

// Define callback function for PumpSwap events
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

// Subscribe to PumpSwap events
println!("Monitoring PumpSwap events, press Ctrl+C to stop...");
client.subscribe_pumpswap(callback).await?;
```

### 7. PumpSwap Trading Operations

```rust
use std::sync::Arc;
use solana_sdk::{pubkey::Pubkey, hash::Hash, signature::Keypair};
use solana_client::rpc_client::RpcClient;
use sol_trade_sdk::{common::{Cluster, PriorityFee}, SolanaTrade};

let payer = Keypair::new();
// Define cluster configuration
let cluster = Cluster {
    rpc_url: "https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY".to_string(),
    commitment: CommitmentConfig::confirmed(),
    priority_fee: PriorityFee::default(),
    use_jito: false,
    use_zeroslot: false,
    use_nozomi: false,
    use_nextblock: false,
    block_engine_url: "".to_string(),
    zeroslot_url: "".to_string(),
    zeroslot_auth_token: "".to_string(),
    nozomi_url: "".to_string(),
    nozomi_auth_token: "".to_string(),
    nextblock_url: "".to_string(),
    nextblock_auth_token: "".to_string(),
    lookup_table_key: None,
    use_rpc: true,
};

let solana_trade_client = SolanaTrade::new(Arc::new(payer), &cluster).await;
let creator = Pubkey::from_str("11111111111111111111111111111111")?; // dev account
let buy_sol_cost = 500_000; // 0.0005 SOL
let slippage_basis_points = Some(100);
let rpc = RpcClient::new(cluster.rpc_url);
let recent_blockhash = rpc.get_latest_blockhash().unwrap();
let trade_platform = "pumpswap".to_string();
let mint_pubkey = Pubkey::from_str("YOUR_TOKEN_MINT")?; // token mint

println!("Buying tokens from PumpSwap...");
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

// Sell 30% * amount_token quantity
solana_trade_client
    .sell_by_percent(
        mint_pubkey,
        creator,
        30,      // percentage (30%)
        100,     // total token amount
        recent_blockhash,
        trade_platform.clone(),
    )
    .await?;
```

### 8. PumpSwap Pool Information

```rust
use solana_sdk::pubkey::Pubkey;

let pool_address = Pubkey::from_str("pool_address")?;

// Get current price from PumpSwap pool
let price = solana_trade_client.get_current_price_with_pumpswap(&pool_address).await?;
println!("PumpSwap pool price: {}", price);

// Get SOL reserves in PumpSwap pool
let sol_reserves = solana_trade_client.get_real_sol_reserves_with_pumpswap(&pool_address).await?;
println!("PumpSwap SOL reserves: {} lamports", sol_reserves);

// Get token balance in PumpSwap pool
let token_balance = solana_trade_client.get_payer_token_balance_with_pumpswap(&pool_address).await?;
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
├── pumpfun/      # PumpFun trading functionality
├── pumpswap/     # PumpSwap trading functionality
├── swqos/        # MEV service clients
├── trading/      # Unified trading engine
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

