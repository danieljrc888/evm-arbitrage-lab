use tracing::debug;

/// Finds the optimal input amount that maximizes profit using ternary search.
///
/// Ternary search works on unimodal functions (single peak), which is the case
/// for arbitrage profit curves: profit increases with amount up to a point,
/// then decreases as slippage dominates.
///
/// ```text
///   profit
///     ▲
///     │      ╱╲
///     │     ╱  ╲
///     │    ╱    ╲
///     │   ╱      ╲
///     │  ╱        ╲
///     │ ╱          ╲
///     └──────────────► amount_in
///           ↑
///       optimal
/// ```
pub struct TernarySearchOptimizer {
    /// Maximum number of iterations before stopping.
    pub max_iterations: u32,
    /// Convergence threshold in wei — stop when the search range is smaller.
    pub convergence_threshold: u128,
}

impl Default for TernarySearchOptimizer {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            convergence_threshold: 10u128.pow(15), // 0.001 ETH
        }
    }
}

impl TernarySearchOptimizer {
    /// Finds the input amount in `[lo, hi]` that maximizes the output of `eval_fn`.
    ///
    /// `eval_fn(amount_in)` should return the output amount for a given input through
    /// the arbitrage path. The profit is `output - input`, and since the search
    /// maximizes `eval_fn(x) - x`, it finds the most profitable trade size.
    ///
    /// Returns `(optimal_amount_in, amount_out)`.
    pub fn find_optimal<F>(&self, lo: u128, hi: u128, eval_fn: F) -> (u128, u128)
    where
        F: Fn(u128) -> Option<u128>,
    {
        if lo >= hi {
            return (0, 0);
        }

        let mut left = lo;
        let mut right = hi;

        let mut best_amount_in = 0u128;
        let mut best_amount_out = 0u128;

        for i in 0..self.max_iterations {
            if right.saturating_sub(left) < self.convergence_threshold {
                debug!(iteration = i, "converged: range < threshold");
                break;
            }

            let mid1 = left + (right - left) / 3;
            let mid2 = right - (right - left) / 3;

            let out1 = eval_fn(mid1);
            let out2 = eval_fn(mid2);

            match (out1, out2) {
                (Some(o1), Some(o2)) => {
                    // Compare profit = amount_out - amount_in
                    let profit1 = o1.saturating_sub(mid1);
                    let profit2 = o2.saturating_sub(mid2);

                    if profit1 > profit2 {
                        right = mid2;
                        if profit1 > best_amount_out.saturating_sub(best_amount_in) {
                            best_amount_in = mid1;
                            best_amount_out = o1;
                        }
                    } else {
                        left = mid1;
                        if profit2 > best_amount_out.saturating_sub(best_amount_in) {
                            best_amount_in = mid2;
                            best_amount_out = o2;
                        }
                    }
                }
                (Some(o1), None) => {
                    right = mid2;
                    let profit1 = o1.saturating_sub(mid1);
                    if profit1 > best_amount_out.saturating_sub(best_amount_in) {
                        best_amount_in = mid1;
                        best_amount_out = o1;
                    }
                }
                (None, Some(o2)) => {
                    left = mid1;
                    let profit2 = o2.saturating_sub(mid2);
                    if profit2 > best_amount_out.saturating_sub(best_amount_in) {
                        best_amount_in = mid2;
                        best_amount_out = o2;
                    }
                }
                (None, None) => break,
            }
        }

        (best_amount_in, best_amount_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_peak_of_unimodal_function() {
        let optimizer = TernarySearchOptimizer {
            max_iterations: 100,
            convergence_threshold: 1,
        };

        // Profit function with peak at x=500: profit = -(x-500)^2 + 250000
        // Meaning amount_out = x + (-(x-500)^2 + 250000)
        let eval = |x: u128| -> Option<u128> {
            let diff = if x > 500 { x - 500 } else { 500 - x };
            let profit = 250_000u128.saturating_sub(diff * diff);
            Some(x + profit)
        };

        let (optimal, _) = optimizer.find_optimal(0, 1000, eval);
        assert!((optimal as i128 - 500).unsigned_abs() < 5);
    }

    #[test]
    fn handles_no_profitable_region() {
        let optimizer = TernarySearchOptimizer::default();

        // Always returns less than input (no profit)
        let eval = |x: u128| -> Option<u128> { Some(x / 2) };

        let (optimal, out) = optimizer.find_optimal(10u128.pow(18), 100 * 10u128.pow(18), eval);
        // No profit: output <= input for every point in the range
        assert!(out <= optimal);
    }

    #[test]
    fn returns_zero_when_lo_equals_hi() {
        let optimizer = TernarySearchOptimizer::default();
        let eval = |x: u128| -> Option<u128> { Some(x * 2) };

        let (amount, out) = optimizer.find_optimal(100, 100, eval);
        assert_eq!(amount, 0);
        assert_eq!(out, 0);
    }

    #[test]
    fn returns_zero_when_lo_greater_than_hi() {
        let optimizer = TernarySearchOptimizer::default();
        let eval = |x: u128| -> Option<u128> { Some(x * 2) };

        let (amount, out) = optimizer.find_optimal(200, 100, eval);
        assert_eq!(amount, 0);
        assert_eq!(out, 0);
    }

    #[test]
    fn handles_eval_always_returning_none() {
        let optimizer = TernarySearchOptimizer::default();
        let eval = |_: u128| -> Option<u128> { None };

        let (amount, out) = optimizer.find_optimal(0, 1000, eval);
        assert_eq!(amount, 0);
        assert_eq!(out, 0);
    }

    #[test]
    fn converges_on_narrow_range() {
        let optimizer = TernarySearchOptimizer {
            max_iterations: 200,
            convergence_threshold: 1,
        };

        // Peak at x=750 in range [700, 800]
        let eval = |x: u128| -> Option<u128> {
            let diff = if x > 750 { x - 750 } else { 750 - x };
            let profit = 10_000u128.saturating_sub(diff * diff);
            Some(x + profit)
        };

        let (optimal, _) = optimizer.find_optimal(700, 800, eval);
        assert!((optimal as i128 - 750).unsigned_abs() < 3);
    }
}
