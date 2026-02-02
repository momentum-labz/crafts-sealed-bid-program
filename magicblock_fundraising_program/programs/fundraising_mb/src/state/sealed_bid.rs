use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct SealedBid {
    pub is_initialized: bool,        // 1 byte
    pub sale: Pubkey,                // 32 bytes
    pub user: Pubkey,                // 32 bytes
    pub amount: u64,                 // 8 bytes - USDC committed

    // Bid value — written privately in PER, zeroed before undelegate
    pub max_fdv_plaintext: Option<u64>, // 9 bytes
    pub bid_submitted: bool,         // 1 byte - Whether bid has been submitted in PER

    // Allocation results (written by PER during settlement)
    pub cleared: bool,               // 1 byte
    pub is_marginal: bool,           // 1 byte
    pub allocation: u64,             // 8 bytes - USDC used
    pub refund: u64,                 // 8 bytes - USDC returned
    pub tokens: u64,                 // 8 bytes - Tokens allocated
    pub claimed: bool,               // 1 byte

    pub commit_time: i64,            // 8 bytes
    pub bump: u8,                    // 1 byte
}
