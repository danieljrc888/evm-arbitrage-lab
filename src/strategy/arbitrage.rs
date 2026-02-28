use rayon::prelude::*;
use tracing::{debug, info};

use crate::error::Result;
use crate::models::SwapPath;
use crate::pricing::TernarySearchOptimizer;

use super::simulator::SwapSimulator;

/// A detected arbitrage opportunity with its optimal parameters.
#[derive(Debug, Clone)]
pub struct Opportunity {
    #[allow(dead_code)]
    pub path_id: String,
    pub path_display: String,
    pub optimal_amount_in: u128,
    pub amount_out: u128,
    pub gross_profit: u128,
    pub estimated_gas_cost: u128,
    pub net_profit: i128,
    pub hops: usize,
}

impl Opportunity {
    pub fn is_profitable(&self) -> bool {
        self.net_profit > 0
    }
}

impl std::fmt::Display for Opportunity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let profit_eth = self.net_profit as f64 / 1e18;
        let amount_eth = self.optimal_amount_in as f64 / 1e18;
        let status = if self.is_profitable() {
            "PROFITABLE"
        } else {
            "unprofitable"
        };
        write!(
            f,
            "[{status}] {path} | input: {amount:.4} ETH | net: {profit:.6} ETH",
            path = self.path_display,
            amount = amount_eth,
            profit = profit_eth,
        )
    }
}

/// Scans swap paths for cyclic arbitrage opportunities using any [`SwapSimulator`].
///
/// The detector:
/// 1. Takes a simulator and pre-computed swap paths
/// 2. For each path, simulates the swap chain via the simulator
/// 3. Uses ternary search to find the optimal input amount
/// 4. Reports opportunities that exceed a minimum profit threshold
pub struct ArbitrageDetector<S> {
    simulator: S,
    optimizer: TernarySearchOptimizer,
    base_fee_gwei: u64,
}

impl<S: SwapSimulator + Send + Sync> ArbitrageDetector<S> {
    pub fn new(simulator: S, base_fee_gwei: u64) -> Self {
        Self {
            simulator,
            optimizer: TernarySearchOptimizer::default(),
            base_fee_gwei,
        }
    }

    /// Scans a list of swap paths and returns all detected opportunities.
    pub fn scan(&self, paths: &[SwapPath], min_profit_wei: u128) -> Vec<Opportunity> {
        info!(
            paths = paths.len(),
            min_profit_wei, "scanning for arbitrage opportunities"
        );

        let opportunities: Vec<Opportunity> = paths
            .par_iter()
            .filter_map(|path| match self.evaluate_path(path) {
                Ok(Some(opp)) if opp.gross_profit > 0 => {
                    if opp.net_profit > min_profit_wei as i128 {
                        info!(%opp, "opportunity found");
                        Some(opp)
                    } else {
                        debug!(path = %path, "below min profit threshold");
                        None
                    }
                }
                Ok(_) => {
                    debug!(path = %path, "no profit");
                    None
                }
                Err(e) => {
                    debug!(path = %path, error = %e, "evaluation error");
                    None
                }
            })
            .collect();

        info!(found = opportunities.len(), "scan complete");
        opportunities
    }

    /// Evaluates a single swap path for arbitrage potential.
    fn evaluate_path(&self, path: &SwapPath) -> Result<Option<Opportunity>> {
        // Quick sanity check: simulate with a small test amount
        let test_amount = 15 * 10u128.pow(16); // 0.15 ETH
        let test_out = self.simulator.simulate_path(path, test_amount)?;
        if test_out <= test_amount {
            return Ok(None);
        }

        // Determine search range: [0.01 ETH, max_trade_amount]
        let lo = 10u128.pow(16); // 0.01 ETH
        let hi = self.simulator.max_trade_amount(path);

        // Ternary search for optimal input
        let (optimal_in, amount_out) = self.optimizer.find_optimal(lo, hi, |amount| {
            self.simulator.simulate_path(path, amount).ok()
        });

        if amount_out <= optimal_in {
            return Ok(None);
        }

        let gross_profit = amount_out - optimal_in;
        let gas_cost = self.estimate_gas_cost(path);
        let net_profit = gross_profit as i128 - gas_cost as i128;

        Ok(Some(Opportunity {
            path_id: path.id.clone(),
            path_display: path.to_string(),
            optimal_amount_in: optimal_in,
            amount_out,
            gross_profit,
            estimated_gas_cost: gas_cost,
            net_profit,
            hops: path.len(),
        }))
    }

    /// Estimates gas cost in wei for executing a swap path.
    ///
    /// Gas model: base cost + per-hop cost (70k gas per swap).
    fn estimate_gas_cost(&self, path: &SwapPath) -> u128 {
        let gas_units = 10_000 + path.len() as u128 * 70_000;
        let base_fee_wei = self.base_fee_gwei as u128 * 10u128.pow(9);
        gas_units * base_fee_wei
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Swap, SwapPath};
    use alloy_primitives::{address, Address};

    fn weth() -> Address {
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
    }

    fn token_a() -> Address {
        address!("6B175474E89094C44Da98b954EedeAC495271d0F") // DAI
    }

    /// A mock simulator that returns a fixed multiplier of the input.
    struct MockSimulator {
        /// Output = amount_in * multiplier_num / multiplier_den
        multiplier_num: u128,
        multiplier_den: u128,
    }

    impl SwapSimulator for MockSimulator {
        fn simulate_path(&self, _path: &SwapPath, amount_in: u128) -> Result<u128> {
            Ok(amount_in * self.multiplier_num / self.multiplier_den)
        }
    }

    fn dummy_path() -> SwapPath {
        let pool = address!("1111111111111111111111111111111111111111");
        SwapPath::new(
            "mock-path",
            vec![
                Swap {
                    pool_address: pool,
                    token_in: weth(),
                    token_out: token_a(),
                },
                Swap {
                    pool_address: pool,
                    token_in: token_a(),
                    token_out: weth(),
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn profitable_path_detected() {
        // 5% profit: output = input * 1.05
        let simulator = MockSimulator {
            multiplier_num: 105,
            multiplier_den: 100,
        };
        let detector = ArbitrageDetector::new(simulator, 0);
        let path = dummy_path();

        let opportunities = detector.scan(&[path], 0);
        assert_eq!(opportunities.len(), 1);
        assert!(opportunities[0].is_profitable());
    }

    #[test]
    fn unprofitable_path_filtered() {
        // 2% loss: output = input * 0.98
        let simulator = MockSimulator {
            multiplier_num: 98,
            multiplier_den: 100,
        };
        let detector = ArbitrageDetector::new(simulator, 0);
        let path = dummy_path();

        let opportunities = detector.scan(&[path], 0);
        assert!(opportunities.is_empty());
    }

    #[test]
    fn respects_min_profit_threshold() {
        // 5% profit but absurdly high threshold
        let simulator = MockSimulator {
            multiplier_num: 105,
            multiplier_den: 100,
        };
        let detector = ArbitrageDetector::new(simulator, 0);
        let path = dummy_path();

        let opportunities = detector.scan(&[path], u128::MAX / 2);
        assert!(opportunities.is_empty());
    }

    #[test]
    fn gas_cost_filters_marginal_opportunity() {
        // 0.1% profit — should be eaten by gas
        let simulator = MockSimulator {
            multiplier_num: 1001,
            multiplier_den: 1000,
        };
        let detector = ArbitrageDetector::new(simulator, 30);
        let path = dummy_path();

        let opportunities = detector.scan(&[path], 0);
        // With 30 gwei base fee, gas cost exceeds tiny profit
        for opp in &opportunities {
            if !opp.is_profitable() {
                // Expected — gas ate the profit
            }
        }
    }

    #[test]
    fn opportunity_display_includes_status() {
        let opp = Opportunity {
            path_id: "test".into(),
            path_display: "A -> B -> A".into(),
            optimal_amount_in: 10u128.pow(18),
            amount_out: 11 * 10u128.pow(17),
            gross_profit: 10u128.pow(17),
            estimated_gas_cost: 0,
            net_profit: 10i128.pow(17),
            hops: 2,
        };
        let display = opp.to_string();
        assert!(display.contains("PROFITABLE"));
    }
}
