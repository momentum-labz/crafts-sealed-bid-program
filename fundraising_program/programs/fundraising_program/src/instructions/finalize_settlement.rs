use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::SettlementFinalized;

#[derive(Accounts)]
pub struct FinalizeSettlement<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = sale.status == SaleStatus::Verifying @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,

    #[account(
        address = sale.token_vault @ ErrorCode::InvalidParameters,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    /// Anyone can finalize (permissionless)
    pub finalizer: Signer<'info>,
}

pub fn handler_finalize_settlement(ctx: Context<FinalizeSettlement>) -> Result<()> {
    let sale = &mut ctx.accounts.sale;
    
    // Verify all users have been processed
    require!(
        sale.verification_count == sale.total_users,
        ErrorCode::VerificationIncomplete
    );
    
    if sale.marginal_user.is_some() {
        require!(
            sale.marginal_user_verified,
            ErrorCode::VerificationIncomplete
        );
    }
    
    require!(
        sale.running_sum <= sale.raise_max,
        ErrorCode::RunningSumExceedsMax
    );

    require!(
        sale.running_sum >= sale.raise_min,
        ErrorCode::RunningSumBelowMin
    );

    require!(sale.running_sum > 0, ErrorCode::InsufficientDemand);

    let clearing_fdv = sale.clearing_fdv.ok_or(ErrorCode::InvalidSaleStatus)?;
    let total_tokens_needed = (sale.running_sum as u128)
        .checked_mul(sale.token_supply as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(clearing_fdv as u128)
        .ok_or(ErrorCode::DivisionByZero)?;
    let total_tokens_needed_u64 = u64::try_from(total_tokens_needed)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?;

    require!(
        ctx.accounts.token_vault.amount >= total_tokens_needed_u64,
        ErrorCode::InsufficientTokens
    );

    // Store the total cleared commits for reference
    sale.total_cleared_commits = Some(sale.running_sum);
    
    // Update status to Settled - users can now claim
    sale.status = SaleStatus::Settled;
    
    msg!(
        "Settlement finalized: sale={}, total_raised={}, users_verified={}, finalizer={}",
        sale.key(),
        sale.running_sum,
        sale.verification_count,
        ctx.accounts.finalizer.key()
    );

    emit!(SettlementFinalized {
        sale: sale.key(),
        total_raised: sale.running_sum,
        total_users: sale.total_users,
    });

    Ok(())
}