use alloy_primitives::Address;
use thiserror::Error;

/// Domain-specific errors for the arbitrage detector.
#[derive(Debug, Error)]
pub enum Error {
    #[error("pool not found: {0}")]
    PoolNotFound(Address),

    #[allow(dead_code)]
    #[error("token not found: {0}")]
    TokenNotFound(Address),

    #[error("swap path is empty")]
    EmptySwapPath,

    #[error("swap path is not cyclic: starts with {start} but ends with {end}")]
    NonCyclicPath { start: Address, end: Address },

    #[error("insufficient liquidity in pool {pool} for amount {amount}")]
    InsufficientLiquidity { pool: Address, amount: u128 },

    #[error("zero amount input")]
    ZeroAmountIn,

    #[error("EVM execution error: {0}")]
    EvmExecution(String),

    #[error("EVM revert: {0}")]
    EvmRevert(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
