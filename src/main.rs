use alloy::eips::BlockId;
use alloy::providers::ProviderBuilder;
use alloy_primitives::address;
use clap::Parser;
use colored::Colorize;
use tracing::info;

use helix::config::Config;
use helix::evm;
use helix::models::{Swap, SwapPath};
use helix::strategy::{ArbitrageDetector, Opportunity};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("helix=info".parse().unwrap()),
        )
        .init();

    let config = Config::parse();

    info!(rpc_url = %config.rpc_url, "connecting to RPC");

    let provider =
        ProviderBuilder::new().connect_http(config.rpc_url.parse().expect("invalid RPC URL"));

    let backend = evm::SharedBackend::spawn(provider, BlockId::latest());
    let simulator = evm::EvmSimulator::new(backend);

    let paths = mainnet_swap_paths();

    info!(paths = paths.len(), "scanning mainnet pools via REVM");

    let detector = ArbitrageDetector::new(simulator, config.base_fee_gwei);
    let opportunities = detector.scan(&paths, config.min_profit_wei);

    print_results(&opportunities);
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

// ---------------------------------------------------------------------------
// Mainnet swap paths — uses real deployed contract addresses
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
