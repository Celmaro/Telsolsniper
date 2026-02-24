# sol-trade-sdk v3.4.1

Rust SDK for Solana DEX trading (Pump.fun, PumpSwap, Raydium, Bonk, Meteora, etc.).

## What's Changed

### New Features

- **PumpFun & PumpSwap Cashback** (#77): Support for cashback in PumpFun and PumpSwap trading flows. See [Cashback documentation](docs/PUMP_CASHBACK_README.md).
- **Events**: `is_cashback_coin` is now passed from events; PumpFun examples use `sol-parser-sdk` only for event parsing.

### Performance

- **SWQoS**: Reduced submit latency; fixed high latency after ~5 minutes idle.

### Bug Fixes

- **WSOL ATA**: WSOL Associated Token Account creation now runs in background with retry and timeout for more reliable setup.
- Silenced unused and deprecated compiler warnings.

### Documentation

- README (EN/中文): Added Cashback section, outline, examples and tables.
- Updated crates.io / docs references in README.

---

## Cargo

**From Git (this release):**
```toml
sol-trade-sdk = { git = "https://github.com/0xfnzero/sol-trade-sdk", tag = "v3.4.1" }
```

**From crates.io** (when published):
```toml
sol-trade-sdk = "3.4.1"
```

**Full Changelog**: https://github.com/0xfnzero/sol-trade-sdk/compare/v3.4.0...v3.4.1
