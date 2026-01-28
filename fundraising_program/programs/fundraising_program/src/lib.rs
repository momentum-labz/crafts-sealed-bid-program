use anchor_lang::prelude::*;

declare_id!("962ZtGmofWxAFJCHugzBpxYgZiLUQ9qrTXMynwYo8sFX");

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

#[program]
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
        drand_reveal_round_override: Option<u64>, 
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
            drand_reveal_round_override,
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
        max_fdv_encrypted: Vec<u8>,
        drand_round: u64,
        score: u32,
        score_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        handler_commit(ctx, amount, max_fdv_encrypted, drand_round, score, score_proof)
    }

    /// Close commitment window and transition to revealing status
    pub fn close_commitment_window(ctx: Context<CloseCommitmentWindow>) -> Result<()> {
        handler_close_commitment_window(ctx)
    }

    /// Batch reveal encrypted bids using drand beacon
    pub fn batch_reveal_bids<'info>(
        ctx: Context<'_, '_, 'info, 'info, BatchRevealBids<'info>>,
        drand_round: u64,
        drand_signature: [u8; 48],
        revealed_bids: Vec<RevealedBidData>,
    ) -> Result<()> {
        handler_batch_reveal_bids(ctx, drand_round, drand_signature, revealed_bids)
    }

    /// fill_rate is in basis points: 10000 = 100%, 5000 = 50% (2x oversubscribed)
    pub fn propose_settlement(
        ctx: Context<ProposeSettlement>,
        clearing_fdv: u64,
        fill_rate: u64,
        marginal_user: Option<Pubkey>,
        marginal_allocation: Option<u64>,
    ) -> Result<()> {
        handler_propose_settlement(ctx, clearing_fdv, fill_rate, marginal_user, marginal_allocation)
    }

    /// Verify a single user's allocation against the proposed settlement
    pub fn verify_and_allocate(ctx: Context<VerifyAndAllocate>) -> Result<()> {
        handler_verify_and_allocate(ctx)
    }

    /// Batch verify and allocate up to 10 users per transaction
    pub fn batch_verify_and_allocate<'info>(
        ctx: Context<'_, '_, 'info, 'info, BatchVerifyAndAllocate<'info>>,
    ) -> Result<()> {
        handler_batch_verify_and_allocate(ctx)
    }

    /// Finalize settlement after all users are verified
    pub fn finalize_settlement(ctx: Context<FinalizeSettlement>) -> Result<()> {
        handler_finalize_settlement(ctx)
    }

    /// Cancel sale - authority only, before settlement
    pub fn cancel_sale(ctx: Context<CancelSale>) -> Result<()> {
        handler_cancel_sale(ctx)
    }

    /// Claim tokens and refund after settlement (requires claims_enabled)
    pub fn claim_allocation(ctx: Context<ClaimAllocation>) -> Result<()> {
        handler_claim_allocation(ctx)
    }

    /// Claim full refund after sale cancellation or refund mode
    pub fn claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
        handler_claim_refund(ctx)
    }

    /// Withdraw unsold tokens after settlement or cancellation - authority only
    pub fn withdraw_unsold(ctx: Context<WithdrawUnsold>) -> Result<()> {
        handler_withdraw_unsold(ctx)
    }

    /// Withdraw raised USDC after settlement - authority only
    pub fn withdraw_raised(ctx: Context<WithdrawRaised>) -> Result<()> {
        handler_withdraw_raised(ctx)
    }

    /// Enable claims after settlement - authority only
    pub fn enable_claims(ctx: Context<EnableClaims>) -> Result<()> {
        handler_enable_claims(ctx)
    }

    /// Pause the sale - authority only
    pub fn pause_sale(ctx: Context<PauseSale>) -> Result<()> {
        handler_pause_sale(ctx)
    }

    /// Unpause the sale - authority only
    pub fn unpause_sale(ctx: Context<UnpauseSale>) -> Result<()> {
        handler_unpause_sale(ctx)
    }

    /// Enable full refund mode - authority only
    pub fn refund_sale(ctx: Context<RefundSale>) -> Result<()> {
        handler_refund_sale(ctx)
    }
}
