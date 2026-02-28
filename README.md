# Helix

A cyclic DEX arbitrage detector built with Rust and REVM. Forks live Ethereum mainnet state to simulate swap paths across Uniswap V2 and Sushiswap, detecting MEV (Maximal Extractable Value) arbitrage opportunities with bit-exact EVM accuracy.

[![CI](https://github.com/danieljrc888/helix/actions/workflows/ci.yml/badge.svg)](https://github.com/danieljrc888/helix/actions/workflows/ci.yml)

## What it does

This tool forks Ethereum mainnet via an RPC endpoint and simulates actual Uniswap V2 contract calls through REVM to find cyclic arbitrage opportunities — price dislocations between DEXes that can be exploited for profit.

**Pipeline:**

```
Connect to RPC → Fork mainnet state → Build swap paths → For each path (in parallel via rayon):
  1. Simulate swaps through actual contract bytecode via REVM
  2. Test with small amount (0.15 ETH)
  3. If profitable → Ternary search for optimal trade size
  4. Estimate gas costs
  5. Report net profit
```

## Architecture

```
src/
├── lib.rs               Public API (re-exports all modules)
├── models/              Domain types
│   └── swap_path.rs       Cyclic swap path with validation
├── pricing/             Optimization
│   └── optimizer.rs       Ternary search for optimal input amount
├── strategy/            Opportunity detection
│   ├── simulator.rs       SwapSimulator trait
│   └── arbitrage.rs       Generic ArbitrageDetector<S: SwapSimulator>
├── evm/                 On-chain simulation via REVM
│   ├── contracts.rs       Uniswap V2 ABI encoding/decoding (sol! macro)
│   ├── simulator.rs       EvmSimulator (impl SwapSimulator)
│   └── shared_backend.rs  Thread-safe lazy RPC backend
├── config.rs            CLI configuration (clap)
├── error.rs             Custom error types (thiserror)
└── main.rs              Entry point & mainnet swap paths
```

## Key Rust concepts demonstrated

| Concept | Where |
|---------|-------|
| **Traits & generics** | `SwapSimulator` trait, `ArbitrageDetector<S: SwapSimulator>` |
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
| **Unit testing** | Tests across modules with `#[cfg(test)]` |
| **Benchmarking** | Criterion benchmarks for hot paths |
| **CI** | GitHub Actions with stable + nightly Rust matrix |

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
# Scan mainnet pools via REVM
cargo run -- --rpc-url https://eth.llamarpc.com

# Custom base fee (gwei)
cargo run -- --rpc-url https://eth.llamarpc.com --base-fee-gwei 15

# Minimum profit filter
cargo run -- --rpc-url https://eth.llamarpc.com --min-profit-wei 1000000000000000

# Verbose mode
cargo run -- --rpc-url https://eth.llamarpc.com --verbose
```

## Example output

```
========================================
  Helix — Arbitrage Results
========================================

Found 2 opportunity(ies):

--- Opportunity #1 ---
  Path:       0xC02aaA39 → 0xA0b86991 → 0xC02aaA39
  Hops:       2
  Input:      5.234100 ETH
  Output:     5.241823 ETH
  Gross:      0.007723 ETH
  Gas cost:   0.004500 ETH
  Net profit: 0.003223 ETH
  Status:     PROFITABLE
```

## Tests

```bash
cargo test
```

## Benchmarks

```bash
cargo bench
```

## Disclaimer

This project is built for **educational and research purposes only**. It demonstrates Rust systems programming, EVM simulation, and DeFi mechanics — it is not a production MEV bot and cannot be used as one.

A production MEV system would additionally require an on-chain execution contract for atomic swaps, private transaction submission via Flashbots/MEV-Share, real-time mempool monitoring, sub-second latency infrastructure, and capital or flashloan integration — none of which are present here.

## Limitations

- **Mempool monitoring** for frontrunning protection
- **Flashbots/MEV-Share** bundle submission
- **V3 concentrated liquidity** support with tick traversal
- **Access list optimization** for gas reduction
- **Multi-block strategies** and cross-domain MEV
- **On-chain execution contract** for atomic arbitrage
- **Dynamic pool discovery** (currently uses hardcoded paths)

## License

MIT
