use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::ArciumSignerAccount;
use crate::validate_callback_ixs;
use crate::CallbackError;
use crate::{ID, ID_CONST};

/// Queue the init_auction_state MPC computation to create initial encrypted state
#[queue_computation_accounts("init_auction_state", payer)]
#[derive(Accounts)]
pub struct QueueInitState<'info> {
    #[account(
        mut,
        constraint = sale.status == SaleStatus::Active @ ErrorCode::InvalidSaleStatus,
        constraint = sale.state_nonce == 0 @ ErrorCode::AlreadyProcessed,
    )]
    pub sale: Box<Account<'info, Sale>>,

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

    #[account(address = derive_comp_def_pda!(comp_def_offset("init_auction_state")))]
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

pub fn handler_queue_init_state(
    ctx: Context<QueueInitState>,
    computation_offset: u64,
) -> Result<()> {
    let sale = &ctx.accounts.sale;

    // No inputs needed for init_auction_state
    let args = ArgBuilder::new().build();

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
        vec![InitAuctionStateCallback::callback_ix(
            computation_offset,
            &ctx.accounts.mxe_account,
            &extra_accs,
        )?],
        1,
        0,
    )?;

    msg!("Init auction state queued: sale={}", sale.key());

    Ok(())
}

/// Callback for init_auction_state MPC result
#[callback_accounts("init_auction_state")]
#[derive(Accounts)]
pub struct InitAuctionStateCallback<'info> {
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

#[arcium_callback(encrypted_ix = "init_auction_state")]
pub fn init_auction_state_callback(
    ctx: Context<InitAuctionStateCallback>,
    output: SignedComputationOutputs<InitAuctionStateOutput>,
) -> Result<()> {
    let result = output.verify_output(
        &ctx.accounts.cluster_account,
        &ctx.accounts.computation_account,
    )?;

    let sale = &mut ctx.accounts.sale;

    // InitAuctionStateOutput is MXEEncryptedStruct<STATE_LEN>: nonce + STATE_LEN ciphertexts
    sale.state_nonce = result.field_0.nonce;
    sale.encrypted_state = result.field_0.ciphertexts;

    msg!("Auction state initialized: sale={}", sale.key());

    Ok(())
}
