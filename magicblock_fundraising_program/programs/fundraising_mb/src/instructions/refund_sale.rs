use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::RefundModeEnabled;

#[derive(Accounts)]
pub struct RefundSale<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = (
            sale.status == SaleStatus::Initialized ||
            sale.status == SaleStatus::Active ||
            sale.status == SaleStatus::Delegated ||
            sale.status == SaleStatus::Computing ||
            sale.status == SaleStatus::Allocated
        ) @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,
    pub authority: Signer<'info>,
}

pub fn handler_refund_sale(ctx: Context<RefundSale>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;

    let prev_status = sale.status;

    sale.status = SaleStatus::Refunding;
    sale.claims_enabled = true;

    msg!("Refund mode enabled: sale={}, prev_status={:?}", sale.key(), prev_status);

    emit!(RefundModeEnabled {
        sale: sale.key(),
    });

    Ok(())
}
