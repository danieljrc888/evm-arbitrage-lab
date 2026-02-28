# Helix

A cyclic DEX arbitrage detector built with Rust and REVM. Scans Ethereum liquidity pools for MEV (Maximal Extractable Value) arbitrage opportunities with dual-mode simulation.

[![CI](https://github.com/YOUR_USERNAME/helix/actions/workflows/ci.yml/badge.svg)](https://github.com/YOUR_USERNAME/helix/actions/workflows/ci.yml)

## What it does

This tool scans Uniswap V2/V3 and Sushiswap liquidity pools for cyclic arbitrage opportunities — price dislocations between DEXes that can be exploited for profit.

**Dual-mode operation:**

- **Math mode** (default) — uses built-in constant-product / V3 formulas with sample or user-provided pool data
- **REVM mode** (`--rpc-url`) — forks live mainnet state and simulates actual Uniswap V2 contract calls via REVM

**Pipeline:**

```
Load pools → Build swap paths → For each path (in parallel via rayon):
  1. Test with small amount (0.15 ETH)
  2. If profitable → Ternary search for optimal trade size
  3. Estimate gas costs
  4. Report net profit
```

## Architecture

```
src/
├── lib.rs             Public API (re-exports all modules)
├── models/            Domain types
│   ├── token.rs         ERC-20 token representation
│   ├── pool.rs          Liquidity pool (V2 reserves, V3 sqrt-price)
│   └── swap_path.rs     Cyclic swap path with validation
├── pricing/           Swap math
│   ├── dex.rs           Dex trait + UniswapV2/V3 implementations
│   └── optimizer.rs     Ternary search for optimal input amount
├── strategy/          Opportunity detection
│   ├── simulator.rs     SwapSimulator trait
│   ├── math_simulator.rs  MathSimulator (local pricing formulas)
│   └── arbitrage.rs     Generic ArbitrageDetector<S: SwapSimulator>
├── evm/               On-chain simulation via REVM
│   ├── contracts.rs     Uniswap V2 ABI encoding/decoding (sol! macro)
│   ├── simulator.rs     EvmSimulator (impl SwapSimulator)
│   └── shared_backend.rs  Thread-safe lazy RPC backend
├── config.rs          CLI configuration (clap)
├── error.rs           Custom error types (thiserror)
└── main.rs            Entry point with sample data & colored output
```

## Key Rust concepts demonstrated

| Concept | Where |
|---------|-------|
| **Traits & generics** | `SwapSimulator` trait, `ArbitrageDetector<S: SwapSimulator>` |
| **Trait objects & dispatch** | `Dex` trait with `UniswapV2`/`UniswapV3` implementors |
| **Enums with variants** | `Protocol`, `Exchange`, `Error` |
| **Custom error handling** | `thiserror` derive, `Result<T>` alias |
| **Generics & closures** | `TernarySearchOptimizer::find_optimal<F>` |
| **Parallel iteration** | `rayon::par_iter()` in path scanning |
| **Async / tokio** | `SharedBackend` background task, `block_in_place` bridging |
| **EVM simulation** | REVM `Context::mainnet()`, `CacheDB` overlays |
| **Type-safe ABI** | `alloy_sol_types::sol!` macro, `abi_decode_returns` |
| **256-bit arithmetic** | `alloy_primitives::U256` for overflow-safe math |
| **Iterator combinators** | `filter_map`, `min`, `collect` in scan logic |
| **Serde serialization** | All models derive `Serialize`/`Deserialize` |
| **Structured logging** | `tracing` with `info!`/`debug!` |
| **CLI parsing** | `clap` derive API |
| **Unit testing** | 28 tests across 6 modules with `#[cfg(test)]` |
| **Benchmarking** | Criterion benchmarks for hot paths |
| **CI** | GitHub Actions with stable + nightly Rust matrix |

## Uniswap V2 pricing formula

```
amount_out = (amount_in × 997 × reserve_out) / (reserve_in × 1000 + amount_in × 997)
```

The 997/1000 factor represents the 0.3% swap fee.

## Ternary search optimization

Arbitrage profit is a unimodal function of input amount — it rises to a peak then falls as slippage dominates. Ternary search efficiently finds the optimal trade size:

```
  profit
    ▲
    │      ╱╲
    │     ╱  ╲
    │    ╱    ╲        ← search narrows to peak
    │   ╱      ╲
    └──────────────► amount_in
```

## Usage

```bash
# Run with built-in sample data (math mode)
cargo run

# Fork mainnet via REVM
cargo run -- --rpc-url https://eth.llamarpc.com

# Custom base fee (gwei)
cargo run -- --base-fee-gwei 15

# Minimum profit filter
cargo run -- --min-profit-wei 1000000000000000

# Load pool data from JSON
cargo run -- --pools-file data/pools.json

# Verbose mode
cargo run -- --verbose
```

## Example output

```
========================================
  Helix — Arbitrage Results
========================================

Found 4 opportunity(ies):

--- Opportunity #1 ---
  Path:       0xC02aaA39 → 0x6B175474 → 0xA0b86991 → 0xC02aaA39
  Hops:       3
  Input:      29.999604 ETH
  Output:     30.880337 ETH
  Gross:      0.880733 ETH
  Gas cost:   0.006600 ETH
  Net profit: 0.874133 ETH
  Status:     PROFITABLE
```

## Tests

```bash
cargo test
```

```
running 28 tests
test evm::contracts::tests::decode_reserves_rejects_short_response ... ok
test evm::contracts::tests::decode_reserves_with_real_abi_encoded_data ... ok
test evm::contracts::tests::get_reserves_selector ... ok
test evm::contracts::tests::token0_selector ... ok
test models::pool::tests::display_includes_exchange_and_protocol ... ok
test models::pool::tests::new_v2_sets_correct_fields ... ok
test models::pool::tests::new_v3_sets_correct_fields ... ok
test models::pool::tests::tokens_returns_pair ... ok
test models::swap_path::tests::accepts_single_swap_cycle ... ok
test models::swap_path::tests::base_token_returns_first_token_in ... ok
test models::swap_path::tests::display_format_shows_arrows ... ok
test models::swap_path::tests::rejects_empty_swaps ... ok
test models::swap_path::tests::rejects_non_cyclic_path ... ok
test pricing::dex::tests::v2_basic_swap ... ok
test pricing::dex::tests::v2_swap_both_directions ... ok
test pricing::dex::tests::v2_zero_input_errors ... ok
test pricing::optimizer::tests::converges_on_narrow_range ... ok
test pricing::optimizer::tests::finds_peak_of_unimodal_function ... ok
test pricing::optimizer::tests::handles_eval_always_returning_none ... ok
test pricing::optimizer::tests::handles_no_profitable_region ... ok
test pricing::optimizer::tests::returns_zero_when_lo_equals_hi ... ok
test pricing::optimizer::tests::returns_zero_when_lo_greater_than_hi ... ok
test strategy::arbitrage::tests::detects_profitable_triangular_arbitrage ... ok
test strategy::arbitrage::tests::mock_profitable_path_detected ... ok
test strategy::arbitrage::tests::mock_unprofitable_path_filtered ... ok
test strategy::arbitrage::tests::no_opportunity_when_pools_are_balanced ... ok
test strategy::arbitrage::tests::opportunity_display_includes_status ... ok
test strategy::arbitrage::tests::respects_min_profit_threshold ... ok
```

## Benchmarks

```bash
cargo bench
```

Three benchmarks covering the hot path: ternary search optimizer, UniswapV2 `get_amount_out`, and a full scan with 4 pools / 2 paths.

## Limitations

This is a proof of concept. A production MEV bot would additionally need:

- **Mempool monitoring** for frontrunning protection
- **Flashbots/MEV-Share** bundle submission
- **Tick-level V3 math** for concentrated liquidity precision
- **Access list optimization** for gas reduction
- **Multi-block strategies** and cross-domain MEV

## License

MIT
