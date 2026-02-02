use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::commit;
use ephemeral_rollups_sdk::ephem::commit_and_undelegate_accounts;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::BatchUndelegated;

/// Undelegate batch: commit_and_undelegate SealedBid accounts from PER back to mainnet.
/// All max_fdv values must already be zeroed (by allocate_batch).
#[commit]
#[derive(Accounts)]
pub struct UndelegateBatch<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = sale.status == SaleStatus::Allocated @ ErrorCode::InvalidSaleStatus,
        constraint = sale.bids_zeroed @ ErrorCode::BidsNotZeroed,
    )]
    pub sale: Account<'info, Sale>,

    #[account(mut)]
    pub payer: Signer<'info>,
}

pub const MAX_UNDELEGATE_BATCH_SIZE: usize = 10;

pub fn handler_undelegate_batch<'info>(
    ctx: Context<'_, '_, 'info, 'info, UndelegateBatch<'info>>,
) -> Result<()> {
    let remaining = &ctx.remaining_accounts;
    require!(!remaining.is_empty(), ErrorCode::BatchEmpty);
    require!(remaining.len() <= MAX_UNDELEGATE_BATCH_SIZE, ErrorCode::BatchSizeTooLarge);

    // Collect account infos for undelegation
    let accounts_to_undelegate: Vec<&AccountInfo<'info>> = remaining.iter().collect();

    commit_and_undelegate_accounts(
        &ctx.accounts.payer,
        accounts_to_undelegate,
        &ctx.accounts.magic_context,
        &ctx.accounts.magic_program,
    )?;

    msg!(
        "Batch undelegated: count={}",
        remaining.len(),
    );

    emit!(BatchUndelegated {
        sale: ctx.accounts.sale.key(),
        batch_size: remaining.len() as u32,
    });

    Ok(())
}
