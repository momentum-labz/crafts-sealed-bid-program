use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;
use crate::state::{Sale, SealedBid, SaleStatus, ENCRYPTED_STATE_OFFSET, ENCRYPTED_STATE_LENGTH};
use crate::errors::ErrorCode;
use crate::ArciumSignerAccount;
use crate::validate_callback_ixs;
use crate::CallbackError;
use crate::{ID, ID_CONST};

/// Byte offset of encryption_key within SealedBid account data
const BID_ENCRYPTED_OFFSET: u32 = 81;
/// Length of SharedEncryptedStruct<1>: pubkey(32) + nonce(16) + ciphertext(32) = 80
const BID_ENCRYPTED_LENGTH: u32 = 80;

/// Queue the submit_bid MPC computation to update encrypted state with a new bid
#[queue_computation_accounts("submit_bid", payer)]
#[derive(Accounts)]
pub struct QueueSubmitBid<'info> {
    #[account(
        constraint = sale.status == SaleStatus::Active @ ErrorCode::InvalidSaleStatus,
        constraint = sale.state_nonce > 0 @ ErrorCode::InvalidParameters,
    )]
    pub sale: Box<Account<'info, Sale>>,

    #[account(
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.is_initialized @ ErrorCode::InvalidParameters,
    )]
    pub sealed_bid: Box<Account<'info, SealedBid>>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(address = derive_mxe_pda!())]
    pub mxe_account: Box<Account<'info, MXEAccount>>,

    #[account(mut, address = derive_mempool_pda!(mxe_account, ErrorCode::ClusterNotSet))]
    /// CHECK: mempool account
    pub mempool_account: UncheckedAccount<'info>,

    #[account(mut, address = derive_execpool_pda!(mxe_account, ErrorCode::ClusterNotSet))]
    /// CHECK: executing pool
    pub executing_pool: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: computation account
    pub computation_account: UncheckedAccount<'info>,

    #[account(address = derive_comp_def_pda!(comp_def_offset("submit_bid")))]
    pub comp_def_account: Box<Account<'info, ComputationDefinitionAccount>>,

    #[account(mut, address = derive_cluster_pda!(mxe_account, ErrorCode::ClusterNotSet))]
    pub cluster_account: Box<Account<'info, Cluster>>,

    #[account(mut, address = ARCIUM_FEE_POOL_ACCOUNT_ADDRESS)]
    pub pool_account: Box<Account<'info, FeePool>>,

    #[account(mut, address = ARCIUM_CLOCK_ACCOUNT_ADDRESS)]
    pub clock_account: Box<Account<'info, ClockAccount>>,

    #[account(
        mut,
        seeds = [SIGN_PDA_SEED],
        bump = sign_pda_account.bump,
    )]
    pub sign_pda_account: Account<'info, ArciumSignerAccount>,

    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
}

pub fn handler_queue_submit_bid(
    ctx: Context<QueueSubmitBid>,
    computation_offset: u64,
) -> Result<()> {
    let sale = &ctx.accounts.sale;
    let bid = &ctx.accounts.sealed_bid;

    // Build args:
    // 1. encrypted_max_fdv: Enc<Shared, u64> via account reference to SealedBid
    // 2. state: Enc<Mxe, [u64; STATE_LEN]> via account reference to Sale
    // 3. amount: u64 plaintext
    // 4. fdv_min: u64 plaintext
    // 5. fdv_max: u64 plaintext
    let args = ArgBuilder::new()
        .account(bid.key(), BID_ENCRYPTED_OFFSET, BID_ENCRYPTED_LENGTH)
        .account(sale.key(), ENCRYPTED_STATE_OFFSET, ENCRYPTED_STATE_LENGTH)
        .plaintext_u64(bid.amount)
        .plaintext_u64(sale.fdv_min)
        .plaintext_u64(sale.fdv_max)
        .plaintext_u64(sale.bucket_count as u64)
        .build();

    let extra_accs = vec![
        arcium_client::idl::arcium::types::CallbackAccount {
            pubkey: sale.key(),
            is_writable: true,
        },
    ];

    queue_computation(
        ctx.accounts,
        computation_offset,
        args,
        vec![SubmitBidCallback::callback_ix(
            computation_offset,
            &ctx.accounts.mxe_account,
            &extra_accs,
        )?],
        1,
        0,
    )?;

    msg!(
        "Submit bid queued: sale={}, user={}, amount={}",
        sale.key(),
        bid.user,
        bid.amount,
    );

    Ok(())
}

/// Callback for submit_bid MPC result — updates encrypted state on Sale
#[callback_accounts("submit_bid")]
#[derive(Accounts)]
pub struct SubmitBidCallback<'info> {
    pub arcium_program: Program<'info, Arcium>,

    pub comp_def_account: Box<Account<'info, ComputationDefinitionAccount>>,

    pub mxe_account: Box<Account<'info, MXEAccount>>,

    /// CHECK: computation account verified by Arcium
    pub computation_account: UncheckedAccount<'info>,

    pub cluster_account: Box<Account<'info, Cluster>>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instructions sysvar
    pub instructions_sysvar: AccountInfo<'info>,

    #[account(mut)]
    pub sale: Box<Account<'info, Sale>>,
}

#[arcium_callback(encrypted_ix = "submit_bid")]
pub fn submit_bid_callback(
    ctx: Context<SubmitBidCallback>,
    output: SignedComputationOutputs<SubmitBidOutput>,
) -> Result<()> {
    let result = output.verify_output(
        &ctx.accounts.cluster_account,
        &ctx.accounts.computation_account,
    )?;

    let sale = &mut ctx.accounts.sale;

    // SubmitBidOutput is MXEEncryptedStruct<STATE_LEN>: updated encrypted state
    sale.state_nonce = result.field_0.nonce;
    sale.encrypted_state = result.field_0.ciphertexts;

    msg!("Bid submitted to encrypted state: sale={}", sale.key());

    Ok(())
}
