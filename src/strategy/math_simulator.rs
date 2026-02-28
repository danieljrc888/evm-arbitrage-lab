use std::collections::HashMap;

use alloy_primitives::Address;

use crate::error::{Error, Result};
use crate::models::{Pool, Protocol, SwapPath};
use crate::pricing::{Dex, UniswapV2, UniswapV3};

use super::simulator::SwapSimulator;

/// Math-based swap simulator using local constant-product / V3 formulas.
pub struct MathSimulator {
    pools: HashMap<Address, Pool>,
}

impl MathSimulator {
    pub fn new(pools: Vec<Pool>) -> Self {
        let pool_map = pools.into_iter().map(|p| (p.address, p)).collect();
        Self { pools: pool_map }
    }

    /// Routes a swap to the correct pricing engine based on pool protocol.
    fn price_swap(&self, amount_in: u128, token_in: Address, pool: &Pool) -> Result<u128> {
        match pool.protocol {
            Protocol::UniswapV2 => UniswapV2.get_amount_out(amount_in, token_in, pool),
            Protocol::UniswapV3 => UniswapV3.get_amount_out(amount_in, token_in, pool),
        }
    }

    /// Returns a reference to the pool map (used by ArbitrageDetector for gas estimation).
    pub fn pools(&self) -> &HashMap<Address, Pool> {
        &self.pools
    }
}

impl SwapSimulator for MathSimulator {
    fn simulate_path(&self, path: &SwapPath, amount_in: u128) -> Result<u128> {
        let mut current_amount = amount_in;

        for swap in &path.swaps {
            let pool = self
                .pools
                .get(&swap.pool_address)
                .ok_or(Error::PoolNotFound(swap.pool_address))?;

            current_amount = self.price_swap(current_amount, swap.token_in, pool)?;
        }

        Ok(current_amount)
    }

    /// Uses 75% of the first pool's input reserve, capped at 30 ETH.
    fn max_trade_amount(&self, path: &SwapPath) -> u128 {
        let first_swap = &path.swaps[0];
        let max_from_reserve = self
            .pools
            .get(&first_swap.pool_address)
            .and_then(|pool| match pool.protocol {
                Protocol::UniswapV2 => {
                    let reserve = if first_swap.token_in == pool.token0 {
                        pool.reserve0_or_sqrt_price
                    } else {
                        pool.reserve1_or_liquidity
                    };
                    Some(reserve * 3 / 4)
                }
                Protocol::UniswapV3 => None,
            })
            .unwrap_or(30 * 10u128.pow(18));

        max_from_reserve.min(30 * 10u128.pow(18))
    }
}
