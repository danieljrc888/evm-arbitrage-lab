use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

/// A single swap step within an arbitrage path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Swap {
    pub pool_address: Address,
    pub token_in: Address,
    pub token_out: Address,
}

/// A cyclic sequence of swaps forming an arbitrage path.
///
/// For a valid arbitrage, the path must start and end with the same token
/// (typically WETH), forming a cycle like: WETH → A → B → WETH.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapPath {
    pub id: String,
    pub swaps: Vec<Swap>,
}

impl SwapPath {
    /// Creates a new swap path and validates it is cyclic.
    pub fn new(id: impl Into<String>, swaps: Vec<Swap>) -> crate::error::Result<Self> {
        if swaps.is_empty() {
            return Err(crate::error::Error::EmptySwapPath);
        }

        let start = swaps.first().unwrap().token_in;
        let end = swaps.last().unwrap().token_out;

        if start != end {
            return Err(crate::error::Error::NonCyclicPath { start, end });
        }

        Ok(Self {
            id: id.into(),
            swaps,
        })
    }

    /// Returns the starting (and ending) token of this cyclic path.
    #[allow(dead_code)]
    pub fn base_token(&self) -> Address {
        self.swaps[0].token_in
    }

    /// Returns all unique pool addresses involved in this path.
    #[allow(dead_code)]
    pub fn pool_addresses(&self) -> Vec<Address> {
        self.swaps.iter().map(|s| s.pool_address).collect()
    }

    /// Returns the number of hops in this path.
    pub fn len(&self) -> usize {
        self.swaps.len()
    }

    /// Returns true if the path has no swaps.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.swaps.is_empty()
    }
}

impl std::fmt::Display for SwapPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.swaps.is_empty() {
            return write!(f, "[empty path]");
        }

        let first_token = &self.swaps[0].token_in.to_string()[..10];
        write!(f, "{first_token}")?;

        for swap in &self.swaps {
            let token_out = &swap.token_out.to_string()[..10];
            write!(f, " → {token_out}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    fn weth() -> Address {
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
    }

    fn dai() -> Address {
        address!("6B175474E89094C44Da98b954EedeAC495271d0F")
    }

    fn usdc() -> Address {
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
    }

    #[test]
    fn rejects_empty_swaps() {
        let result = SwapPath::new("empty", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_non_cyclic_path() {
        let swaps = vec![Swap {
            pool_address: Address::ZERO,
            token_in: weth(),
            token_out: dai(),
        }];
        let result = SwapPath::new("noncyclic", swaps);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_single_swap_cycle() {
        // A → A is technically cyclic (same token in/out)
        let swaps = vec![Swap {
            pool_address: Address::ZERO,
            token_in: weth(),
            token_out: weth(),
        }];
        let path = SwapPath::new("self-loop", swaps).unwrap();
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn base_token_returns_first_token_in() {
        let pool1 = address!("1111111111111111111111111111111111111111");
        let pool2 = address!("2222222222222222222222222222222222222222");
        let path = SwapPath::new(
            "test",
            vec![
                Swap {
                    pool_address: pool1,
                    token_in: weth(),
                    token_out: dai(),
                },
                Swap {
                    pool_address: pool2,
                    token_in: dai(),
                    token_out: weth(),
                },
            ],
        )
        .unwrap();
        assert_eq!(path.base_token(), weth());
    }

    #[test]
    fn display_format_shows_arrows() {
        let pool1 = address!("1111111111111111111111111111111111111111");
        let pool2 = address!("2222222222222222222222222222222222222222");
        let pool3 = address!("3333333333333333333333333333333333333333");
        let path = SwapPath::new(
            "tri",
            vec![
                Swap {
                    pool_address: pool1,
                    token_in: weth(),
                    token_out: dai(),
                },
                Swap {
                    pool_address: pool2,
                    token_in: dai(),
                    token_out: usdc(),
                },
                Swap {
                    pool_address: pool3,
                    token_in: usdc(),
                    token_out: weth(),
                },
            ],
        )
        .unwrap();
        let display = path.to_string();
        // Should contain arrow separators and start/end tokens
        assert!(display.contains('→'));
        assert_eq!(display.matches('→').count(), 3);
    }
}
