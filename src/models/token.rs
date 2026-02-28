use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

/// An ERC-20 token on Ethereum.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Token {
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
}

impl Token {
    #[allow(dead_code)]
    pub fn new(address: Address, symbol: impl Into<String>, decimals: u8) -> Self {
        Self {
            address,
            symbol: symbol.into(),
            decimals,
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.symbol, &self.address.to_string()[..10])
    }
}
