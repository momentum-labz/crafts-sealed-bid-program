use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct SealedBid {
    pub is_initialized: bool,        // 1 byte
    pub sale: Pubkey,                // 32 bytes
    pub user: Pubkey,                // 32 bytes
    pub amount: u64,                 // 8 bytes - USDC committed

    // Arcium encrypted max_fdv (Enc<Shared, u64>)
    pub encryption_key: [u8; 32],    // 32 bytes - User's x25519 public key
    pub bid_nonce: u128,             // 16 bytes - Nonce used for Shared encryption
    pub encrypted_max_fdv: [u8; 32], // 32 bytes - Single ciphertext

    // Settlement results (set by check_bid_cleared callback)
    pub bid_status: u8,              // 1 byte - 0=below, 1=marginal, 2=above
    pub allocation: u64,             // 8 bytes - USDC used
    pub refund: u64,                 // 8 bytes - USDC returned
    pub tokens: u64,                 // 8 bytes - Tokens allocated
    pub claimed: bool,               // 1 byte
    pub bid_checked: bool,           // 1 byte - Whether bid has been checked by MPC
    pub commit_time: i64,            // 8 bytes
    pub bump: u8,                    // 1 byte
}
