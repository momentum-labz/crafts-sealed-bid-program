use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use crate::state::{Sale, SaleStatus, SealedBid};
use crate::errors::ErrorCode;
use crate::events::RefundClaimed;

#[derive(Accounts)]
pub struct ClaimRefund<'info> {
    #[account(
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = (
            sale.status == SaleStatus::Cancelled ||
            sale.status == SaleStatus::Refunding
        ) @ ErrorCode::SaleNotCancelled,
    )]
    pub sale: Account<'info, Sale>,

    #[account(
        mut,
        close = user,
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.user == user.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.is_initialized @ ErrorCode::InvalidParameters,
        constraint = !sealed_bid.claimed @ ErrorCode::AlreadyClaimed,
        constraint = sealed_bid.amount > 0 @ ErrorCode::InvalidAmount,
        seeds = [b"sealed_bid", sale.key().as_ref(), user.key().as_ref()],
        bump = sealed_bid.bump,
    )]
    pub sealed_bid: Account<'info, SealedBid>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        address = sale.usdc_mint @ ErrorCode::InvalidParameters
    )]
    pub usdc_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == usdc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_usdc_account.owner == user.key() @ ErrorCode::InvalidParameters,
    )]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = sale.usdc_vault @ ErrorCode::InvalidParameters,
    )]
    pub usdc_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler_claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
    let bid = &mut ctx.accounts.sealed_bid;
    let sale = &ctx.accounts.sale;

    let refund_amount = bid.amount;

    bid.claimed = true;
    bid.refund = refund_amount;

    let authority_key = sale.authority;
    let token_mint_key = sale.token_mint;
    let sale_seeds = &[
        b"sale",
        authority_key.as_ref(),
        token_mint_key.as_ref(),
        &[sale.bump],
    ];
    let signer_seeds = &[&sale_seeds[..]];

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

    token::transfer_checked(cpi_ctx, refund_amount, ctx.accounts.usdc_mint.decimals)?;

    msg!(
        "Refund claimed: user={}, amount={}",
        ctx.accounts.user.key(),
        refund_amount
    );

    emit!(RefundClaimed {
        sale: sale.key(),
        user: ctx.accounts.user.key(),
        amount: refund_amount,
    });

    Ok(())
}
