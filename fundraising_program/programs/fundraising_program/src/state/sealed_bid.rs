use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct SealedBid {
    pub is_initialized: bool,        // 1 byte
    pub sale: Pubkey,                // 32 bytes
    pub user: Pubkey,                // 32 bytes
    pub amount: u64,                 // 8 bytes - USDC committed

    // Bid Data (v0.2: encrypted bids only)
    #[max_len(256)]
    pub max_fdv_encrypted: Option<Vec<u8>>, // Timelock ciphertext (~256 bytes)
    pub max_fdv_plaintext: Option<u64>, // 9 bytes - After reveal (for encrypted bids)
    pub drand_round: u64,            // 8 bytes - Drand round used for encryption
    pub bid_revealed: bool,          // 1 byte - Whether bid has been revealed

    pub cleared: bool,               // 1 byte
    pub is_marginal: bool,           // 1 byte
    pub allocation: u64,             // 8 bytes - USDC used
    pub refund: u64,                 // 8 bytes - USDC returned
    pub tokens: u64,                 // 8 bytes - Tokens allocated
    pub claimed: bool,               // 1 byte
    pub verified_for_nonce: u64,     // 8 bytes
    pub commit_time: i64,            // 8 bytes
    pub bump: u8,                    // 1 byte
}


