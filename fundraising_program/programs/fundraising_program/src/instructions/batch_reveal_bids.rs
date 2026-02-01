use anchor_lang::prelude::*;
use sha2::{Sha256, Digest};
use crate::state::{Sale, SaleStatus, SealedBid};
use crate::errors::ErrorCode;
use crate::utils::verify_drand_signature;
use crate::events::BidsRevealed;

pub const MAX_REVEAL_BATCH_SIZE: usize = 20;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RevealedBidData {
    pub user: Pubkey,
    pub max_fdv: u64,
    pub salt: [u8; 16],
}

#[derive(Accounts)]
pub struct BatchRevealBids<'info> {
    #[account(
        mut,
        constraint = sale.status == SaleStatus::Revealing @ ErrorCode::SaleNotRevealing,
    )]
    pub sale: Account<'info, Sale>,

    /// Anyone can reveal (permissionless)
    pub cranker: Signer<'info>,
}

pub fn handler_batch_reveal_bids<'info>(
    ctx: Context<'_, '_, 'info, 'info, BatchRevealBids<'info>>,
    drand_round: u64,
    drand_signature: [u8; 48],
    revealed_bids: Vec<RevealedBidData>,
) -> Result<()> {
    let sale = &mut ctx.accounts.sale;
    let remaining_accounts = &ctx.remaining_accounts;

    require!(
        !remaining_accounts.is_empty() && remaining_accounts.len() <= MAX_REVEAL_BATCH_SIZE,
        ErrorCode::InvalidParameters
    );
    require!(
        revealed_bids.len() == remaining_accounts.len(),
        ErrorCode::InvalidParameters
    );

    require!(
        drand_round == sale.drand_reveal_round,
        ErrorCode::InvalidDrandRound
    );

    verify_drand_signature(
        &sale.drand_chain_hash,
        drand_round,
        &drand_signature,
    )?;

    let sale_key = sale.key();
    let mut revealed_count: u32 = 0;

    // Process each commitment
    for (i, account_info) in remaining_accounts.iter().enumerate() {
        let mut bid: Account<SealedBid> = Account::try_from(account_info)?;
        let revealed_bid = &revealed_bids[i];

        require!(bid.sale == sale_key, ErrorCode::InvalidParameters);
        require!(bid.user == revealed_bid.user, ErrorCode::InvalidParameters);
        require!(!bid.bid_revealed, ErrorCode::BidAlreadyRevealed);
        require!(bid.drand_round == drand_round, ErrorCode::InvalidDrandRound);

        let (expected_pda, _bump) = Pubkey::find_program_address(
            &[b"sealed_bid", sale_key.as_ref(), bid.user.as_ref()],
            ctx.program_id,
        );
        require!(account_info.key() == expected_pda, ErrorCode::InvalidParameters);

        require!(
            revealed_bid.max_fdv >= sale.fdv_min && revealed_bid.max_fdv <= sale.fdv_max,
            ErrorCode::InvalidFdvRange
        );

        let mut hasher = Sha256::new();
        hasher.update(&revealed_bid.max_fdv.to_le_bytes());
        hasher.update(&revealed_bid.salt);
        let computed_hash: [u8; 32] = hasher.finalize().into();
        require!(
            computed_hash == bid.hash_commitment,
            ErrorCode::HashCommitmentMismatch
        );

        // Write revealed plaintext
        bid.max_fdv_plaintext = Some(revealed_bid.max_fdv);
        bid.bid_revealed = true;

        revealed_count += 1;

        bid.exit(ctx.program_id)?;

        msg!(
            "Bid revealed: user={}, max_fdv={}",
            bid.user,
            revealed_bid.max_fdv
        );
    }

    sale.reveal_count = sale.reveal_count
        .checked_add(revealed_count)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if sale.reveal_count >= sale.total_users {
        sale.all_bids_revealed = true;
        sale.status = SaleStatus::CommitmentEnded;
        sale.commitment_ended_at = Clock::get()?.unix_timestamp;
        msg!("All bids revealed! Status → CommitmentEnded");
    }

    msg!(
        "Batch reveal complete: revealed={}, total={}/{}",
        revealed_count,
        sale.reveal_count,
        sale.total_users
    );

    emit!(BidsRevealed {
        sale: sale.key(),
        batch_size: revealed_count,
        total_revealed: sale.reveal_count,
        all_revealed: sale.all_bids_revealed,
    });

    Ok(())
}
