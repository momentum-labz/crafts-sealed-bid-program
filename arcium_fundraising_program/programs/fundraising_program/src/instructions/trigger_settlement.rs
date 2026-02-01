use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;
use crate::state::{Sale, SaleStatus, ENCRYPTED_STATE_OFFSET, ENCRYPTED_STATE_LENGTH};
use crate::errors::ErrorCode;
use crate::events::SettlementTriggered;
use crate::ArciumSignerAccount;
use super::receive_settlement::SettleAuctionCallback;
use crate::{ID, ID_CONST};

#[queue_computation_accounts("settle_auction", payer)]
#[derive(Accounts)]
pub struct TriggerSettlement<'info> {
    #[account(
        mut,
        constraint = sale.status == SaleStatus::Ready @ ErrorCode::InvalidSaleStatus,
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

    #[account(address = derive_comp_def_pda!(comp_def_offset("settle_auction")))]
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

pub fn handler_trigger_settlement(
    ctx: Context<TriggerSettlement>,
    computation_offset: u64,
) -> Result<()> {
    // Read sale fields before borrowing ctx.accounts for queue_computation
    let sale_key = ctx.accounts.sale.key();
    let args = ArgBuilder::new()
        .account(sale_key, ENCRYPTED_STATE_OFFSET, ENCRYPTED_STATE_LENGTH)
        .plaintext_u64(ctx.accounts.sale.fdv_min)
        .plaintext_u64(ctx.accounts.sale.fdv_max)
        .plaintext_u64(ctx.accounts.sale.token_supply)
        .plaintext_u64(ctx.accounts.sale.supply_percentage as u64)
        .plaintext_u64(ctx.accounts.sale.bucket_count as u64)
        .build();

    let extra_accs = vec![
        arcium_client::idl::arcium::types::CallbackAccount {
            pubkey: sale_key,
            is_writable: true,
        },
    ];

    queue_computation(
        ctx.accounts,
        computation_offset,
        args,
        vec![SettleAuctionCallback::callback_ix(
            computation_offset,
            &ctx.accounts.mxe_account,
            &extra_accs,
        )?],
        1,
        0,
    )?;

    ctx.accounts.sale.status = SaleStatus::Settling;

    msg!("Settlement triggered: sale={}", sale_key);

    emit!(SettlementTriggered {
        sale: sale_key,
    });

    Ok(())
}
