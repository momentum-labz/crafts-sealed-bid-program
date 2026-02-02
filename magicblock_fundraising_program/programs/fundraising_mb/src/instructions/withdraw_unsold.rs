use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::UnsoldWithdrawn;

#[derive(Accounts)]
pub struct WithdrawUnsold<'info> {
    #[account(
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = (
            sale.status == SaleStatus::Settled ||
            sale.status == SaleStatus::Finalized ||
            sale.status == SaleStatus::Cancelled ||
            sale.status == SaleStatus::Refunding
        ) @ ErrorCode::InvalidSaleStatus,
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
        address = sale.token_vault @ ErrorCode::InvalidParameters,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = sale.token_treasury @ ErrorCode::InvalidParameters,
    )]
    pub token_treasury: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler_withdraw_unsold(ctx: Context<WithdrawUnsold>) -> Result<()> {
    let sale = &ctx.accounts.sale;

    let vault_balance = ctx.accounts.token_vault.amount;
    require!(vault_balance > 0, ErrorCode::InsufficientTokens);

    let tokens_owed_to_users = if sale.status == SaleStatus::Cancelled || sale.status == SaleStatus::Refunding {
        0u64
    } else {
        if let Some(clearing_fdv) = sale.clearing_fdv {
            if clearing_fdv > 0 {
                let tokens_128 = (sale.running_sum as u128)
                    .checked_mul(sale.token_supply as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(clearing_fdv as u128)
                    .ok_or(ErrorCode::DivisionByZero)?;

                u64::try_from(tokens_128).map_err(|_| ErrorCode::ArithmeticOverflow)?
            } else {
                0u64
            }
        } else {
            0u64
        }
    };

    let remaining_tokens = vault_balance
        .checked_sub(tokens_owed_to_users)
        .ok_or(ErrorCode::InsufficientTokens)?;

    require!(remaining_tokens > 0, ErrorCode::InsufficientTokens);

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
        from: ctx.accounts.token_vault.to_account_info(),
        mint: ctx.accounts.token_mint.to_account_info(),
        to: ctx.accounts.token_treasury.to_account_info(),
        authority: ctx.accounts.sale.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );

    token::transfer_checked(cpi_ctx, remaining_tokens, ctx.accounts.token_mint.decimals)?;

    msg!("Unsold tokens withdrawn: sale={}, amount={}", sale.key(), remaining_tokens);

    emit!(UnsoldWithdrawn {
        sale: sale.key(),
        amount: remaining_tokens,
    });

    Ok(())
}
