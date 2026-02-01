use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;
use crate::{ID, ID_CONST};

// ---- init_auction_state ----
#[init_computation_definition_accounts("init_auction_state", payer)]
#[derive(Accounts)]
pub struct InitInitAuctionStateCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, address = derive_mxe_pda!())]
    pub mxe_account: Box<Account<'info, MXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program
    pub comp_def_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: address_lookup_table
    pub address_lookup_table: UncheckedAccount<'info>,
    /// CHECK: lut_program
    pub lut_program: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

pub fn handler_init_init_auction_state_comp_def(
    ctx: Context<InitInitAuctionStateCompDef>,
) -> Result<()> {
    init_comp_def(ctx.accounts, None, None)
}

// ---- submit_bid ----
#[init_computation_definition_accounts("submit_bid", payer)]
#[derive(Accounts)]
pub struct InitSubmitBidCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, address = derive_mxe_pda!())]
    pub mxe_account: Box<Account<'info, MXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program
    pub comp_def_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: address_lookup_table
    pub address_lookup_table: UncheckedAccount<'info>,
    /// CHECK: lut_program
    pub lut_program: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

pub fn handler_init_submit_bid_comp_def(
    ctx: Context<InitSubmitBidCompDef>,
) -> Result<()> {
    init_comp_def(ctx.accounts, None, None)
}

// ---- settle_auction ----
#[init_computation_definition_accounts("settle_auction", payer)]
#[derive(Accounts)]
pub struct InitSettleAuctionCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, address = derive_mxe_pda!())]
    pub mxe_account: Box<Account<'info, MXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program
    pub comp_def_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: address_lookup_table
    pub address_lookup_table: UncheckedAccount<'info>,
    /// CHECK: lut_program
    pub lut_program: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

pub fn handler_init_settle_auction_comp_def(
    ctx: Context<InitSettleAuctionCompDef>,
) -> Result<()> {
    init_comp_def(ctx.accounts, None, None)
}

// ---- check_bid_cleared ----
#[init_computation_definition_accounts("check_bid_cleared", payer)]
#[derive(Accounts)]
pub struct InitCheckBidClearedCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, address = derive_mxe_pda!())]
    pub mxe_account: Box<Account<'info, MXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program
    pub comp_def_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: address_lookup_table
    pub address_lookup_table: UncheckedAccount<'info>,
    /// CHECK: lut_program
    pub lut_program: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

pub fn handler_init_check_bid_cleared_comp_def(
    ctx: Context<InitCheckBidClearedCompDef>,
) -> Result<()> {
    init_comp_def(ctx.accounts, None, None)
}
