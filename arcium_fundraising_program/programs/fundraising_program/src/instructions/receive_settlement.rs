use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;
use crate::state::{Sale, SaleStatus};
use crate::errors::ErrorCode;
use crate::events::SettlementReceived;
use crate::validate_callback_ixs;
use crate::CallbackError;
use crate::{ID, ID_CONST};

#[callback_accounts("settle_auction")]
#[derive(Accounts)]
pub struct SettleAuctionCallback<'info> {
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

#[arcium_callback(encrypted_ix = "settle_auction")]
pub fn settle_auction_callback(
    ctx: Context<SettleAuctionCallback>,
    output: SignedComputationOutputs<SettleAuctionOutput>,
) -> Result<()> {
    let result = output.verify_output(
        &ctx.accounts.cluster_account,
        &ctx.accounts.computation_account,
    )?;

    let sale = &mut ctx.accounts.sale;

    require!(
        sale.status == SaleStatus::Settling,
        ErrorCode::InvalidSaleStatus
    );

    // SettleAuctionOutput wraps a tuple struct: field_0 = SettleAuctionOutputStruct0
    let settlement = &result.field_0;
    sale.clearing_fdv = Some(settlement.field_0);
    sale.marginal_fill_rate = Some(settlement.field_1);
    sale.clearing_bucket_low = Some(settlement.field_2);
    sale.clearing_bucket_high = Some(settlement.field_3);
    sale.total_raised = Some(settlement.field_4);
    // settlement.field_5 is bid_count (informational)

    sale.arcium_settled = true;
    sale.status = SaleStatus::Settled;
    sale.settlement_nonce = sale.settlement_nonce.checked_add(1).unwrap_or(1);

    msg!(
        "Settlement received: sale={}, clearing_fdv={}, marginal_fill_rate_bps={}, total_raised={}",
        sale.key(),
        settlement.field_0,
        settlement.field_1,
        settlement.field_4,
    );

    emit!(SettlementReceived {
        sale: sale.key(),
        clearing_fdv: settlement.field_0,
        marginal_fill_rate_bps: settlement.field_1,
        clearing_bucket_low: settlement.field_2,
        clearing_bucket_high: settlement.field_3,
        total_raised: settlement.field_4,
    });

    Ok(())
}
