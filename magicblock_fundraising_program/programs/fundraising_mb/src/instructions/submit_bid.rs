use anchor_lang::prelude::*;
use crate::state::{Sale, SealedBid, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::BidSubmitted;

/// Submit bid privately in PER. Only the bidder can call this (permission-gated).
/// The max_fdv value is written to the SealedBid account inside the TEE —
/// it will be zeroed before the account is undelegated back to mainnet.
#[derive(Accounts)]
pub struct SubmitBid<'info> {
    #[account(
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = sale.status == SaleStatus::Active @ ErrorCode::SaleNotActive,
        constraint = !sale.is_paused @ ErrorCode::SaleNotActive,
    )]
    pub sale: Account<'info, Sale>,

    #[account(
        mut,
        seeds = [b"sealed_bid", sale.key().as_ref(), user.key().as_ref()],
        bump = sealed_bid.bump,
        constraint = sealed_bid.is_initialized @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.user == user.key() @ ErrorCode::InvalidParameters,
        constraint = !sealed_bid.bid_submitted @ ErrorCode::BidAlreadySubmitted,
    )]
    pub sealed_bid: Account<'info, SealedBid>,

    pub user: Signer<'info>,
}

pub fn handler_submit_bid(
    ctx: Context<SubmitBid>,
    max_fdv: u64,
) -> Result<()> {
    let sale = &ctx.accounts.sale;
    let bid = &mut ctx.accounts.sealed_bid;

    let now = Clock::get()?.unix_timestamp;

    require!(
        now >= sale.commitment_start,
        ErrorCode::OutsideCommitmentWindow
    );
    require!(
        now <= sale.commitment_end,
        ErrorCode::CommitmentWindowClosed
    );

    // Validate max_fdv is within sale's FDV range
    require!(
        max_fdv >= sale.fdv_min && max_fdv <= sale.fdv_max,
        ErrorCode::MaxFdvOutOfRange
    );

    bid.max_fdv_plaintext = Some(max_fdv);
    bid.bid_submitted = true;

    msg!(
        "Bid submitted in PER: user={}",
        ctx.accounts.user.key(),
    );

    emit!(BidSubmitted {
        sale: sale.key(),
        user: ctx.accounts.user.key(),
    });

    Ok(())
}
