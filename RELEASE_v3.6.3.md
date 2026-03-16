# Release v3.6.3

## Changes from v3.6.2

### Parallel multi-SWQoS submit

- **Transaction builder pool**: Introduced `PARALLEL_SENDER_COUNT` (18) and ensured pool prefill is at least 18, so that when using dedicated sender threads all channels can acquire a builder without waiting or allocating. Prefill is now 64 with a guaranteed minimum of 18.
- **Async executor**: Documented that builder pool prefill must match the dedicated sender thread count so multi-channel submit never serializes on `build_transaction`.
- **Executor logging**: Submit timings are no longer sorted before printing (avoids any extra work on the hot path). Log order reflects completion order (first-completed first); the last line is the slowest channel in that batch.

### Other

- Added `docs/ASYNC_EXECUTOR_REVIEW.md`.
- Minor updates in types, lib, params, and middleware example.
