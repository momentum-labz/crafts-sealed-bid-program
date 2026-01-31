use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use crate::state::{Sale, SealedBid, SaleStatus};
use crate::errors::ErrorCode;
use crate::utils::{verify_merkle_proof, compute_merkle_leaf};
use crate::events::CommitmentMade;

#[derive(Accounts)]
pub struct Commit<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = sale.status == SaleStatus::Active @ ErrorCode::SaleNotActive,
        constraint = !sale.is_paused @ ErrorCode::SaleNotActive,
    )]
    pub sale: Account<'info, Sale>,

    #[account(
        init,
        payer = user,
        space = 8 + SealedBid::INIT_SPACE,
        seeds = [b"sealed_bid", sale.key().as_ref(), user.key().as_ref()],
        bump,
    )]
    pub sealed_bid: Account<'info, SealedBid>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == usdc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_usdc_account.owner == user.key() @ ErrorCode::InvalidParameters,
    )]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = sale.usdc_vault @ ErrorCode::InvalidParameters
    )]
    pub usdc_vault: Account<'info, TokenAccount>,

    #[account(
        address = sale.usdc_mint @ ErrorCode::InvalidParameters
    )]
    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Top up an existing bid with additional USDC (no bid price change).
///
/// NOTE (MEDIUM): Users cannot update their bid price (hash_commitment /
/// max_fdv_encrypted) after the initial commit. To support bid price updates,
/// a future `update_bid` instruction could allow replacing hash_commitment and
/// max_fdv_encrypted while the commitment window is still open. This would
/// require the user to generate a new salt and re-encrypt with the same drand
/// round. The old ciphertext would be overwritten.
#[derive(Accounts)]
pub struct TopupCommit<'info> {
    #[account(
        mut,
        seeds = [b"sale", sale.authority.as_ref(), sale.token_mint.as_ref()],
        bump = sale.bump,
        constraint = sale.status == SaleStatus::Active @ ErrorCode::SaleNotActive,
        constraint = !sale.is_paused @ ErrorCode::SaleNotActive,
    )]
    pub sale: Account<'info, Sale>,

    #[account(
        mut,
        seeds = [b"sealed_bid", sale.key().as_ref(), user.key().as_ref()],
        bump = sealed_bid.bump,
        constraint = sealed_bid.is_initialized @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.user == user.key() @ ErrorCode::InvalidParameters,
        constraint = !sealed_bid.claimed @ ErrorCode::AlreadyClaimed,
    )]
    pub sealed_bid: Account<'info, SealedBid>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == usdc_mint.key() @ ErrorCode::InvalidParameters,
        constraint = user_usdc_account.owner == user.key() @ ErrorCode::InvalidParameters,
    )]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = sale.usdc_vault @ ErrorCode::InvalidParameters
    )]
    pub usdc_vault: Account<'info, TokenAccount>,

    #[account(
        address = sale.usdc_mint @ ErrorCode::InvalidParameters
    )]
    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

pub const MAX_MERKLE_PROOF_DEPTH: usize = 32;

pub const MAX_USERS_PER_SALE: u32 = 10_000;

pub fn handler_commit(
    ctx: Context<Commit>,
    amount: u64,
    max_fdv_encrypted: Vec<u8>,
    drand_round: u64,
    score: u32,
    score_proof: Vec<[u8; 32]>,
    hash_commitment: [u8; 32],
) -> Result<()> {
    require!(
        score_proof.len() <= MAX_MERKLE_PROOF_DEPTH,
        ErrorCode::InvalidMerkleProof
    );

    let sale = &mut ctx.accounts.sale;
    let bid = &mut ctx.accounts.sealed_bid;

    let now = Clock::get()?.unix_timestamp;

    require!(
        now >= sale.commitment_start,
        ErrorCode::OutsideCommitmentWindow
    );
    require!(
        now <= sale.commitment_end,
        ErrorCode::CommitmentWindowClosed
    );

    let leaf = compute_merkle_leaf(&ctx.accounts.user.key(), score);
    require!(
        verify_merkle_proof(&sale.score_merkle_root, &leaf, &score_proof)?,
        ErrorCode::InvalidScoreProof
    );

    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(amount >= sale.min_commitment, ErrorCode::InvalidAmount);

    require!(!max_fdv_encrypted.is_empty(), ErrorCode::EncryptedBidRequired);
    require!(max_fdv_encrypted.len() <= 600, ErrorCode::InvalidParameters);

    require!(
        drand_round == sale.drand_reveal_round,
        ErrorCode::InvalidDrandRound
    );

    require!(
        sale.total_users < MAX_USERS_PER_SALE,
        ErrorCode::CapacityExceeded
    );

    bid.is_initialized = true;
    bid.sale = sale.key();
    bid.user = ctx.accounts.user.key();
    bid.commit_time = now;
    bid.bump = ctx.bumps.sealed_bid;
    bid.cleared = false;
    bid.is_marginal = false;
    bid.claimed = false;
    bid.allocation = 0;
    bid.refund = 0;
    bid.tokens = 0;
    bid.verified_for_nonce = 0;
    bid.amount = amount;
    bid.max_fdv_plaintext = None;
    bid.bid_revealed = false;
    bid.drand_round = drand_round;
    bid.max_fdv_encrypted = Some(max_fdv_encrypted);
    bid.hash_commitment = hash_commitment;

    require!(bid.amount <= sale.raise_max, ErrorCode::InvalidAmount);

    sale.total_users = sale.total_users.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    sale.total_committed = sale.total_committed.checked_add(amount).ok_or(ErrorCode::ArithmeticOverflow)?;

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.user_usdc_account.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        to: ctx.accounts.usdc_vault.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    token::transfer_checked(cpi_ctx, amount, ctx.accounts.usdc_mint.decimals)?;

    msg!(
        "Encrypted bid placed: user={}, amount={}, drand_round={}",
        ctx.accounts.user.key(),
        bid.amount,
        drand_round
    );

    emit!(CommitmentMade {
        sale: sale.key(),
        user: ctx.accounts.user.key(),
        amount,
        total_amount: bid.amount,
        max_fdv: 0,
    });

    Ok(())
}

pub fn handler_topup_commit(
    ctx: Context<TopupCommit>,
    amount: u64,
) -> Result<()> {
    let sale = &mut ctx.accounts.sale;
    let bid = &mut ctx.accounts.sealed_bid;

    let now = Clock::get()?.unix_timestamp;

    require!(
        now >= sale.commitment_start,
        ErrorCode::OutsideCommitmentWindow
    );
    require!(
        now <= sale.commitment_end,
        ErrorCode::CommitmentWindowClosed
    );

    require!(amount > 0, ErrorCode::InvalidAmount);

    require!(
        bid.verified_for_nonce < sale.settlement_nonce || sale.settlement_nonce == 0,
        ErrorCode::AlreadyVerified
    );

    bid.amount = bid.amount.checked_add(amount).ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(bid.amount <= sale.raise_max, ErrorCode::InvalidAmount);

    sale.total_committed = sale.total_committed.checked_add(amount).ok_or(ErrorCode::ArithmeticOverflow)?;

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.user_usdc_account.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        to: ctx.accounts.usdc_vault.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    token::transfer_checked(cpi_ctx, amount, ctx.accounts.usdc_mint.decimals)?;

    msg!(
        "Bid topped up: user={}, added={}, total={}",
        ctx.accounts.user.key(),
        amount,
        bid.amount
    );

    emit!(CommitmentMade {
        sale: sale.key(),
        user: ctx.accounts.user.key(),
        amount,
        total_amount: bid.amount,
        max_fdv: 0,
    });

    Ok(())
}
