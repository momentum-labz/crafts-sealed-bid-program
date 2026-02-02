use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use crate::state::{Sale, SaleStatus, SealedBid};
use crate::errors::ErrorCode;
use crate::events::ClaimCompleted;

#[derive(Accounts)]
pub struct ClaimAllocation<'info> {
    #[account(
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = (
            sale.status == SaleStatus::Settled ||
            sale.status == SaleStatus::Finalized
        ) @ ErrorCode::SaleNotSettled,
        constraint = sale.claims_enabled @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Box<Account<'info, Sale>>,

    #[account(
        mut,
        close = user,
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.user == user.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.is_initialized @ ErrorCode::InvalidParameters,
        constraint = !sealed_bid.claimed @ ErrorCode::AlreadyClaimed,
        constraint = sealed_bid.cleared @ ErrorCode::InvalidParameters,
        seeds = [b"sealed_bid", sale.key().as_ref(), user.key().as_ref()],
        bump = sealed_bid.bump,
    )]
    pub sealed_bid: Box<Account<'info, SealedBid>>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        address = sale.token_mint @ ErrorCode::InvalidParameters
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        constraint = user_token_account.mint == token_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_token_account.owner == user.key() @ ErrorCode::InvalidParameters,
    )]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        address = sale.token_vault @ ErrorCode::InvalidParameters,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        address = sale.usdc_mint @ ErrorCode::InvalidParameters
    )]
    pub usdc_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == usdc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_usdc_account.owner == user.key() @ ErrorCode::InvalidParameters,
    )]
    pub user_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        address = sale.usdc_vault @ ErrorCode::InvalidParameters,
    )]
    pub usdc_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn handler_claim_allocation(ctx: Context<ClaimAllocation>) -> Result<()> {
    let bid = &mut ctx.accounts.sealed_bid;
    let sale = &ctx.accounts.sale;

    require!(
        bid.allocation > 0 || bid.refund > 0,
        ErrorCode::InvalidParameters
    );

    let tokens_to_transfer = bid.tokens;
    let refund_to_transfer = bid.refund;

    bid.claimed = true;

    let authority_key = sale.authority;
    let token_mint_key = sale.token_mint;
    let sale_seeds = &[
        b"sale",
        authority_key.as_ref(),
        token_mint_key.as_ref(),
        &[sale.bump],
    ];
    let signer_seeds = &[&sale_seeds[..]];

    if tokens_to_transfer > 0 {
        let token_decimals = ctx.accounts.token_mint.decimals;

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.token_vault.to_account_info(),
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.sale.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        token::transfer_checked(cpi_ctx, tokens_to_transfer, token_decimals)?;
    }

    if refund_to_transfer > 0 {
        let cpi_accounts = TransferChecked {
            from: ctx.accounts.usdc_vault.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
            to: ctx.accounts.user_usdc_account.to_account_info(),
            authority: ctx.accounts.sale.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        token::transfer_checked(cpi_ctx, refund_to_transfer, ctx.accounts.usdc_mint.decimals)?;
    }

    msg!(
        "Claim completed: user={}, tokens={}, refund={}",
        ctx.accounts.user.key(),
        tokens_to_transfer,
        refund_to_transfer
    );

    emit!(ClaimCompleted {
        sale: sale.key(),
        user: ctx.accounts.user.key(),
        tokens: tokens_to_transfer,
        refund: refund_to_transfer,
    });

    Ok(())
}
