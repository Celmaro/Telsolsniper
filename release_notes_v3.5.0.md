# sol-trade-sdk v3.5.0

Rust SDK for Solana DEX trading (Pump.fun, PumpSwap, Raydium, Bonk, Meteora, etc.).

## What's Changed

### Performance

- **Hot-path timing**: `Instant::now()` for build/submit/total/confirm only when `log_enabled` or (for total) simulate, reducing cold-path syscalls.
- **Fewer clones**: `execute_parallel` now takes `&[Arc<SwqosClient>]` instead of `Vec<Arc<SwqosClient>>`; caller no longer clones the client list.
- **SWQoS HTTP**: Named constants for pool idle timeout, connect/request timeouts, and HTTP/2 keepalive in `swqos/common.rs`.

### Code quality

- **Protocol params**: Single `validate_protocol_params(dex_type, params)` used by both buy and sell; removed duplicated match blocks.
- **Constants**: `BYTES_PER_ACCOUNT`, `MAX_INSTRUCTIONS_WARN` in execution; HTTP timeout constants in swqos common.
- **Comments**: Prefetch and branch-hint safety/usage documented; `SYSCALL_BYPASS` marked as reserved for future use.

### Documentation

- **Bilingual docs**: English + 中文 doc comments in `trading/core/execution.rs`, `trading/core/executor.rs`, `perf/hardware_optimizations.rs`, `perf/mod.rs`, `perf/syscall_bypass.rs`, `swqos/common.rs`.
- **README**: Version references and "What's new in 3.5.0" (EN) / "3.5.0 更新说明" (CN) updated.

---

## Cargo

**From Git (this release):**
```toml
sol-trade-sdk = { git = "https://github.com/0xfnzero/sol-trade-sdk", tag = "v3.5.0" }
```

**From crates.io** (when published):
```toml
sol-trade-sdk = "3.5.0"
```

**Full Changelog**: https://github.com/0xfnzero/sol-trade-sdk/compare/v3.4.1...v3.5.0
