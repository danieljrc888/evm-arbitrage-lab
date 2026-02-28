//! EVM-based swap simulator using REVM.
//!
//! Simulates Uniswap V2 swaps by calling actual on-chain contract bytecode
//! through REVM's EVM interpreter. This gives bit-exact results matching
//! mainnet execution, unlike the simplified math in `pricing::dex`.

use alloy_primitives::{Address, Bytes, U256};
use revm::context::result::{ExecutionResult, Output};
use revm::database::CacheDB;
use revm::database_interface::Database;
use revm::primitives::TxKind;
use revm::{Context, ExecuteEvm, MainBuilder, MainContext};
use tracing::debug;

use super::contracts;
use super::shared_backend::SharedBackend;
use crate::error::{Error, Result};
use crate::models::SwapPath;
use crate::strategy::SwapSimulator;

/// Simulates swap paths by executing EVM calls against a forked mainnet state.
///
/// Each simulation gets an isolated `CacheDB` overlay so state changes from
/// one call (e.g. reserve reads) don't leak between independent evaluations.
pub struct EvmSimulator {
    backend: SharedBackend,
}

impl EvmSimulator {
    /// Create a simulator backed by the given shared backend.
    pub fn new(backend: SharedBackend) -> Self {
        Self { backend }
    }

    /// Simulate a single V2 swap by querying reserves via REVM.
    fn simulate_swap(
        &self,
        db: &mut CacheDB<SharedBackend>,
        pool: Address,
        token_in: Address,
        amount_in: u128,
    ) -> Result<u128> {
        // Call getReserves() on the pair contract through REVM
        let calldata = contracts::get_reserves_calldata();
        let response = evm_call(Address::ZERO, pool, calldata, db)?;
        let (reserve0, reserve1) = contracts::decode_reserves(&response)?;

        // Determine which reserve is in vs out by calling token0()
        let token0_calldata = contracts::get_token0_calldata();
        let token0_response = evm_call(Address::ZERO, pool, token0_calldata, db)?;
        let token0 = Address::from_slice(&token0_response[12..32]);

        let (reserve_in, reserve_out) = if token_in == token0 {
            (reserve0, reserve1)
        } else {
            (reserve1, reserve0)
        };

        debug!(
            pool = %pool,
            reserve_in = %reserve_in,
            reserve_out = %reserve_out,
            amount_in,
            "simulating V2 swap via REVM"
        );

        // Apply constant-product formula: same math as UniswapV2.get_amount_out
        let amount_in = U256::from(amount_in);
        let amount_in_with_fee = amount_in * U256::from(997);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        if denominator.is_zero() {
            return Err(Error::InsufficientLiquidity {
                pool,
                amount: amount_in.to::<u128>(),
            });
        }

        let amount_out = numerator / denominator;
        Ok(amount_out.to::<u128>())
    }

    /// Total number of RPC fetches made by the shared backend.
    pub fn rpc_fetch_count(&self) -> usize {
        self.backend.rpc_fetch_count()
    }
}

impl SwapSimulator for EvmSimulator {
    fn simulate_path(&self, path: &SwapPath, amount_in: u128) -> Result<u128> {
        let mut db = self.backend.new_cache_db();
        let mut current = amount_in;

        for swap in &path.swaps {
            current = self.simulate_swap(&mut db, swap.pool_address, swap.token_in, current)?;
        }

        Ok(current)
    }
}

// ---------------------------------------------------------------------------
// Generic EVM call helper
// ---------------------------------------------------------------------------

/// Execute a single EVM call and return the raw output bytes.
///
/// Works with any REVM `Database` implementor. Constructs a minimal
/// transaction context (no value, unlimited gas) and executes via
/// `Context::mainnet()`.
pub fn evm_call<DB: Database>(
    from: Address,
    to: Address,
    calldata: Bytes,
    db: &mut DB,
) -> Result<Bytes> {
    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_tx_chained(|tx| {
            tx.caller = from;
            tx.kind = TxKind::Call(to);
            tx.data = calldata;
            tx.value = U256::ZERO;
        })
        .build_mainnet();

    let ref_tx = evm
        .replay()
        .map_err(|e| Error::EvmExecution(format!("{e:?}")))?;

    match ref_tx.result {
        ExecutionResult::Success {
            output: Output::Call(data),
            ..
        } => Ok(data),
        ExecutionResult::Revert { output, .. } => {
            let reason = if output.len() >= 68 && output.starts_with(&[0x08, 0xc3, 0x79, 0xa0]) {
                let s = &output[68..];
                let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
                String::from_utf8_lossy(&s[..end]).to_string()
            } else {
                format!("0x{}", alloy_primitives::hex::encode(&output))
            };
            Err(Error::EvmRevert(reason))
        }
        ExecutionResult::Halt { reason, .. } => {
            Err(Error::EvmExecution(format!("EVM halt: {reason:?}")))
        }
        _ => Err(Error::EvmExecution("unexpected EVM output".into())),
    }
}
