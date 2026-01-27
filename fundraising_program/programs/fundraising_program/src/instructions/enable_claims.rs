use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::ClaimsEnabled;

#[derive(Accounts)]
pub struct EnableClaims<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = sale.status == SaleStatus::Settled @ ErrorCode::InvalidSaleStatus,
        constraint = !sale.claims_enabled @ ErrorCode::AlreadyProcessed,
    )]
    pub sale: Account<'info, Sale>,
    pub authority: Signer<'info>,
}

pub fn handler_enable_claims(ctx: Context<EnableClaims>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;

    sale.claims_enabled = true;

    msg!(
        "Claims enabled: sale={}, authority={}",
        sale.key(),
        ctx.accounts.authority.key()
    );

    emit!(ClaimsEnabled {
        sale: sale.key(),
    });

    Ok(())
}
