use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::SaleCancelled;

#[derive(Accounts)]
pub struct CancelSale<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = sale.status != SaleStatus::Settled @ ErrorCode::CannotCancelSettledSale,
        constraint = sale.status != SaleStatus::Finalized @ ErrorCode::CannotCancelSettledSale,
    )]
    pub sale: Account<'info, Sale>,
    
    pub authority: Signer<'info>,
}

pub fn handler_cancel_sale(ctx: Context<CancelSale>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;

    let prev_status = sale.status;

    if sale.status == SaleStatus::Proposed || sale.status == SaleStatus::Verifying {
        let is_undersubscribed = sale.total_committed < sale.raise_min;
        require!(
            is_undersubscribed,
            ErrorCode::CannotCancelDuringVerification
        );
    }

    sale.status = SaleStatus::Cancelled;

    msg!(
        "Sale cancelled: sale={}, authority={}, prev_status={:?}",
        sale.key(),
        ctx.accounts.authority.key(),
        prev_status
    );

    emit!(SaleCancelled {
        sale: sale.key(),
    });

    Ok(())
}