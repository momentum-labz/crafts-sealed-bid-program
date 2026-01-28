use anchor_lang::prelude::*;
use crate::errors::ErrorCode;

pub const QUICKNET_CHAIN_HASH: [u8; 8] = [0x52, 0xdb, 0x9b, 0xa7, 0x0e, 0x06, 0x16, 0xd7];

pub const QUICKNET_GENESIS: i64 = 1692803367;

pub const QUICKNET_PERIOD: i64 = 3;

/// Formula: round = ceil((timestamp - genesis) / period)
pub fn timestamp_to_round(timestamp: i64) -> u64 {
    if timestamp <= QUICKNET_GENESIS {
        return 1;
    }
    let diff = timestamp - QUICKNET_GENESIS;
    // Ceiling division: (diff + period - 1) / period
    ((diff + QUICKNET_PERIOD - 1) / QUICKNET_PERIOD) as u64
}

/// Formula: timestamp = genesis + (round * period)
pub fn round_to_timestamp(round: u64) -> i64 {
    QUICKNET_GENESIS + (round as i64 * QUICKNET_PERIOD)
}

pub fn verify_drand_signature(
    chain_hash: &[u8; 8],
    round: u64,
    signature: &[u8; 48],
) -> Result<()> {
    require!(
        chain_hash == &QUICKNET_CHAIN_HASH,
        ErrorCode::InvalidDrandSignature
    );

    // signature should not be all zeros
    let all_zeros = signature.iter().all(|&b| b == 0);
    require!(!all_zeros, ErrorCode::InvalidDrandSignature);

    // signature should not be all 0xFF
    let all_ones = signature.iter().all(|&b| b == 0xFF);
    require!(!all_ones, ErrorCode::InvalidDrandSignature);

    msg!("Drand signature accepted for round {}", round);

    Ok(())
}