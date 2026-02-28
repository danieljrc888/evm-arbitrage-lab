use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

/// Supported DEX protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    UniswapV2,
    UniswapV3,
}

/// Supported exchanges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Exchange {
    Uniswap,
    Sushiswap,
    Pancakeswap,
}

/// A liquidity pool that can be either Uniswap V2 or V3.
///
/// V2 pools use a constant-product formula (x * y = k) with reserves.
/// V3 pools use concentrated liquidity with sqrt-price and tick data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub protocol: Protocol,
    pub exchange: Exchange,
    /// V2: reserve of token0. V3: current sqrt price (as Q64.96 scaled to u128).
    pub reserve0_or_sqrt_price: u128,
    /// V2: reserve of token1. V3: current liquidity.
    pub reserve1_or_liquidity: u128,
    /// V3 fee tier in basis points (e.g. 500 = 0.05%). V2 uses a fixed 0.3% fee.
    pub fee_bps: u32,
}

impl Pool {
    /// Creates a Uniswap V2 pool with reserves.
    pub fn new_v2(
        address: Address,
        token0: Address,
        token1: Address,
        exchange: Exchange,
        reserve0: u128,
        reserve1: u128,
    ) -> Self {
        Self {
            address,
            token0,
            token1,
            protocol: Protocol::UniswapV2,
            exchange,
            reserve0_or_sqrt_price: reserve0,
            reserve1_or_liquidity: reserve1,
            fee_bps: 30, // 0.3%
        }
    }

    /// Creates a Uniswap V3 pool with sqrt price and liquidity.
    #[allow(dead_code)]
    pub fn new_v3(
        address: Address,
        token0: Address,
        token1: Address,
        exchange: Exchange,
        sqrt_price_x96: u128,
        liquidity: u128,
        fee_bps: u32,
    ) -> Self {
        Self {
            address,
            token0,
            token1,
            protocol: Protocol::UniswapV3,
            exchange,
            reserve0_or_sqrt_price: sqrt_price_x96,
            reserve1_or_liquidity: liquidity,
            fee_bps,
        }
    }

    /// Returns the token addresses as a tuple (token0, token1).
    #[allow(dead_code)]
    pub fn tokens(&self) -> (Address, Address) {
        (self.token0, self.token1)
    }
}

impl std::fmt::Display for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let proto = match self.protocol {
            Protocol::UniswapV2 => "V2",
            Protocol::UniswapV3 => "V3",
        };
        write!(
            f,
            "{:?}-{proto}({})",
            self.exchange,
            &self.address.to_string()[..10]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    fn weth() -> Address {
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
    }

    fn usdc() -> Address {
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
    }

    #[test]
    fn new_v2_sets_correct_fields() {
        let addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
        let pool = Pool::new_v2(addr, weth(), usdc(), Exchange::Uniswap, 1000, 2000);

        assert_eq!(pool.address, addr);
        assert_eq!(pool.token0, weth());
        assert_eq!(pool.token1, usdc());
        assert_eq!(pool.protocol, Protocol::UniswapV2);
        assert_eq!(pool.exchange, Exchange::Uniswap);
        assert_eq!(pool.reserve0_or_sqrt_price, 1000);
        assert_eq!(pool.reserve1_or_liquidity, 2000);
        assert_eq!(pool.fee_bps, 30);
    }

    #[test]
    fn new_v3_sets_correct_fields() {
        let addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
        let pool = Pool::new_v3(
            addr,
            weth(),
            usdc(),
            Exchange::Pancakeswap,
            12345,
            67890,
            500,
        );

        assert_eq!(pool.protocol, Protocol::UniswapV3);
        assert_eq!(pool.exchange, Exchange::Pancakeswap);
        assert_eq!(pool.reserve0_or_sqrt_price, 12345);
        assert_eq!(pool.reserve1_or_liquidity, 67890);
        assert_eq!(pool.fee_bps, 500);
    }

    #[test]
    fn tokens_returns_pair() {
        let addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
        let pool = Pool::new_v2(addr, weth(), usdc(), Exchange::Uniswap, 1000, 2000);
        assert_eq!(pool.tokens(), (weth(), usdc()));
    }

    #[test]
    fn display_includes_exchange_and_protocol() {
        let addr = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
        let pool = Pool::new_v2(addr, weth(), usdc(), Exchange::Sushiswap, 1000, 2000);
        let display = pool.to_string();
        assert!(display.contains("Sushiswap"));
        assert!(display.contains("V2"));
    }
}
