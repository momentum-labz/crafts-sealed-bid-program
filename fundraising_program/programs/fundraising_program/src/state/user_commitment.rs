use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserCommitment {
    pub sale: Pubkey,                // 32 bytes
    pub user: Pubkey,                // 32 bytes

    pub amount: u64,                 // 8 bytes - USDC committed

    #[max_len(256)]
    pub max_fdv_encrypted: Option<Vec<u8>>, // Variable - Encrypted bid (V0.2+)
    pub encryption_pubkey: Option<Pubkey>,  // 33 bytes
    pub max_fdv_plaintext: Option<u64>,     // 9 bytes - After reveal (V0.1: visible bids)

    pub cleared: bool,               // 1 byte - Passed clearing?
    pub is_marginal: bool,           // 1 byte - Is marginal bidder?
    pub allocation: u64,             // 8 bytes - USDC used
    pub refund: u64,                 // 8 bytes - USDC refunded
    pub tokens: u64,                 // 8 bytes - Tokens allocated
    pub claimed: bool,               // 1 byte - Claimed?

    pub commit_time: i64,            // 8 bytes
    pub bump: u8,                    // 1 byte
}