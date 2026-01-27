use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::CommitmentWindowClosed;

#[derive(Accounts)]
pub struct CloseCommitmentWindow<'info> {
    #[account(
        mut,
        constraint = sale.status == SaleStatus::Active @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,

    /// Anyone can close the window (permissionless)
    pub closer: Signer<'info>,
}

pub fn handler_close_commitment_window(
    ctx: Context<CloseCommitmentWindow>,
) -> Result<()> {
    let sale = &mut ctx.accounts.sale;
    let now = Clock::get()?.unix_timestamp;

    require!(
        now > sale.commitment_end,
        ErrorCode::CommitmentWindowNotClosed
    );

    sale.status = SaleStatus::Revealing;

    msg!(
        "Commitment window closed: sale={}, total_users={}, total_committed={}",
        sale.key(),
        sale.total_users,
        sale.total_committed
    );

    emit!(CommitmentWindowClosed {
        sale: sale.key(),
        total_users: sale.total_users,
        total_committed: sale.total_committed,
    });

    Ok(())
}
