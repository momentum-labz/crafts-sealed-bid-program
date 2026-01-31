use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::SaleInitialized;
use crate::utils::{timestamp_to_round, QUICKNET_CHAIN_HASH};

/// NOTE (MEDIUM - Governance): The authority is a single signer with full
/// control over sale lifecycle (init, fund, propose, pause, cancel, refund).
/// For production deployments, consider:
/// 1. Using a Squads multisig as the authority
/// 2. Adding a timelock for critical operations (cancel, refund)
/// 3. Separating roles (proposer vs admin vs emergency)
#[derive(Accounts)]
pub struct InitializeSale<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Sale::INIT_SPACE,
        seeds = [b"sale", authority.key().as_ref(), token_mint.key().as_ref()],
        bump,
    )]
    pub sale: Account<'info, Sale>,

    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub token_mint: Account<'info, Mint>,
    pub usdc_mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = authority,
        associated_token::mint = usdc_mint,
        associated_token::authority = sale
    )]
    pub usdc_vault: Account<'info, TokenAccount>,
    
    #[account(
        init,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = sale
    )]
    pub token_vault: Account<'info, TokenAccount>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

/// Minimum commitment: $5 USDC (with 6 decimals)
pub const MIN_COMMITMENT: u64 = 5_000_000;

/// Minimum commitment window: 15 minutes
pub const MIN_COMMITMENT_WINDOW_SECONDS: i64 = 1 * 60; // set to 1 minute for testing, 15 minutes for production

pub fn handler_initialize_sale(
    ctx: Context<InitializeSale>,
    name: String,
    token_supply: u64,
    supply_percentage: u16,
    raise_min: u64,
    raise_max: u64,
    fdv_min: u64,
    fdv_max: u64,
    commitment_start: i64,
    commitment_end: i64,
    score_merkle_root: [u8; 32],
) -> Result<()> {
    require!(raise_min > 0, ErrorCode::InvalidRaiseRange);  
    require!(raise_min <= raise_max, ErrorCode::InvalidRaiseRange);
    require!(raise_max > 0, ErrorCode::InvalidRaiseRange);  
    require!(supply_percentage <= 10000, ErrorCode::InvalidSupplyRange);
    require!(supply_percentage > 0, ErrorCode::InvalidSupplyRange);
    require!(fdv_min <= fdv_max, ErrorCode::InvalidFdvRange);
    require!(fdv_min > 0, ErrorCode::InvalidFdvRange); 
    require!(fdv_max > 0, ErrorCode::InvalidFdvRange);  
    require!(commitment_start < commitment_end, ErrorCode::InvalidTiming);
    require!(token_supply > 0, ErrorCode::InvalidParameters);

    // Minimum 15 minute commitment window
    let commitment_duration = commitment_end
        .checked_sub(commitment_start)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(
        commitment_duration >= MIN_COMMITMENT_WINDOW_SECONDS,
        ErrorCode::CommitmentWindowTooShort
    );

    let now = Clock::get()?.unix_timestamp;
    
    require!(
        commitment_start > now,
        ErrorCode::InvalidTimestamps
    );
    require!(
        commitment_end > now,
        ErrorCode::InvalidTimestamps
    );
    
    let sale = &mut ctx.accounts.sale;
    
    sale.bump = ctx.bumps.sale;
    sale.authority = ctx.accounts.authority.key();
    sale.token_mint = ctx.accounts.token_mint.key();
    sale.usdc_mint = ctx.accounts.usdc_mint.key();
    sale.usdc_vault = ctx.accounts.usdc_vault.key();
    sale.token_vault = ctx.accounts.token_vault.key();
    sale.token_supply = token_supply;
    sale.supply_percentage = supply_percentage;
    sale.raise_min = raise_min;
    sale.raise_max = raise_max;
    sale.fdv_min = fdv_min;
    sale.fdv_max = fdv_max;
    sale.min_commitment = MIN_COMMITMENT; 
    sale.commitment_start = commitment_start;
    sale.commitment_end = commitment_end;
    sale.score_merkle_root = score_merkle_root;
    sale.name = name;
    sale.created_at = now;
    sale.status = SaleStatus::Initialized;
    sale.clearing_fdv = None;
    sale.total_cleared_commits = None;
    sale.fill_rate = None;
    sale.marginal_user = None;
    sale.marginal_allocation = None;
    sale.marginal_user_verified = false; 
    sale.proposer = None;
    sale.verification_count = 0;
    sale.total_users = 0;
    sale.total_committed = 0;
    sale.running_sum = 0;
    sale.settlement_nonce = 0;
    sale.last_proposal_time = 0;

    // Authority control flags
    sale.is_paused = false;
    sale.claims_enabled = false;
    sale.drand_reveal_round = timestamp_to_round(commitment_end);
    sale.drand_chain_hash = QUICKNET_CHAIN_HASH;
    sale.reveal_count = 0;
    sale.all_bids_revealed = false;

    msg!("Sale initialized: {:?}", sale.key());
    msg!("Drand reveal round: {}", sale.drand_reveal_round);

    emit!(SaleInitialized {
        sale: sale.key(),
        authority: ctx.accounts.authority.key(),
        token_mint: ctx.accounts.token_mint.key(),
        raise_min,
        raise_max,
    });

    Ok(())
}