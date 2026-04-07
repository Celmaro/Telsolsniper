# Solana Trading SDK - Python

Production-grade instruction builders for various Solana DEX protocols.

## Supported Protocols

- **PumpFun** - Bonding curve trading
- **PumpSwap** - Pump AMM trading
- **Bonk** - Bonk DEX trading
- **Raydium CPMM** - Raydium Concentrated Product Market Maker
- **Raydium AMM V4** - Raydium Automated Market Maker V4
- **Meteora DAMM V2** - Meteora Dynamic AMM V2

## Installation

```bash
pip install sol-trade-sdk
```

## Quick Start

```python
from solders.pubkey import Pubkey
from sol_trade_sdk.instruction import (
    PumpFunParams,
    build_pumpfun_buy_instructions,
    get_bonding_curve_pda,
)

# Set up parameters
payer = Pubkey.from_string("YOUR_WALLET_ADDRESS")
output_mint = Pubkey.from_string("TOKEN_MINT_ADDRESS")

# Build parameters
params = PumpFunParams(
    virtual_token_reserves=1000000000000,
    virtual_sol_reserves=30000000000,
    real_token_reserves=793000000000000,
    creator_vault=Pubkey.from_string("CREATOR_VAULT_ADDRESS"),
    token_program=Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
)

# Build buy instructions
instructions = build_pumpfun_buy_instructions(
    payer=payer,
    output_mint=output_mint,
    input_amount=1000000000,  # 1 SOL
    params=params,
    slippage_bps=500,  # 5%
)
```

## Features

- All program IDs and constants from Rust implementations
- Correct discriminators from Rust (not placeholder values)
- PDA derivation functions using solders.pubkey
- `build_buy_instructions` function for each protocol
- `build_sell_instructions` function for each protocol
- WSOL handling (wrap/close)
- ATA creation support
- Slippage calculation

## Development

```bash
# Install dev dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Format code
black src/
isort src/
```

## License

MIT License
