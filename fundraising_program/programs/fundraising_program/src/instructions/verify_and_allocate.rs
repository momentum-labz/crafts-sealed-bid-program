use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus, SealedBid};
use crate::errors::ErrorCode;
use crate::utils::{is_marginal_bidder, calculate_tokens};
use crate::events::UserVerified;

#[derive(Accounts)]
pub struct VerifyAndAllocate<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = (
            sale.status == SaleStatus::Proposed || 
            sale.status == SaleStatus::Verifying
        ) @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,
    
    #[account(
        mut,
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.is_initialized @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.amount > 0 @ ErrorCode::InvalidAmount,
        constraint = sealed_bid.verified_for_nonce != sale.settlement_nonce @ ErrorCode::AlreadyVerified,
        seeds = [b"sealed_bid", sale.key().as_ref(), sealed_bid.user.as_ref()],
        bump = sealed_bid.bump,
    )]
    pub sealed_bid: Account<'info, SealedBid>,
    
    /// Anyone can run verification (permissionless, parallelizable!)
    pub cranker: Signer<'info>,
}

pub fn handler_verify_and_allocate(ctx: Context<VerifyAndAllocate>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;
    let bid = &mut ctx.accounts.sealed_bid;

    let clearing_fdv = sale.clearing_fdv
        .ok_or(ErrorCode::InvalidSaleStatus)?;

    // fill_rate is in basis points: 10000 = 100%, 5000 = 50%
    let fill_rate = sale.fill_rate
        .ok_or(ErrorCode::InvalidSaleStatus)?;

    require!(bid.bid_revealed, ErrorCode::BidNotRevealed);

    // Get max_fdv from plaintext (after reveal)
    let user_max_fdv = bid.max_fdv_plaintext.ok_or(ErrorCode::BidNotRevealed)?;

    let (allocation, refund, is_marginal, cleared) = if user_max_fdv > clearing_fdv {
        let raw_allocation_128 = (bid.amount as u128)
            .checked_mul(fill_rate as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::DivisionByZero)?;
        let raw_allocation = u64::try_from(raw_allocation_128)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
        let user_refund = bid.amount
            .checked_sub(raw_allocation)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        (raw_allocation, user_refund, false, true)
    } else if is_marginal_bidder(user_max_fdv, clearing_fdv) {
        if sale.marginal_user == Some(bid.user) {
            require!(
                user_max_fdv == clearing_fdv,
                ErrorCode::InvalidMarginalAllocation
            );

            let marginal_alloc = sale.marginal_allocation
                .ok_or(ErrorCode::InvalidMarginalAllocation)?;
            
            require!(
                marginal_alloc <= bid.amount,
                ErrorCode::InvalidMarginalAllocation
            );
            
            sale.marginal_user_verified = true;
            
            let user_refund = bid.amount
                .checked_sub(marginal_alloc)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            (marginal_alloc, user_refund, true, true)
        } else {
            let raw_allocation_128 = (bid.amount as u128)
                .checked_mul(fill_rate as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(10000)
                .ok_or(ErrorCode::DivisionByZero)?;
            let raw_allocation = u64::try_from(raw_allocation_128)
                .map_err(|_| ErrorCode::ArithmeticOverflow)?;
            let user_refund = bid.amount
                .checked_sub(raw_allocation)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            (raw_allocation, user_refund, false, true)
        }
    } else {
        (0u64, bid.amount, false, false)
    };
    
    let tokens = if allocation > 0 {
        // tokens = (allocation * tokens_for_sale) / raise_at_clearing
        // tokens_for_sale = token_supply * supply_percentage / 10000
        // raise_at_clearing = clearing_fdv * supply_percentage / 10000
        // Simplified: tokens = (allocation * token_supply) / clearing_fdv
        calculate_tokens(allocation, sale.token_supply, clearing_fdv)?
    } else {
        0
    };
    
    let (final_allocation, final_refund, final_tokens, final_cleared) = if allocation > 0 && tokens == 0 {
        msg!("Dust protection: user {} allocation {} resulted in 0 tokens, giving full refund", 
             bid.user, allocation);
        (0u64, bid.amount, 0u64, false)
    } else {
        (allocation, refund, tokens, cleared)
    };
    
    bid.cleared = final_cleared;
    bid.is_marginal = is_marginal;
    bid.allocation = final_allocation;
    bid.refund = final_refund;
    bid.tokens = final_tokens;
    bid.verified_for_nonce = sale.settlement_nonce;
    
    sale.running_sum = sale.running_sum
        .checked_add(final_allocation)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Increment verification count
    sale.verification_count = sale.verification_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Update status to Verifying if this is the first verification
    if sale.status == SaleStatus::Proposed {
        sale.status = SaleStatus::Verifying;
    }
    
    msg!(
        "User verified: user={}, cleared={}, is_marginal={}, allocation={}, refund={}, tokens={}",
        bid.user,
        final_cleared,
        is_marginal,
        final_allocation,
        final_refund,
        final_tokens
    );

    emit!(UserVerified {
        sale: sale.key(),
        user: bid.user,
        cleared: final_cleared,
        allocation: final_allocation,
        tokens: final_tokens,
    });

    Ok(())
}