use clap::Parser;

/// MEV Arbitrage Opportunity Detector — a proof-of-concept CLI tool
/// that scans Uniswap V2/V3 pools to find cyclic arbitrage opportunities.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Config {
    /// Path to a JSON file with pool data (uses built-in sample data if omitted)
    #[arg(short, long)]
    pub pools_file: Option<String>,

    /// Ethereum RPC URL for REVM-based simulation (e.g. https://eth.llamarpc.com).
    /// When provided, swaps are simulated against forked mainnet state via REVM
    /// instead of using the built-in math engine.
    #[arg(long)]
    pub rpc_url: Option<String>,

    /// Minimum profit in wei to report an opportunity
    #[arg(short, long, default_value_t = 0)]
    pub min_profit_wei: u128,

    /// Base fee in gwei used for gas cost estimation
    #[arg(long, default_value_t = 30)]
    pub base_fee_gwei: u64,

    /// Show detailed output for every evaluated path
    #[arg(short, long)]
    pub verbose: bool,
}
