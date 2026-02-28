use alloy_primitives::{Address, U256};

use crate::error::{Error, Result};
use crate::models::Pool;

/// Trait for computing swap outputs across different DEX protocols.
///
/// Each protocol has its own pricing formula. Implementors provide
/// the math for calculating how much `token_out` you receive for a
/// given `amount_in` of `token_in`.
pub trait Dex {
    /// Calculates the output amount for a given input amount and pool.
    fn get_amount_out(&self, amount_in: u128, token_in: Address, pool: &Pool) -> Result<u128>;
}

/// Uniswap V2 constant-product pricing: x * y = k.
///
/// Fee: 0.3% (applied as 997/1000 on input).
///
/// Formula:
/// ```text
/// amount_out = (amount_in * 997 * reserve_out) / (reserve_in * 1000 + amount_in * 997)
/// ```
pub struct UniswapV2;

impl Dex for UniswapV2 {
    fn get_amount_out(&self, amount_in: u128, token_in: Address, pool: &Pool) -> Result<u128> {
        if amount_in == 0 {
            return Err(Error::ZeroAmountIn);
        }

        let (reserve_in, reserve_out) = if token_in == pool.token0 {
            (pool.reserve0_or_sqrt_price, pool.reserve1_or_liquidity)
        } else {
            (pool.reserve1_or_liquidity, pool.reserve0_or_sqrt_price)
        };

        if reserve_in == 0 || reserve_out == 0 {
            return Err(Error::InsufficientLiquidity {
                pool: pool.address,
                amount: amount_in,
            });
        }

        // Use U256 arithmetic to prevent overflow on large reserves
        let amount_in = U256::from(amount_in);
        let reserve_in = U256::from(reserve_in);
        let reserve_out = U256::from(reserve_out);

        let amount_in_with_fee = amount_in * U256::from(997);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        let amount_out = numerator / denominator;

        Ok(amount_out.to::<u128>())
    }
}

/// Simplified Uniswap V3 pricing using concentrated liquidity math.
///
/// This is a simplified model that approximates V3 behavior using the
/// pool's current liquidity and sqrt-price. A production implementation
/// would traverse ticks, but this captures the core concept.
///
/// Formula (for token0 → token1):
/// ```text
/// new_sqrt_price = sqrt_price + amount_in * 2^96 / liquidity
/// amount_out = liquidity * (new_sqrt - old_sqrt) / Q96
/// ```
pub struct UniswapV3;

impl Dex for UniswapV3 {
    fn get_amount_out(&self, amount_in: u128, token_in: Address, pool: &Pool) -> Result<u128> {
        if amount_in == 0 {
            return Err(Error::ZeroAmountIn);
        }

        let sqrt_price_x96 = pool.reserve0_or_sqrt_price;
        let liquidity = pool.reserve1_or_liquidity;

        if liquidity == 0 || sqrt_price_x96 == 0 {
            return Err(Error::InsufficientLiquidity {
                pool: pool.address,
                amount: amount_in,
            });
        }

        // Apply fee (e.g., 500 bps = 0.05%, 3000 bps = 0.30%)
        let fee_complement = 1_000_000u128 - pool.fee_bps as u128 * 100;
        let effective_amount_in = amount_in * fee_complement / 1_000_000;

        let q96 = U256::from(1u128) << 96;
        let sqrt_price = U256::from(sqrt_price_x96);
        let liq = U256::from(liquidity);
        let amt = U256::from(effective_amount_in);

        let amount_out: U256 = if token_in == pool.token0 {
            // token0 → token1: price increases
            let new_sqrt_price = sqrt_price + (amt * q96 / liq);
            // delta_y = L * (new_sqrt - old_sqrt) / Q96
            liq * (new_sqrt_price - sqrt_price) / q96
        } else {
            // token1 → token0: price decreases
            let new_sqrt_price = sqrt_price - (amt * q96 / liq);
            // delta_x = L * Q96 * (old_sqrt - new_sqrt) / (old_sqrt * new_sqrt)
            liq * q96 * (sqrt_price - new_sqrt_price) / (sqrt_price * new_sqrt_price)
        };

        Ok(amount_out.to::<u128>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Exchange, Pool};
    use alloy_primitives::address;

    fn weth() -> Address {
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
    }

    fn usdc() -> Address {
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
    }

    fn sample_v2_pool() -> Pool {
        Pool::new_v2(
            address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            weth(),
            usdc(),
            Exchange::Uniswap,
            50_000 * 10u128.pow(18),     // 50k WETH
            100_000_000 * 10u128.pow(6), // 100M USDC
        )
    }

    #[test]
    fn v2_basic_swap() {
        let pool = sample_v2_pool();
        let amount_in = 10u128.pow(18); // 1 WETH

        let out = UniswapV2.get_amount_out(amount_in, weth(), &pool).unwrap();

        // ~1994 USDC for 1 WETH (with 0.3% fee and slippage)
        assert!(out > 1_990 * 10u128.pow(6));
        assert!(out < 2_000 * 10u128.pow(6));
    }

    #[test]
    fn v2_zero_input_errors() {
        let pool = sample_v2_pool();
        assert!(UniswapV2.get_amount_out(0, weth(), &pool).is_err());
    }

    #[test]
    fn v2_swap_both_directions() {
        let pool = sample_v2_pool();
        let weth_in = 10u128.pow(18);

        let usdc_out = UniswapV2.get_amount_out(weth_in, weth(), &pool).unwrap();
        let weth_back = UniswapV2.get_amount_out(usdc_out, usdc(), &pool).unwrap();

        // Round-trip should lose ~0.6% (two 0.3% fees)
        assert!(weth_back < weth_in);
        assert!(weth_back > weth_in * 99 / 100);
    }
}
