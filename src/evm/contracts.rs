//! Uniswap V2 pair ABI definitions for EVM simulation.
//!
//! Uses the `sol!` macro to generate type-safe calldata encoding and
//! response decoding for on-chain contract calls.

use alloy_primitives::{Bytes, U256};
use alloy_sol_types::SolCall;

// Uniswap V2 Pair interface — only the functions we need
alloy_sol_types::sol! {
    function getReserves() external view returns (
        uint112 reserve0,
        uint112 reserve1,
        uint32 blockTimestampLast
    );

    function token0() external view returns (address);
    function token1() external view returns (address);
}

/// Encode the calldata for `getReserves()`.
pub fn get_reserves_calldata() -> Bytes {
    Bytes::from(getReservesCall {}.abi_encode())
}

/// Decode a `getReserves()` response into (reserve0, reserve1).
pub fn decode_reserves(response: &Bytes) -> crate::error::Result<(U256, U256)> {
    let decoded = getReservesCall::abi_decode_returns(response).map_err(|e| {
        crate::error::Error::EvmExecution(format!("failed to decode getReserves response: {e}"))
    })?;
    Ok((U256::from(decoded.reserve0), U256::from(decoded.reserve1)))
}

/// Encode the calldata for `token0()`.
pub fn get_token0_calldata() -> Bytes {
    Bytes::from(token0Call {}.abi_encode())
}

/// Encode the calldata for `token1()`.
#[allow(dead_code)]
pub fn get_token1_calldata() -> Bytes {
    Bytes::from(token1Call {}.abi_encode())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_sol_types::SolValue;

    #[test]
    fn get_reserves_selector() {
        let calldata = get_reserves_calldata();
        // getReserves() selector: 0x0902f1ac
        assert_eq!(&calldata[0..4], &[0x09, 0x02, 0xf1, 0xac]);
        assert_eq!(calldata.len(), 4);
    }

    #[test]
    fn token0_selector() {
        let calldata = get_token0_calldata();
        // token0() selector: 0x0dfe1681
        assert_eq!(&calldata[0..4], &[0x0d, 0xfe, 0x16, 0x81]);
    }

    #[test]
    fn decode_reserves_with_real_abi_encoded_data() {
        // ABI-encode realistic reserve values
        let r0: u128 = 50_000 * 10u128.pow(18);
        let r1: u128 = 100_000_000 * 10u128.pow(6);
        let timestamp: u32 = 1_700_000_000;

        let encoded = (
            alloy_primitives::aliases::U112::from(r0),
            alloy_primitives::aliases::U112::from(r1),
            timestamp,
        )
            .abi_encode();

        let response = Bytes::from(encoded);
        let (reserve0, reserve1) = decode_reserves(&response).unwrap();

        assert_eq!(reserve0, U256::from(r0));
        assert_eq!(reserve1, U256::from(r1));
    }

    #[test]
    fn decode_reserves_rejects_short_response() {
        let short = Bytes::from(vec![0u8; 32]); // needs at least 96 bytes (3 × 32)
        assert!(decode_reserves(&short).is_err());
    }
}
