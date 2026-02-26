# sol-trade-sdk v3.5.0

Rust SDK for Solana DEX trading (Pump.fun, PumpSwap, Raydium, Bonk, Meteora, etc.).

## What's Changed

### Performance

- **Executor hot path**: Sample `Instant::now()` only when `log_enabled` or `simulate` (total, build, submit, confirm) to reduce cold-path syscalls.
- **SWQOS**: `execute_parallel` now takes `&[Arc<SwqosClient>]` to avoid cloning the client list on each swap.
- **Constants**: Named constants for instruction/account sizes and HTTP client timeouts; no magic numbers in hot paths.

### Code Quality

- **Protocol params**: Single `validate_protocol_params()` used for both buy and sell; removed duplicate match blocks in `lib.rs`.
- **Comments**: Bilingual (English + 中文) doc and inline comments across execution, executor, perf (hardware_optimizations, syscall_bypass), and swqos/common.
- **Prefetch/safety**: Clearer docs for cache prefetch and `unsafe` usage (valid read-only ref, no concurrent write).

### Documentation

- README (EN/CN): Version references updated to 3.5.0 for path and crates.io usage.

---

## Cargo

**From Git (this release):**
```toml
sol-trade-sdk = { git = "https://github.com/0xfnzero/sol-trade-sdk", tag = "v3.5.0" }
```

**From crates.io:**
```toml
sol-trade-sdk = "3.5.0"
```

**Full Changelog**: https://github.com/0xfnzero/sol-trade-sdk/compare/v3.4.1...v3.5.0
