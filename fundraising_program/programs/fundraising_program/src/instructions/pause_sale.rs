use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::{SalePaused, SaleUnpaused};

#[derive(Accounts)]
pub struct PauseSale<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = (
            sale.status == SaleStatus::Initialized ||
            sale.status == SaleStatus::Active
        ) @ ErrorCode::InvalidSaleStatus,
        constraint = !sale.is_paused @ ErrorCode::AlreadyProcessed,
    )]
    pub sale: Account<'info, Sale>,
    pub authority: Signer<'info>,
}

pub fn handler_pause_sale(ctx: Context<PauseSale>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;

    sale.is_paused = true;

    msg!(
        "Sale paused: sale={}, authority={}",
        sale.key(),
        ctx.accounts.authority.key()
    );

    emit!(SalePaused {
        sale: sale.key(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UnpauseSale<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = (
            sale.status == SaleStatus::Initialized ||
            sale.status == SaleStatus::Active
        ) @ ErrorCode::InvalidSaleStatus,
        constraint = sale.is_paused @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,
    pub authority: Signer<'info>,
}

pub fn handler_unpause_sale(ctx: Context<UnpauseSale>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;

    sale.is_paused = false;

    msg!(
        "Sale unpaused: sale={}, authority={}",
        sale.key(),
        ctx.accounts.authority.key()
    );

    emit!(SaleUnpaused {
        sale: sale.key(),
    });

    Ok(())
}
