use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::RefundModeEnabled;

/// Trigger full refund mode - allows authority to enable refunds for all users
/// without going through full cancellation. Users must claim their refunds.
#[derive(Accounts)]
pub struct RefundSale<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        // Can refund from most states except already finalized or already refunding
        constraint = (
            sale.status == SaleStatus::Initialized ||
            sale.status == SaleStatus::Active ||
            sale.status == SaleStatus::Ready ||
            sale.status == SaleStatus::Settling ||
            sale.status == SaleStatus::Settled
        ) @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Box<Account<'info, Sale>>,
    pub authority: Signer<'info>,
}

pub fn handler_refund_sale(ctx: Context<RefundSale>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;

    let prev_status = sale.status;

    // Set to Refunding status - users can claim full refunds via claim_refund
    sale.status = SaleStatus::Refunding;

    // Ensure claims are enabled so users can get their refunds
    sale.claims_enabled = true;

    msg!(
        "Refund mode enabled: sale={}, authority={}, prev_status={:?}",
        sale.key(),
        ctx.accounts.authority.key(),
        prev_status
    );

    emit!(RefundModeEnabled {
        sale: sale.key(),
    });

    Ok(())
}
