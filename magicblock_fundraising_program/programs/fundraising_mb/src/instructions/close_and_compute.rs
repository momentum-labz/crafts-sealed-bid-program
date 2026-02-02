use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::SettlementComputed;

/// Close commitment window and compute settlement parameters in PER.
/// Called by cranker after commitment_end. Sets clearing_fdv, fill_rate, and marginal info.
/// The cranker must sort bids off-chain and propose the correct settlement.
/// This runs inside PER where it can read all delegated SealedBid accounts.
#[derive(Accounts)]
pub struct CloseAndCompute<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = (
            sale.status == SaleStatus::Active ||
            sale.status == SaleStatus::Delegated
        ) @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,

    /// Permissionless — anyone can crank
    pub cranker: Signer<'info>,
}

pub fn handler_close_and_compute<'info>(
    ctx: Context<'_, '_, 'info, 'info, CloseAndCompute<'info>>,
    clearing_fdv: u64,
    fill_rate: u64,
    marginal_user: Option<Pubkey>,
    marginal_allocation: Option<u64>,
) -> Result<()> {
    let sale = &mut ctx.accounts.sale;

    let now = Clock::get()?.unix_timestamp;

    require!(
        now > sale.commitment_end,
        ErrorCode::CommitmentWindowNotEnded
    );

    // Validate clearing_fdv within sale range
    require!(
        clearing_fdv >= sale.fdv_min && clearing_fdv <= sale.fdv_max,
        ErrorCode::InvalidClearingFdv
    );

    // Validate fill_rate: 1-10000 basis points
    require!(
        fill_rate >= 1 && fill_rate <= 10000,
        ErrorCode::InvalidFillRate
    );

    // If marginal_user is set, marginal_allocation must also be set
    if marginal_user.is_some() {
        require!(
            marginal_allocation.is_some(),
            ErrorCode::InvalidMarginalAllocation
        );
    }

    sale.clearing_fdv = Some(clearing_fdv);
    sale.fill_rate = Some(fill_rate);
    sale.marginal_user = marginal_user;
    sale.marginal_allocation = marginal_allocation;
    sale.marginal_user_verified = false;
    sale.verification_count = 0;
    sale.running_sum = 0;
    sale.settlement_computed = true;
    sale.status = SaleStatus::Computing;

    msg!(
        "Settlement computed in PER: clearing_fdv={}, fill_rate={}, marginal_user={:?}",
        clearing_fdv,
        fill_rate,
        marginal_user,
    );

    emit!(SettlementComputed {
        sale: sale.key(),
        clearing_fdv,
        fill_rate,
    });

    Ok(())
}
