use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::SettlementProposed;

#[derive(Accounts)]
pub struct ProposeSettlement<'info> {
    #[account(
        mut,
        seeds = [b"sale", authority.key().as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        has_one = authority,
        constraint = (
            sale.status == SaleStatus::Active ||
            sale.status == SaleStatus::CommitmentEnded ||
            sale.status == SaleStatus::Proposed ||
            sale.status == SaleStatus::Verifying
        ) @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,
    pub authority: Signer<'info>,
}

pub const PROPOSAL_COOLDOWN_SECONDS: i64 = 60;

pub const INITIAL_PROPOSAL_DELAY_SECONDS: i64 = 30;

pub fn handler_propose_settlement(
    ctx: Context<ProposeSettlement>,
    clearing_fdv: u64,
    fill_rate: u64,
    marginal_user: Option<Pubkey>,
    marginal_allocation: Option<u64>,
) -> Result<()> {
    let sale = &mut ctx.accounts.sale;
    let now = Clock::get()?.unix_timestamp;

    require!(
        now > sale.commitment_end,
        ErrorCode::CommitmentWindowNotEnded
    );

    if sale.last_proposal_time == 0 {
        let min_proposal_time = sale.commitment_end
            .checked_add(INITIAL_PROPOSAL_DELAY_SECONDS)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        require!(
            now >= min_proposal_time,
            ErrorCode::InvalidTimestamps
        );
    } else {
        let min_proposal_time = sale.last_proposal_time
            .checked_add(PROPOSAL_COOLDOWN_SECONDS)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        require!(
            now >= min_proposal_time,
            ErrorCode::InvalidTimestamps
        );
    }

    require!(
        sale.total_users > 0,
        ErrorCode::InsufficientDemand
    );

    require!(
        sale.all_bids_revealed,
        ErrorCode::BidsNotRevealed
    );

    require!(
        clearing_fdv >= sale.fdv_min && clearing_fdv <= sale.fdv_max,
        ErrorCode::InvalidClearingFdv
    );

    require!(
        clearing_fdv > 0,
        ErrorCode::DivisionByZero
    );

    require!(
        fill_rate > 0 && fill_rate <= 10000,
        ErrorCode::InvalidFillRate
    );
    
    if fill_rate == 10000 {
        let tolerance_threshold = sale.raise_max
            .checked_mul(3)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(2)
            .ok_or(ErrorCode::DivisionByZero)?; // 1.5x raise_max
        
        if sale.total_committed > tolerance_threshold && sale.total_committed > sale.raise_min {
            msg!(
                "Warning: High total_committed ({}) with 100% fill_rate. May fail finalization.",
                sale.total_committed
            );
        }
    } else {
        if sale.total_committed <= sale.raise_min {
            msg!(
                "Warning: Low total_committed ({}) with oversubscribed fill_rate ({}). May fail finalization.",
                sale.total_committed,
                fill_rate
            );
        }
        
        let expected_max_raise_128 = (sale.total_committed as u128)
            .checked_mul(fill_rate as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::DivisionByZero)?;
        
        let expected_max_raise = u64::try_from(expected_max_raise_128)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
        
        let raise_max_with_tolerance = sale.raise_max
            .checked_mul(11)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10)
            .ok_or(ErrorCode::DivisionByZero)?; 
        
        require!(
            expected_max_raise <= raise_max_with_tolerance,
            ErrorCode::InvalidFillRate
        );
    }

    if marginal_user.is_some() {
        require!(
            marginal_allocation.is_some(),
            ErrorCode::InvalidMarginalAllocation
        );
    }

    if let Some(alloc) = marginal_allocation {
        require!(
            alloc <= sale.raise_max,
            ErrorCode::InvalidMarginalAllocation
        );
    }

    sale.settlement_nonce = sale.settlement_nonce
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    sale.clearing_fdv = Some(clearing_fdv);
    sale.fill_rate = Some(fill_rate);
    sale.marginal_user = marginal_user;
    sale.marginal_allocation = marginal_allocation;
    sale.proposer = Some(ctx.accounts.authority.key());

    sale.verification_count = 0;
    sale.running_sum = 0;
    sale.marginal_user_verified = false; 
    sale.last_proposal_time = now; 

    sale.status = SaleStatus::Proposed;
    
    msg!(
        "Settlement proposed: sale={}, clearing_fdv={}, fill_rate={}, marginal_user={:?}, marginal_allocation={:?}, authority={}",
        sale.key(),
        clearing_fdv,
        fill_rate,
        marginal_user,
        marginal_allocation,
        ctx.accounts.authority.key()
    );

    emit!(SettlementProposed {
        sale: sale.key(),
        clearing_fdv,
        fill_rate,
        settlement_nonce: sale.settlement_nonce,
    });

    Ok(())
}