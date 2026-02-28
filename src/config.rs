use clap::Parser;

/// Helix — a cyclic DEX arbitrage detector that simulates swaps against
/// forked mainnet state via REVM.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Config {
    /// Ethereum RPC URL for REVM-based simulation (e.g. https://eth.llamarpc.com).
    #[arg(long)]
    pub rpc_url: String,

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
