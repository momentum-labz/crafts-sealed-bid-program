use anchor_lang::prelude::*;

#[event]
pub struct SaleInitialized {
    pub sale: Pubkey,
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub raise_min: u64,
    pub raise_max: u64,
}

#[event]
pub struct SaleFunded {
    pub sale: Pubkey,
    pub amount: u64,
}

#[event]
pub struct CommitmentMade {
    pub sale: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub total_amount: u64,
    pub max_fdv: u64,
}

#[event]
pub struct SettlementProposed {
    pub sale: Pubkey,
    pub clearing_fdv: u64,
    pub fill_rate: u64,
    pub settlement_nonce: u64,
}

#[event]
pub struct UserVerified {
    pub sale: Pubkey,
    pub user: Pubkey,
    pub cleared: bool,
    pub allocation: u64,
    pub tokens: u64,
}

#[event]
pub struct SettlementFinalized {
    pub sale: Pubkey,
    pub total_raised: u64,
    pub total_users: u32,
}

#[event]
pub struct ClaimsEnabled {
    pub sale: Pubkey,
}

#[event]
pub struct ClaimCompleted {
    pub sale: Pubkey,
    pub user: Pubkey,
    pub tokens: u64,
    pub refund: u64,
}

#[event]
pub struct RefundClaimed {
    pub sale: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
}

#[event]
pub struct SalePaused {
    pub sale: Pubkey,
}

#[event]
pub struct SaleUnpaused {
    pub sale: Pubkey,
}

#[event]
pub struct SaleCancelled {
    pub sale: Pubkey,
}

#[event]
pub struct RefundModeEnabled {
    pub sale: Pubkey,
}

#[event]
pub struct UnsoldWithdrawn {
    pub sale: Pubkey,
    pub amount: u64,
}

#[event]
pub struct RaisedWithdrawn {
    pub sale: Pubkey,
    pub amount: u64,
}

#[event]
pub struct CommitmentWindowClosed {
    pub sale: Pubkey,
    pub total_users: u32,
    pub total_committed: u64,
}

#[event]
pub struct BidsRevealed {
    pub sale: Pubkey,
    pub batch_size: u32,
    pub total_revealed: u32,
    pub all_revealed: bool,
}
