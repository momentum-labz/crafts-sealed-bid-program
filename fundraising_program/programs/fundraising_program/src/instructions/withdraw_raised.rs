use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::RaisedWithdrawn;

#[derive(Accounts)]
pub struct WithdrawRaised<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = sale.status == SaleStatus::Settled @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        address = sale.usdc_mint @ ErrorCode::InvalidParameters
    )]
    pub usdc_mint: Account<'info, Mint>,

    #[account(
        mut,
        address = sale.usdc_vault @ ErrorCode::InvalidParameters,
    )]
    pub usdc_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = sale.usdc_treasury @ ErrorCode::InvalidParameters,
    )]
    pub usdc_treasury: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler_withdraw_raised(ctx: Context<WithdrawRaised>) -> Result<()> {
    let raised_amount = ctx.accounts.sale.running_sum;
    let authority_key = ctx.accounts.sale.authority;
    let token_mint_key = ctx.accounts.sale.token_mint;
    let bump = ctx.accounts.sale.bump;

    require!(raised_amount > 0, ErrorCode::InsufficientFunds);
    
    let vault_balance = ctx.accounts.usdc_vault.amount;
    require!(
        vault_balance >= raised_amount,
        ErrorCode::InsufficientFunds
    );

    ctx.accounts.sale.status = SaleStatus::Finalized;

    let sale_seeds = &[
        b"sale",
        authority_key.as_ref(),
        token_mint_key.as_ref(),
        &[bump],
    ];
    let signer_seeds = &[&sale_seeds[..]];

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.usdc_vault.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        to: ctx.accounts.usdc_treasury.to_account_info(),
        authority: ctx.accounts.sale.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    token::transfer_checked(cpi_ctx, raised_amount, ctx.accounts.usdc_mint.decimals)?;

    msg!(
        "Raised USDC withdrawn: sale={}, amount={}, authority={}",
        ctx.accounts.sale.key(),
        raised_amount,
        ctx.accounts.authority.key()
    );

    emit!(RaisedWithdrawn {
        sale: ctx.accounts.sale.key(),
        amount: raised_amount,
    });

    Ok(())
}

