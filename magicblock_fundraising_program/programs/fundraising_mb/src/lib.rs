use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::ephemeral;

declare_id!("FRMBe2tWBoo9sTQ6v2bqLbQiSaLkMZm55N3Cnbb3CZXE");

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

#[ephemeral]
#[program]
pub mod fundraising_mb {
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
        usdc_treasury: Pubkey,
        token_treasury: Pubkey,
        per_validator: Pubkey,
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
            usdc_treasury,
            token_treasury,
            per_validator,
        )
    }

    pub fn fund_sale(
        ctx: Context<FundSale>,
        amount: u64,
    ) -> Result<()> {
        handler_fund_sale(ctx, amount)
    }

    /// Commit USDC + create SealedBid + delegate to PER
    pub fn commit_usdc(
        ctx: Context<CommitUsdc>,
        amount: u64,
        score: u32,
        score_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        handler_commit_usdc(ctx, amount, score, score_proof)
    }

    /// Delegate a SealedBid account to PER (call after commit_usdc)
    pub fn delegate_bid(ctx: Context<DelegateBid>) -> Result<()> {
        handler_delegate_bid(ctx)
    }

    /// Submit bid privately in PER (user writes max_fdv)
    pub fn submit_bid(
        ctx: Context<SubmitBid>,
        max_fdv: u64,
    ) -> Result<()> {
        handler_submit_bid(ctx, max_fdv)
    }

    /// Update bid price while commitment window is open (PER only)
    pub fn update_bid(
        ctx: Context<UpdateBidMb>,
        max_fdv: u64,
    ) -> Result<()> {
        handler_update_bid_mb(ctx, max_fdv)
    }

    /// Close commitment window + compute clearing FDV and allocations (PER, batched)
    pub fn close_and_compute<'info>(
        ctx: Context<'_, '_, 'info, 'info, CloseAndCompute<'info>>,
        clearing_fdv: u64,
        fill_rate: u64,
        marginal_user: Option<Pubkey>,
        marginal_allocation: Option<u64>,
    ) -> Result<()> {
        handler_close_and_compute(ctx, clearing_fdv, fill_rate, marginal_user, marginal_allocation)
    }

    /// Allocate batch: write allocation/refund/tokens, zero max_fdv (PER, batched)
    pub fn allocate_batch<'info>(
        ctx: Context<'_, '_, 'info, 'info, AllocateBatch<'info>>,
    ) -> Result<()> {
        handler_allocate_batch(ctx)
    }

    /// Undelegate batch: commit_and_undelegate N SealedBid accounts (PER→Mainnet)
    pub fn undelegate_batch<'info>(
        ctx: Context<'_, '_, 'info, 'info, UndelegateBatch<'info>>,
    ) -> Result<()> {
        handler_undelegate_batch(ctx)
    }

    /// Finalize settlement on mainnet — verify running_sum bounds
    pub fn finalize_settlement(ctx: Context<FinalizeSettlement>) -> Result<()> {
        handler_finalize_settlement(ctx)
    }

    /// Enable claims after settlement — authority only
    pub fn enable_claims(ctx: Context<EnableClaims>) -> Result<()> {
        handler_enable_claims(ctx)
    }

    /// Claim tokens and refund after settlement
    pub fn claim_allocation(ctx: Context<ClaimAllocation>) -> Result<()> {
        handler_claim_allocation(ctx)
    }

    /// Claim full refund after cancellation or refund mode
    pub fn claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
        handler_claim_refund(ctx)
    }

    /// Withdraw raised USDC after settlement — authority only
    pub fn withdraw_raised(ctx: Context<WithdrawRaised>) -> Result<()> {
        handler_withdraw_raised(ctx)
    }

    /// Withdraw unsold tokens — authority only
    pub fn withdraw_unsold(ctx: Context<WithdrawUnsold>) -> Result<()> {
        handler_withdraw_unsold(ctx)
    }

    /// Cancel sale — authority only
    pub fn cancel_sale(ctx: Context<CancelSale>) -> Result<()> {
        handler_cancel_sale(ctx)
    }

    /// Enable full refund mode — authority only
    pub fn refund_sale(ctx: Context<RefundSale>) -> Result<()> {
        handler_refund_sale(ctx)
    }

    /// Pause the sale — authority only
    pub fn pause_sale(ctx: Context<PauseSale>) -> Result<()> {
        handler_pause_sale(ctx)
    }

    /// Unpause the sale — authority only
    pub fn unpause_sale(ctx: Context<UnpauseSale>) -> Result<()> {
        handler_unpause_sale(ctx)
    }
}
