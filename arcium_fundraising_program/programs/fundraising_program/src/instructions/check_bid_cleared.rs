use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;
use crate::state::{Sale, SealedBid, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::BidChecked;
use crate::utils::calculate_tokens;
use crate::ArciumSignerAccount;
use crate::validate_callback_ixs;
use crate::CallbackError;
use crate::{ID, ID_CONST};

/// Byte offset of encryption_key within SealedBid account data (after 8-byte discriminator)
const BID_ENCRYPTED_OFFSET: u32 = 81;
/// Length of SharedEncryptedStruct<1>: pubkey(32) + nonce(16) + 1 ciphertext(32) = 80
const BID_ENCRYPTED_LENGTH: u32 = 80;

/// Queue the check_bid_cleared MPC computation for a single bid
#[queue_computation_accounts("check_bid_cleared", payer)]
#[derive(Accounts)]
pub struct QueueCheckBid<'info> {
    #[account(
        constraint = (
            sale.status == SaleStatus::Settled ||
            sale.status == SaleStatus::CheckingBids
        ) @ ErrorCode::InvalidSaleStatus,
    )]
    pub sale: Box<Account<'info, Sale>>,

    #[account(
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
        constraint = sealed_bid.is_initialized @ ErrorCode::InvalidParameters,
        constraint = !sealed_bid.bid_checked @ ErrorCode::AlreadyProcessed,
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

    #[account(address = derive_comp_def_pda!(comp_def_offset("check_bid_cleared")))]
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

pub fn handler_queue_check_bid(
    ctx: Context<QueueCheckBid>,
    computation_offset: u64,
) -> Result<()> {
    let sale = &ctx.accounts.sale;
    let bid = &ctx.accounts.sealed_bid;

    let clearing_bucket_low = sale.clearing_bucket_low
        .ok_or(ErrorCode::ArciumSettlementNotReceived)?;
    let clearing_bucket_high = sale.clearing_bucket_high
        .ok_or(ErrorCode::ArciumSettlementNotReceived)?;

    // Build args: Enc<Shared, u64> via account reference + plaintext clearing bucket bounds
    let args = ArgBuilder::new()
        .account(bid.key(), BID_ENCRYPTED_OFFSET, BID_ENCRYPTED_LENGTH)
        .plaintext_u64(clearing_bucket_low)
        .plaintext_u64(clearing_bucket_high)
        .build();

    // Callback needs sale + sealed_bid accounts
    let extra_accs = vec![
        arcium_client::idl::arcium::types::CallbackAccount {
            pubkey: sale.key(),
            is_writable: true,
        },
        arcium_client::idl::arcium::types::CallbackAccount {
            pubkey: bid.key(),
            is_writable: true,
        },
    ];

    queue_computation(
        ctx.accounts,
        computation_offset,
        args,
        vec![CheckBidClearedCallback::callback_ix(
            computation_offset,
            &ctx.accounts.mxe_account,
            &extra_accs,
        )?],
        1,
        0,
    )?;

    msg!(
        "Check bid queued: sale={}, user={}",
        sale.key(),
        bid.user,
    );

    Ok(())
}

/// Callback for check_bid_cleared MPC result
#[callback_accounts("check_bid_cleared")]
#[derive(Accounts)]
pub struct CheckBidClearedCallback<'info> {
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

    #[account(
        mut,
        constraint = sealed_bid.sale == sale.key() @ ErrorCode::InvalidParameters,
    )]
    pub sealed_bid: Box<Account<'info, SealedBid>>,
}

#[arcium_callback(encrypted_ix = "check_bid_cleared")]
pub fn check_bid_cleared_callback(
    ctx: Context<CheckBidClearedCallback>,
    output: SignedComputationOutputs<CheckBidClearedOutput>,
) -> Result<()> {
    let result = output.verify_output(
        &ctx.accounts.cluster_account,
        &ctx.accounts.computation_account,
    )?;

    let sale = &mut ctx.accounts.sale;
    let bid = &mut ctx.accounts.sealed_bid;

    let bid_status: u8 = result.field_0 as u8;
    bid.bid_status = bid_status;
    bid.bid_checked = true;

    // Calculate allocation based on bid status (0=below, 1=marginal, 2=above)
    let clearing_fdv = sale.clearing_fdv.ok_or(ErrorCode::ArciumSettlementNotReceived)?;
    let marginal_fill_rate = sale.marginal_fill_rate.ok_or(ErrorCode::ArciumSettlementNotReceived)?;

    if bid_status == 2 {
        // Above clearing bucket: 100% fill
        let allocation = bid.amount;
        let tokens = calculate_tokens(allocation, sale.token_supply, clearing_fdv)?;

        if tokens == 0 {
            bid.allocation = 0;
            bid.refund = bid.amount;
            bid.tokens = 0;
            bid.bid_status = 0;
        } else {
            bid.allocation = allocation;
            bid.refund = 0;
            bid.tokens = tokens;
        }
    } else if bid_status == 1 {
        // Marginal bucket: apply marginal fill rate
        let allocation = (bid.amount as u128)
            .checked_mul(marginal_fill_rate as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::DivisionByZero)? as u64;

        let tokens = calculate_tokens(allocation, sale.token_supply, clearing_fdv)?;

        if tokens == 0 {
            bid.allocation = 0;
            bid.refund = bid.amount;
            bid.tokens = 0;
            bid.bid_status = 0;
        } else {
            bid.allocation = allocation;
            bid.refund = bid.amount.saturating_sub(allocation);
            bid.tokens = tokens;
        }
    } else {
        // Below clearing bucket: full refund
        bid.allocation = 0;
        bid.refund = bid.amount;
        bid.tokens = 0;
    }

    sale.bids_checked = sale.bids_checked.checked_add(1).unwrap_or(sale.bids_checked);

    if sale.status == SaleStatus::Settled {
        sale.status = SaleStatus::CheckingBids;
    }

    msg!(
        "Bid checked: user={}, bid_status={}, allocation={}, tokens={}, refund={}",
        bid.user,
        bid_status,
        bid.allocation,
        bid.tokens,
        bid.refund,
    );

    emit!(BidChecked {
        sale: sale.key(),
        user: bid.user,
        bid_status,
        allocation: bid.allocation,
        tokens: bid.tokens,
    });

    Ok(())
}
