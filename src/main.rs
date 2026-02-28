use std::path::Path;

use alloy_primitives::address;
use clap::Parser;
use colored::Colorize;
use tracing::info;

use helix::config::Config;
use helix::error;
use helix::models::{Exchange, Pool, Swap, SwapPath};
use helix::strategy::{ArbitrageDetector, MathSimulator, Opportunity};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("helix=info".parse().unwrap()),
        )
        .init();

    let config = Config::parse();

    // Decide mode based on --rpc-url flag
    let opportunities = if let Some(rpc_url) = &config.rpc_url {
        run_revm_mode(rpc_url, &config).await
    } else {
        run_math_mode(&config)
    };

    print_results(&opportunities);
}

/// Math-based mode: uses built-in constant-product formula with sample data.
fn run_math_mode(config: &Config) -> Vec<Opportunity> {
    let pools = match &config.pools_file {
        Some(path) => load_pools_from_file(path).expect("failed to load pools file"),
        None => sample_pools(),
    };

    let paths = build_swap_paths(&pools);

    info!(
        pools = pools.len(),
        paths = paths.len(),
        mode = "math",
        "starting arbitrage scan"
    );

    let simulator = MathSimulator::new(pools);
    let detector = ArbitrageDetector::new(simulator, config.base_fee_gwei);
    detector.scan(&paths, config.min_profit_wei)
}

/// REVM mode: forks mainnet state and simulates swaps through actual contracts.
async fn run_revm_mode(rpc_url: &str, config: &Config) -> Vec<Opportunity> {
    use alloy::eips::BlockId;
    use alloy::providers::ProviderBuilder;
    use helix::evm;

    info!(rpc_url, mode = "revm", "connecting to RPC");

    let provider = ProviderBuilder::new().connect_http(rpc_url.parse().expect("invalid RPC URL"));

    // Spawn the shared backend — no accounts populated on startup.
    // State is fetched lazily from RPC on first access and cached.
    let backend = evm::SharedBackend::spawn(provider, BlockId::latest());
    let simulator = evm::EvmSimulator::new(backend);

    // Build paths using real mainnet pool addresses
    let paths = mainnet_swap_paths();

    info!(paths = paths.len(), "scanning mainnet pools via REVM");

    let detector = ArbitrageDetector::new(simulator, config.base_fee_gwei);
    detector.scan(&paths, config.min_profit_wei)
}

fn print_results(opportunities: &[Opportunity]) {
    let separator = "========================================".bold();
    println!("\n{separator}");
    println!("{}", "  Helix — Arbitrage Results".bold());
    println!("{separator}\n");

    if opportunities.is_empty() {
        println!("No profitable arbitrage opportunities found.");
    } else {
        println!(
            "Found {} opportunity(ies):\n",
            opportunities.len().to_string().yellow()
        );

        for (i, opp) in opportunities.iter().enumerate() {
            println!("{}", format!("--- Opportunity #{} ---", i + 1).bold());
            println!("  Path:       {}", opp.path_display.cyan());
            println!("  Hops:       {}", opp.hops);
            println!(
                "  Input:      {} ETH",
                format!("{:.6}", opp.optimal_amount_in as f64 / 1e18).yellow()
            );
            println!(
                "  Output:     {} ETH",
                format!("{:.6}", opp.amount_out as f64 / 1e18).yellow()
            );
            println!(
                "  Gross:      {} ETH",
                format!("{:.6}", opp.gross_profit as f64 / 1e18).yellow()
            );
            println!(
                "  Gas cost:   {} ETH",
                format!("{:.6}", opp.estimated_gas_cost as f64 / 1e18).yellow()
            );
            println!(
                "  Net profit: {} ETH",
                format!("{:.6}", opp.net_profit as f64 / 1e18).yellow()
            );
            println!(
                "  Status:     {}",
                if opp.is_profitable() {
                    "PROFITABLE".green().bold()
                } else {
                    "unprofitable (gas > gross)".red().normal()
                }
            );
            println!();
        }
    }

    println!("{separator}\n");
}

/// Loads pool data from a JSON file.
fn load_pools_from_file(path: impl AsRef<Path>) -> error::Result<Vec<Pool>> {
    let data = std::fs::read_to_string(path)?;
    let pools: Vec<Pool> = serde_json::from_str(&data)?;
    Ok(pools)
}

// ---------------------------------------------------------------------------
// Mainnet swap paths for REVM mode — uses real deployed contract addresses
// ---------------------------------------------------------------------------

/// Real Uniswap V2 pair addresses on Ethereum mainnet.
fn mainnet_swap_paths() -> Vec<SwapPath> {
    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let usdt = address!("dAC17F958D2ee523a2206206994597C13D831ec7");

    let mut paths = Vec::new();

    // WETH/USDC on Uniswap V2 -> USDC/WETH on Sushiswap V2
    let uni_weth_usdc = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
    let sushi_weth_usdc = address!("397FF1542f962076d0BFE58eA045FfA2d347ACa0");
    if let Ok(p) = SwapPath::new(
        "mainnet-weth-usdc-cross",
        vec![
            Swap {
                pool_address: uni_weth_usdc,
                token_in: weth,
                token_out: usdc,
            },
            Swap {
                pool_address: sushi_weth_usdc,
                token_in: usdc,
                token_out: weth,
            },
        ],
    ) {
        paths.push(p);
    }

    // WETH/USDT on Uniswap V2 -> USDT/WETH on Sushiswap V2
    let uni_weth_usdt = address!("0d4a11d5EEaaC28EC3F61d100daF4d40471f1852");
    let sushi_weth_usdt = address!("06da0fd433C1A5d7a4faa01111c044910A184553");
    if let Ok(p) = SwapPath::new(
        "mainnet-weth-usdt-cross",
        vec![
            Swap {
                pool_address: uni_weth_usdt,
                token_in: weth,
                token_out: usdt,
            },
            Swap {
                pool_address: sushi_weth_usdt,
                token_in: usdt,
                token_out: weth,
            },
        ],
    ) {
        paths.push(p);
    }

    paths
}

// ---------------------------------------------------------------------------
// Sample data — simulates a realistic set of DEX pools with price dislocations
// ---------------------------------------------------------------------------

/// Well-known token addresses on Ethereum mainnet.
mod tokens {
    use alloy_primitives::{address, Address};

    pub const WETH: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    pub const DAI: Address = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
    pub const USDC: Address = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    pub const USDT: Address = address!("dAC17F958D2ee523a2206206994597C13D831ec7");
    pub const WBTC: Address = address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599");
}

/// Creates a set of sample pools that mimic real DEX state.
fn sample_pools() -> Vec<Pool> {
    vec![
        Pool::new_v2(
            address!("A478c2975Ab1Ea89e8196811F51A7B7Ade33eB11"),
            tokens::WETH,
            tokens::DAI,
            Exchange::Uniswap,
            5_000 * 10u128.pow(18),
            10_000_000 * 10u128.pow(18),
        ),
        Pool::new_v2(
            address!("C3D03e4F041Fd4cD388c549Ee2A29a9E5075882f"),
            tokens::WETH,
            tokens::DAI,
            Exchange::Sushiswap,
            4_800 * 10u128.pow(18),
            9_800_000 * 10u128.pow(18),
        ),
        Pool::new_v2(
            address!("AE461cA67B15dc8dc81CE7615e0320dA1A9aB8D5"),
            tokens::DAI,
            tokens::USDC,
            Exchange::Uniswap,
            50_000_000 * 10u128.pow(18),
            50_000_000 * 10u128.pow(6),
        ),
        Pool::new_v2(
            address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            tokens::USDC,
            tokens::WETH,
            Exchange::Uniswap,
            9_500_000 * 10u128.pow(6),
            5_000 * 10u128.pow(18),
        ),
        Pool::new_v2(
            address!("0d4a11d5EEaaC28EC3F61d100daF4d40471f1852"),
            tokens::WETH,
            tokens::USDT,
            Exchange::Uniswap,
            3_000 * 10u128.pow(18),
            6_000_000 * 10u128.pow(6),
        ),
        Pool::new_v2(
            address!("06da0fd433C1A5d7a4faa01111c044910A184553"),
            tokens::USDT,
            tokens::WETH,
            Exchange::Sushiswap,
            5_800_000 * 10u128.pow(6),
            3_100 * 10u128.pow(18),
        ),
        Pool::new_v2(
            address!("BB2b8038a1640196FbE3e38816F3e67Cba72D940"),
            tokens::WBTC,
            tokens::WETH,
            Exchange::Uniswap,
            200 * 10u128.pow(8),
            3_000 * 10u128.pow(18),
        ),
        Pool::new_v2(
            address!("CEfF51756c56CeFFCA006cD410B03FFC46dd3a58"),
            tokens::WBTC,
            tokens::WETH,
            Exchange::Sushiswap,
            180 * 10u128.pow(8),
            2_800 * 10u128.pow(18),
        ),
    ]
}

/// Builds swap paths from the available pools (math mode).
fn build_swap_paths(pools: &[Pool]) -> Vec<SwapPath> {
    let mut paths = Vec::new();

    if pools.len() >= 4 {
        if let Ok(p) = SwapPath::new(
            "tri-weth-dai-usdc-weth",
            vec![
                Swap {
                    pool_address: pools[0].address,
                    token_in: tokens::WETH,
                    token_out: tokens::DAI,
                },
                Swap {
                    pool_address: pools[2].address,
                    token_in: tokens::DAI,
                    token_out: tokens::USDC,
                },
                Swap {
                    pool_address: pools[3].address,
                    token_in: tokens::USDC,
                    token_out: tokens::WETH,
                },
            ],
        ) {
            paths.push(p);
        }

        if let Ok(p) = SwapPath::new(
            "tri-weth-dai-sushi-usdc-weth",
            vec![
                Swap {
                    pool_address: pools[1].address,
                    token_in: tokens::WETH,
                    token_out: tokens::DAI,
                },
                Swap {
                    pool_address: pools[2].address,
                    token_in: tokens::DAI,
                    token_out: tokens::USDC,
                },
                Swap {
                    pool_address: pools[3].address,
                    token_in: tokens::USDC,
                    token_out: tokens::WETH,
                },
            ],
        ) {
            paths.push(p);
        }
    }

    if pools.len() >= 6 {
        if let Ok(p) = SwapPath::new(
            "cross-dex-weth-usdt",
            vec![
                Swap {
                    pool_address: pools[4].address,
                    token_in: tokens::WETH,
                    token_out: tokens::USDT,
                },
                Swap {
                    pool_address: pools[5].address,
                    token_in: tokens::USDT,
                    token_out: tokens::WETH,
                },
            ],
        ) {
            paths.push(p);
        }
    }

    if pools.len() >= 8 {
        if let Ok(p) = SwapPath::new(
            "cross-dex-weth-wbtc",
            vec![
                Swap {
                    pool_address: pools[6].address,
                    token_in: tokens::WETH,
                    token_out: tokens::WBTC,
                },
                Swap {
                    pool_address: pools[7].address,
                    token_in: tokens::WBTC,
                    token_out: tokens::WETH,
                },
            ],
        ) {
            paths.push(p);
        }
    }

    paths
}
