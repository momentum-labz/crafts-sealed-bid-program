use anchor_lang::prelude::*;
use crate::state::{Sale, SealedBid, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::BidUpdated;

/// Update bid price while commitment window is open (PER only).
#[derive(Accounts)]
pub struct UpdateBidMb<'info> {
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
        constraint = sealed_bid.bid_submitted @ ErrorCode::BidNotSubmitted,
    )]
    pub sealed_bid: Account<'info, SealedBid>,

    pub user: Signer<'info>,
}

pub fn handler_update_bid_mb(
    ctx: Context<UpdateBidMb>,
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

    require!(
        max_fdv >= sale.fdv_min && max_fdv <= sale.fdv_max,
        ErrorCode::MaxFdvOutOfRange
    );

    bid.max_fdv_plaintext = Some(max_fdv);

    msg!(
        "Bid updated in PER: user={}",
        ctx.accounts.user.key(),
    );

    emit!(BidUpdated {
        sale: sale.key(),
        user: ctx.accounts.user.key(),
    });

    Ok(())
}
