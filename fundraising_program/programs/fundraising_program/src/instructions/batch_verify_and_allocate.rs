use anchor_lang::prelude::*;
use crate::state::{Sale, SaleStatus, SealedBid};
use crate::errors::ErrorCode;
use crate::utils::{calculate_tokens, is_marginal_bidder};

pub const MAX_BATCH_SIZE: usize = 10;

#[derive(Accounts)]
pub struct BatchVerifyAndAllocate<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = sale.status == SaleStatus::Proposed ||
                     sale.status == SaleStatus::Verifying @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Account<'info, Sale>,

    pub cranker: Signer<'info>,
}

pub fn handler_batch_verify_and_allocate<'info>(
    ctx: Context<'_, '_, 'info, 'info, BatchVerifyAndAllocate<'info>>,
) -> Result<()> {
    let clearing_fdv = ctx.accounts.sale.clearing_fdv.ok_or(ErrorCode::InvalidSaleStatus)?;
    let fill_rate = ctx.accounts.sale.fill_rate.ok_or(ErrorCode::InvalidSaleStatus)?;
    let sale_key = ctx.accounts.sale.key();
    let program_id = *ctx.program_id;
    let settlement_nonce = ctx.accounts.sale.settlement_nonce;
    let marginal_user = ctx.accounts.sale.marginal_user;
    let marginal_allocation = ctx.accounts.sale.marginal_allocation;
    let token_supply = ctx.accounts.sale.token_supply;

    let params = BatchProcessParams {
        sale_key,
        program_id,
        clearing_fdv,
        fill_rate,
        settlement_nonce,
        marginal_user,
        marginal_allocation,
        token_supply,
    };

    require!(!ctx.remaining_accounts.is_empty(), ErrorCode::BatchEmpty);
    require!(
        ctx.remaining_accounts.len() <= MAX_BATCH_SIZE,
        ErrorCode::BatchSizeTooLarge
    );

    let mut verified_count: u32 = 0;
    let mut running_sum_delta: u64 = 0;
    let mut marginal_user_verified = false;

    // Process each SealedBid
    for account_info in ctx.remaining_accounts.iter() {
        require!(
            account_info.owner == &params.program_id,
            ErrorCode::InvalidParameters
        );

        let mut bid: Account<SealedBid> = Account::try_from(account_info)?;

        require!(bid.sale == params.sale_key, ErrorCode::InvalidParameters);
        require!(bid.is_initialized, ErrorCode::InvalidParameters);
        require!(bid.amount > 0, ErrorCode::InvalidAmount);
        require!(
            bid.verified_for_nonce != params.settlement_nonce,
            ErrorCode::AlreadyVerified
        );

        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"sealed_bid", params.sale_key.as_ref(), bid.user.as_ref()],
            &params.program_id,
        );
        require!(account_info.key() == expected_pda, ErrorCode::InvalidParameters);

        let (allocation, refund, is_marginal, cleared, tokens) =
            calculate_user_allocation(&params, &bid)?;

        let (final_allocation, final_refund, final_tokens, final_cleared) =
            if allocation > 0 && tokens == 0 {
                (
                    0u64,
                    bid.amount,
                    0u64,
                    false,
                )
            } else {
                (allocation, refund, tokens, cleared)
            };

        bid.cleared = final_cleared;
        bid.is_marginal = is_marginal;
        bid.allocation = final_allocation;
        bid.refund = final_refund;
        bid.tokens = final_tokens;
        bid.verified_for_nonce = params.settlement_nonce;

        if is_marginal && params.marginal_user == Some(bid.user) {
            marginal_user_verified = true;
        }

        // Accumulate totals
        running_sum_delta = running_sum_delta
            .checked_add(final_allocation)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        verified_count = verified_count
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        msg!(
            "Verified: user={}, cleared={}, allocation={}, tokens={}",
            bid.user,
            final_cleared,
            final_allocation,
            final_tokens
        );

        bid.exit(&params.program_id)?;
    }

    let sale = &mut ctx.accounts.sale;
    
    sale.running_sum = sale
        .running_sum
        .checked_add(running_sum_delta)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    sale.verification_count = sale
        .verification_count
        .checked_add(verified_count)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if marginal_user_verified {
        sale.marginal_user_verified = true;
    }

    if sale.status == SaleStatus::Proposed {
        sale.status = SaleStatus::Verifying;
    }

    msg!(
        "Batch complete: verified={}, running_sum_delta={}, total_verified={}/{}",
        verified_count,
        running_sum_delta,
        sale.verification_count,
        sale.total_users
    );

    Ok(())
}

struct BatchProcessParams {
    sale_key: Pubkey,
    program_id: Pubkey,
    clearing_fdv: u64,
    fill_rate: u64,
    settlement_nonce: u64,
    marginal_user: Option<Pubkey>,
    marginal_allocation: Option<u64>,
    token_supply: u64,
}

fn calculate_user_allocation(
    params: &BatchProcessParams,
    bid: &SealedBid,
) -> Result<(u64, u64, bool, bool, u64)> {
    require!(bid.bid_revealed, ErrorCode::BidNotRevealed);

    let user_max_fdv = bid.max_fdv_plaintext.ok_or(ErrorCode::BidNotRevealed)?;
    let clearing_fdv = params.clearing_fdv;
    let fill_rate = params.fill_rate;

    let (allocation, refund, is_marginal, cleared) = if user_max_fdv > clearing_fdv {
        let raw_allocation = (bid.amount as u128)
            .checked_mul(fill_rate as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::DivisionByZero)?;
        let allocation = u64::try_from(raw_allocation).map_err(|_| ErrorCode::ArithmeticOverflow)?;
        let refund = bid
            .amount
            .checked_sub(allocation)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        (allocation, refund, false, true)
    } else if is_marginal_bidder(user_max_fdv, clearing_fdv) {
        if params.marginal_user == Some(bid.user) {
            require!(
                user_max_fdv == clearing_fdv,
                ErrorCode::InvalidMarginalAllocation
            );

            let marginal_alloc = params
                .marginal_allocation
                .ok_or(ErrorCode::InvalidMarginalAllocation)?;
            require!(
                marginal_alloc <= bid.amount,
                ErrorCode::InvalidMarginalAllocation
            );
            let refund = bid
                .amount
                .checked_sub(marginal_alloc)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            (marginal_alloc, refund, true, true)
        } else {
            let raw_allocation = (bid.amount as u128)
                .checked_mul(fill_rate as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(10000)
                .ok_or(ErrorCode::DivisionByZero)?;
            let allocation =
                u64::try_from(raw_allocation).map_err(|_| ErrorCode::ArithmeticOverflow)?;
            let refund = bid
                .amount
                .checked_sub(allocation)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            (allocation, refund, false, true)
        }
    } else {
        (0u64, bid.amount, false, false)
    };

    let tokens = if allocation > 0 {
        calculate_tokens(allocation, params.token_supply, clearing_fdv)?
    } else {
        0
    };

    Ok((allocation, refund, is_marginal, cleared, tokens))
}

