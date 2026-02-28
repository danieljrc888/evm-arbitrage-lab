use alloy_primitives::Address;
use thiserror::Error;

/// Domain-specific errors for the arbitrage detector.
#[derive(Debug, Error)]
pub enum Error {
    #[error("swap path is empty")]
    EmptySwapPath,

    #[error("swap path is not cyclic: starts with {start} but ends with {end}")]
    NonCyclicPath { start: Address, end: Address },

    #[error("insufficient liquidity in pool {pool} for amount {amount}")]
    InsufficientLiquidity { pool: Address, amount: u128 },

    #[error("EVM execution error: {0}")]
    EvmExecution(String),

    #[error("EVM revert: {0}")]
    EvmRevert(String),
}

pub type Result<T> = std::result::Result<T, Error>;
