use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::SaleFunded;

#[derive(Accounts)]
pub struct FundSale<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), token_mint.key().as_ref()],
        bump = sale.bump,
        has_one = authority @ ErrorCode::InvalidParameters,
        constraint = sale.status == SaleStatus::Initialized @ ErrorCode::SaleNotInitialized,
    )]
    pub sale: Account<'info, Sale>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        address = sale.token_mint @ ErrorCode::InvalidParameters
    )]
    pub token_mint: Account<'info, Mint>,
    
    #[account(
        mut,
        constraint = authority_token_account.mint == token_mint.key() @ ErrorCode::InvalidParameters,
        constraint = authority_token_account.owner == authority.key() @ ErrorCode::InvalidParameters,
    )]
    pub authority_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        address = sale.token_vault @ ErrorCode::InvalidParameters
    )]
    pub token_vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

pub fn handler_fund_sale(
    ctx: Context<FundSale>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);
    
    let now = Clock::get()?.unix_timestamp;
    require!(
        now < ctx.accounts.sale.commitment_start,
        ErrorCode::CommitmentWindowClosed
    );
    
    let required_tokens = (ctx.accounts.sale.token_supply as u128)
        .checked_mul(ctx.accounts.sale.supply_percentage as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(ErrorCode::DivisionByZero)?;
    
    require!(
        amount as u128 >= required_tokens,
        ErrorCode::InsufficientTokens
    );
    
    let decimals = ctx.accounts.token_mint.decimals;
    
    let cpi_accounts = TransferChecked {
        from: ctx.accounts.authority_token_account.to_account_info(),
        mint: ctx.accounts.token_mint.to_account_info(),
        to: ctx.accounts.token_vault.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };
    
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );
    
    token::transfer_checked(cpi_ctx, amount, decimals)?;
    
    ctx.accounts.sale.status = SaleStatus::Active;

    msg!("Sale funded and activated: {:?}", ctx.accounts.sale.key());

    emit!(SaleFunded {
        sale: ctx.accounts.sale.key(),
        amount,
    });

    Ok(())
}