use anchor_lang::prelude::*;
use crate::state::{Sale, SealedBid, SaleStatus};
use crate::errors::ErrorCode;
use crate::utils::calculate_tokens;
use crate::events::BatchAllocated;

/// Allocate batch: compute allocation/refund/tokens for each bid,
/// zero out max_fdv_plaintext so bid values never hit mainnet.
/// Runs in PER. Processes bids passed via remaining_accounts.
#[derive(Accounts)]
pub struct AllocateBatch<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = sale.status == SaleStatus::Computing @ ErrorCode::InvalidSaleStatus,
        constraint = sale.settlement_computed @ ErrorCode::SettlementNotComputed,
    )]
    pub sale: Account<'info, Sale>,

    /// Permissionless cranker
    pub cranker: Signer<'info>,
}

pub const MAX_ALLOCATE_BATCH_SIZE: usize = 10;

pub fn handler_allocate_batch<'info>(
    ctx: Context<'_, '_, 'info, 'info, AllocateBatch<'info>>,
) -> Result<()> {
    let remaining = &ctx.remaining_accounts;
    require!(!remaining.is_empty(), ErrorCode::BatchEmpty);
    require!(remaining.len() <= MAX_ALLOCATE_BATCH_SIZE, ErrorCode::BatchSizeTooLarge);

    let sale = &mut ctx.accounts.sale;
    let clearing_fdv = sale.clearing_fdv.ok_or(ErrorCode::InvalidSaleStatus)?;
    let fill_rate = sale.fill_rate.ok_or(ErrorCode::InvalidSaleStatus)?;
    let token_supply = sale.token_supply;
    let sale_key = sale.key();

    let mut batch_count: u32 = 0;

    for account_info in remaining.iter() {
        // Deserialize the SealedBid account
        let mut data = account_info.try_borrow_mut_data()?;

        // Validate it's a SealedBid by checking the discriminator
        let (disc, rest) = data.split_at_mut(8);
        let expected_disc = SealedBid::DISCRIMINATOR;
        require!(disc == expected_disc, ErrorCode::InvalidParameters);

        let mut bid = SealedBid::try_from_slice(rest)
            .map_err(|_| ErrorCode::InvalidParameters)?;

        // Validate this bid belongs to this sale
        require!(bid.sale == sale_key, ErrorCode::InvalidParameters);
        require!(bid.is_initialized, ErrorCode::InvalidParameters);

        // Skip already allocated bids
        if bid.cleared {
            continue;
        }

        let max_fdv = bid.max_fdv_plaintext.ok_or(ErrorCode::BidNotSubmitted)?;

        // Determine allocation based on clearing FDV
        if max_fdv > clearing_fdv {
            // Bid clears fully
            let allocation = if fill_rate < 10000 {
                // Pro-rata: allocation = amount * fill_rate / 10000
                (bid.amount as u128)
                    .checked_mul(fill_rate as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(10000)
                    .ok_or(ErrorCode::DivisionByZero)? as u64
            } else {
                bid.amount
            };

            let tokens = calculate_tokens(allocation, token_supply, clearing_fdv)?;

            // Dust protection: if 0 tokens, full refund
            if tokens == 0 {
                bid.allocation = 0;
                bid.refund = bid.amount;
                bid.tokens = 0;
                bid.cleared = true;
                bid.is_marginal = false;
            } else {
                bid.allocation = allocation;
                bid.refund = bid.amount.checked_sub(allocation).ok_or(ErrorCode::ArithmeticOverflow)?;
                bid.tokens = tokens;
                bid.cleared = true;
                bid.is_marginal = false;
            }

            sale.running_sum = sale.running_sum.checked_add(bid.allocation).ok_or(ErrorCode::ArithmeticOverflow)?;
        } else if max_fdv == clearing_fdv && sale.marginal_user.is_some() && bid.user == sale.marginal_user.unwrap() {
            // Marginal bidder
            let allocation = sale.marginal_allocation.ok_or(ErrorCode::InvalidMarginalAllocation)?;
            let tokens = calculate_tokens(allocation, token_supply, clearing_fdv)?;

            if tokens == 0 {
                bid.allocation = 0;
                bid.refund = bid.amount;
                bid.tokens = 0;
            } else {
                bid.allocation = allocation;
                bid.refund = bid.amount.checked_sub(allocation).ok_or(ErrorCode::ArithmeticOverflow)?;
                bid.tokens = tokens;
            }
            bid.cleared = true;
            bid.is_marginal = true;
            sale.marginal_user_verified = true;
            sale.running_sum = sale.running_sum.checked_add(bid.allocation).ok_or(ErrorCode::ArithmeticOverflow)?;
        } else if max_fdv == clearing_fdv {
            // At clearing price but not marginal — full allocation with fill_rate
            let allocation = if fill_rate < 10000 {
                (bid.amount as u128)
                    .checked_mul(fill_rate as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(10000)
                    .ok_or(ErrorCode::DivisionByZero)? as u64
            } else {
                bid.amount
            };

            let tokens = calculate_tokens(allocation, token_supply, clearing_fdv)?;

            if tokens == 0 {
                bid.allocation = 0;
                bid.refund = bid.amount;
                bid.tokens = 0;
            } else {
                bid.allocation = allocation;
                bid.refund = bid.amount.checked_sub(allocation).ok_or(ErrorCode::ArithmeticOverflow)?;
                bid.tokens = tokens;
            }
            bid.cleared = true;
            bid.is_marginal = false;
            sale.running_sum = sale.running_sum.checked_add(bid.allocation).ok_or(ErrorCode::ArithmeticOverflow)?;
        } else {
            // Bid below clearing price — full refund
            bid.allocation = 0;
            bid.refund = bid.amount;
            bid.tokens = 0;
            bid.cleared = true;
            bid.is_marginal = false;
        }

        // CRITICAL: Zero out the bid value so it never appears on mainnet
        bid.max_fdv_plaintext = None;

        // Write back the modified bid
        let serialized = bid.try_to_vec().map_err(|_| ErrorCode::InvalidParameters)?;
        rest[..serialized.len()].copy_from_slice(&serialized);

        sale.verification_count = sale.verification_count.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
        batch_count += 1;
    }

    // Check if all bids are allocated
    if sale.verification_count == sale.total_users {
        sale.bids_zeroed = true;
        sale.status = SaleStatus::Allocated;
    }

    msg!(
        "Batch allocated: count={}, total_verified={}/{}",
        batch_count,
        sale.verification_count,
        sale.total_users,
    );

    emit!(BatchAllocated {
        sale: sale.key(),
        batch_size: batch_count,
        verification_count: sale.verification_count,
    });

    Ok(())
}
