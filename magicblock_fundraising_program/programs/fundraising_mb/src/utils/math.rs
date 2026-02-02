use anchor_lang::prelude::*;
use crate::errors::ErrorCode;

/// Safe multiplication with overflow check
pub fn safe_mul_u128(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or(ErrorCode::ArithmeticOverflow.into())
}

/// Safe division with overflow check
pub fn safe_div_u128(a: u128, b: u128) -> Result<u128> {
    if b == 0 {
        return Err(ErrorCode::DivisionByZero.into())
    }

    Ok(a / b)
}

/// Calculate marginal bidder's allocation
pub fn calculate_marginal_allocation(
    raise_target: u64,
    sum_full_allocations: u64,
) -> Result<u64> {
    raise_target
        .checked_sub(sum_full_allocations)
        .ok_or(ErrorCode::ArithmeticOverflow.into())
}

/// Calculate token price from clearing FDV
pub fn calculate_token_price(
    clearing_fdv: u64,
    token_supply: u64,
) -> Result<u64> {
    if token_supply == 0 {
        return Err(ErrorCode::DivisionByZero.into());
    }
    Ok(clearing_fdv / token_supply)
}

/// Calculate tokens from allocation
/// Formula: tokens = (allocation * token_supply) / clearing_fdv
pub fn calculate_tokens(
    allocation: u64,
    token_supply: u64,
    clearing_fdv: u64,
) -> Result<u64> {
    let numerator = (allocation as u128)
        .checked_mul(token_supply as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let tokens = numerator
        .checked_div(clearing_fdv as u128)
        .ok_or(ErrorCode::DivisionByZero)?;

    u64::try_from(tokens).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

/// Determine if a user is a marginal bidder
pub fn is_marginal_bidder(max_fdv: u64, clearing_fdv: u64) -> bool {
    max_fdv == clearing_fdv
}
