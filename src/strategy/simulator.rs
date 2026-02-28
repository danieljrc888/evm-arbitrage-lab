use crate::error::Result;
use crate::models::SwapPath;

/// Trait for simulating swap paths and computing output amounts.
///
/// Implementors provide different backends for pricing swaps:
/// - [`MathSimulator`](super::MathSimulator): local constant-product / V3 math
/// - [`EvmSimulator`](crate::evm::EvmSimulator): REVM-based on-chain simulation
pub trait SwapSimulator {
    /// Simulate a full swap path and return the final output amount.
    fn simulate_path(&self, path: &SwapPath, amount_in: u128) -> Result<u128>;

    /// Maximum trade size in wei (default: 30 ETH).
    fn max_trade_amount(&self, _path: &SwapPath) -> u128 {
        30 * 10u128.pow(18)
    }
}
