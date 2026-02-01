use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;


declare_id!("FbujbtVcr2jYXAPMR57V6bCffq87PJj55Pgz8pMymEDS");

pub mod instructions;
pub mod state;
pub mod errors;
pub mod utils;
pub mod events;

pub use instructions::*;
pub use state::*;
#[allow(unused_imports)]
pub use errors::*;
pub use utils::*;
pub use events::*;

#[arcium_program]
pub mod fundraising_program {
    use super::*;

    pub fn initialize_sale(
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
        bucket_count: u16,
    ) -> Result<()> {
        handler_initialize_sale(
            ctx,
            name,
            token_supply,
            supply_percentage,
            raise_min,
            raise_max,
            fdv_min,
            fdv_max,
            commitment_start,
            commitment_end,
            score_merkle_root,
            bucket_count,
        )
    }

    pub fn fund_sale(
        ctx: Context<FundSale>,
        amount: u64,
    ) -> Result<()> {
        handler_fund_sale(ctx, amount)
    }

    pub fn commit(
        ctx: Context<Commit>,
        amount: u64,
        encrypted_max_fdv: [u8; 32],
        bid_nonce: u128,
        encryption_key: [u8; 32],
        score: u32,
        score_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        handler_commit(ctx, amount, encrypted_max_fdv, bid_nonce, encryption_key, score, score_proof)
    }

    pub fn close_commitment_window(ctx: Context<CloseCommitmentWindow>) -> Result<()> {
        handler_close_commitment_window(ctx)
    }

    /// Queue init_auction_state MPC to create initial encrypted state
    pub fn queue_init_state(
        ctx: Context<QueueInitState>,
        computation_offset: u64,
    ) -> Result<()> {
        handler_queue_init_state(ctx, computation_offset)
    }

    /// Callback from Arcium: receive initial encrypted state
    pub fn init_auction_state_callback(
        ctx: Context<InitAuctionStateCallback>,
        output: SignedComputationOutputs<InitAuctionStateOutput>,
    ) -> Result<()> {
        instructions::queue_init_state::init_auction_state_callback(ctx, output)
    }

    /// Queue submit_bid MPC to add a bid to encrypted state
    pub fn queue_submit_bid(
        ctx: Context<QueueSubmitBid>,
        computation_offset: u64,
    ) -> Result<()> {
        handler_queue_submit_bid(ctx, computation_offset)
    }

    /// Callback from Arcium: receive updated encrypted state after bid
    pub fn submit_bid_callback(
        ctx: Context<SubmitBidCallback>,
        output: SignedComputationOutputs<SubmitBidOutput>,
    ) -> Result<()> {
        instructions::queue_submit_bid::submit_bid_callback(ctx, output)
    }

    /// Trigger Arcium MPC settlement computation
    pub fn trigger_settlement(
        ctx: Context<TriggerSettlement>,
        computation_offset: u64,
    ) -> Result<()> {
        handler_trigger_settlement(ctx, computation_offset)
    }

    /// Callback from Arcium: receive settlement results
    pub fn settle_auction_callback(
        ctx: Context<SettleAuctionCallback>,
        output: SignedComputationOutputs<SettleAuctionOutput>,
    ) -> Result<()> {
        instructions::receive_settlement::settle_auction_callback(ctx, output)
    }

    /// Queue check for a single bid via Arcium MPC
    pub fn queue_check_bid(
        ctx: Context<QueueCheckBid>,
        computation_offset: u64,
    ) -> Result<()> {
        handler_queue_check_bid(ctx, computation_offset)
    }

    /// Callback from Arcium: receive bid check result
    pub fn check_bid_cleared_callback(
        ctx: Context<CheckBidClearedCallback>,
        output: SignedComputationOutputs<CheckBidClearedOutput>,
    ) -> Result<()> {
        instructions::check_bid_cleared::check_bid_cleared_callback(ctx, output)
    }

    // ---- Comp def initializers ----

    pub fn init_init_auction_state_comp_def(
        ctx: Context<InitInitAuctionStateCompDef>,
    ) -> Result<()> {
        handler_init_init_auction_state_comp_def(ctx)
    }

    pub fn init_submit_bid_comp_def(
        ctx: Context<InitSubmitBidCompDef>,
    ) -> Result<()> {
        handler_init_submit_bid_comp_def(ctx)
    }

    pub fn init_settle_auction_comp_def(
        ctx: Context<InitSettleAuctionCompDef>,
    ) -> Result<()> {
        handler_init_settle_auction_comp_def(ctx)
    }

    pub fn init_check_bid_cleared_comp_def(
        ctx: Context<InitCheckBidClearedCompDef>,
    ) -> Result<()> {
        handler_init_check_bid_cleared_comp_def(ctx)
    }

    // ---- Existing instructions ----

    pub fn cancel_sale(ctx: Context<CancelSale>) -> Result<()> {
        handler_cancel_sale(ctx)
    }

    pub fn claim_allocation(ctx: Context<ClaimAllocation>) -> Result<()> {
        handler_claim_allocation(ctx)
    }

    pub fn claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
        handler_claim_refund(ctx)
    }

    pub fn withdraw_unsold(ctx: Context<WithdrawUnsold>) -> Result<()> {
        handler_withdraw_unsold(ctx)
    }

    pub fn withdraw_raised(ctx: Context<WithdrawRaised>) -> Result<()> {
        handler_withdraw_raised(ctx)
    }

    pub fn enable_claims(ctx: Context<EnableClaims>) -> Result<()> {
        handler_enable_claims(ctx)
    }

    pub fn pause_sale(ctx: Context<PauseSale>) -> Result<()> {
        handler_pause_sale(ctx)
    }

    pub fn unpause_sale(ctx: Context<UnpauseSale>) -> Result<()> {
        handler_unpause_sale(ctx)
    }

    pub fn refund_sale(ctx: Context<RefundSale>) -> Result<()> {
        handler_refund_sale(ctx)
    }
}
