use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Sale {
    pub authority: Pubkey,           // 32 bytes
    pub token_mint: Pubkey,          // 32 bytes
    pub usdc_mint: Pubkey,           // 32 bytes - USDC mint for validation

    pub usdc_vault: Pubkey,          // 32 bytes
    pub token_vault: Pubkey,         // 32 bytes

    pub token_supply: u64,           // 8 bytes - Total token supply
    pub supply_percentage: u16,       // 2 bytes - % to sell (in bps, e.g., 2500 = 25%)
    pub raise_min: u64,              // 8 bytes - Min USDC to raise
    pub raise_max: u64,              // 8 bytes - Max USDC to raise
    pub fdv_min: u64,                // 8 bytes - Min FDV
    pub fdv_max: u64,                // 8 bytes - Max FDV
    
    pub min_commitment: u64,         // 8 bytes - Min USDC per commitment

    pub commitment_start: i64,       // 8 bytes
    pub commitment_end: i64,         // 8 bytes

    pub score_merkle_root: [u8; 32], // 32 bytes - For score verification in commit

    pub clearing_fdv: Option<u64>,   // 9 bytes - Proposed clearing FDV
    pub total_cleared_commits: Option<u64>, // 9 bytes - Sum of cleared commitments (expected)
    pub fill_rate: Option<u64>,      // 9 bytes - Pro-rata fill rate for oversubscribed sales
    pub marginal_user: Option<Pubkey>, // 33 bytes - Marginal bidder's pubkey
    pub marginal_allocation: Option<u64>, // 9 bytes - Marginal bidder's partial allocation
    pub marginal_user_verified: bool, // 1 byte - Was marginal user verified?
    pub proposer: Option<Pubkey>,    // 33 bytes - Who proposed settlement
    pub verification_count: u32,     // 4 bytes - # of users verified
    pub total_users: u32,            // 4 bytes - Total # of commitments
    pub total_committed: u64,        // 8 bytes - Total USDC committed (for quick raise_min check)
    pub running_sum: u64,            // 8 bytes - Running sum for verification
    pub settlement_nonce: u64,       // 8 bytes
    
    pub last_proposal_time: i64,     // 8 bytes

    pub status: SaleStatus,          // 1 byte

    // Authority control flags
    pub is_paused: bool,             // 1 byte - Pause commitments
    pub claims_enabled: bool,        // 1 byte - Authority controls when claims can start

    // Drand Configuration (for sealed-bid auctions - MANDATORY in v0.2)
    pub drand_reveal_round: u64,     // 8 bytes - Drand round when bids become decryptable
    pub drand_chain_hash: [u8; 8],   // 8 bytes - Quicknet chain hash prefix for verification
    pub reveal_count: u32,           // 4 bytes - Number of bids revealed
    pub all_bids_revealed: bool,     // 1 byte - Whether all bids have been revealed

    #[max_len(100)]
    pub name: String,                // Variable
    pub created_at: i64,             // 8 bytes
    pub bump: u8,                    // 1 byte
}


#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum SaleStatus {
    Initialized = 0,
    Active = 1,
    CommitmentEnded = 2,
    Revealing = 3,
    Proposed = 4,
    Verifying = 5,
    Settled = 6,
    Finalized = 7,
    Cancelled = 8,
    Refunding = 9,
}