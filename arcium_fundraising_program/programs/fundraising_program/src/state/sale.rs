use anchor_lang::prelude::*;

/// Maximum number of demand buckets for encrypted auction state
pub const MAX_BUCKET_COUNT: usize = 100;
/// Total encrypted state slots: MAX_BUCKET_COUNT buckets + bid_count + total_demand + 3 reserved
pub const STATE_LEN: usize = MAX_BUCKET_COUNT + 5;
/// Byte offset of state_nonce within Sale account data (after 8-byte discriminator)
/// Fields before state_nonce:
///   authority(32) + token_mint(32) + usdc_mint(32) + usdc_vault(32) + token_vault(32)
///   + token_supply(8) + supply_percentage(2) + raise_min(8) + raise_max(8)
///   + fdv_min(8) + fdv_max(8) + min_commitment(8) + bucket_count(2)
///   + commitment_start(8) + commitment_end(8) + score_merkle_root(32)
///   + clearing_fdv(9) + marginal_fill_rate(9) + clearing_bucket_low(9) + clearing_bucket_high(9) + total_raised(9)
///   + total_users(4) + total_committed(8) + settlement_nonce(8)
///   + status(1) + is_paused(1) + claims_enabled(1)
///   = 32*5 + 8 + 2 + 8*3 + 8*2 + 8 + 2 + 8*2 + 32 + 9*5 + 4 + 8 + 8 + 1 + 1 + 1
///   = 160 + 8 + 2 + 24 + 16 + 8 + 2 + 16 + 32 + 45 + 4 + 8 + 8 + 1 + 1 + 1 = 336
pub const ENCRYPTED_STATE_OFFSET: u32 = 336;
/// Byte length of MXE encrypted state: u128 nonce (16) + STATE_LEN * 32 ciphertexts
pub const ENCRYPTED_STATE_LENGTH: u32 = 16 + (STATE_LEN as u32) * 32;

#[account]
#[derive(InitSpace)]
pub struct Sale {
    pub authority: Pubkey,           // 32 bytes
    pub token_mint: Pubkey,          // 32 bytes
    pub usdc_mint: Pubkey,           // 32 bytes

    pub usdc_vault: Pubkey,          // 32 bytes
    pub token_vault: Pubkey,         // 32 bytes

    pub token_supply: u64,           // 8 bytes - Total token supply
    pub supply_percentage: u16,      // 2 bytes - % to sell (in bps, e.g., 2500 = 25%)
    pub raise_min: u64,              // 8 bytes - Min USDC to raise
    pub raise_max: u64,              // 8 bytes - Max USDC to raise
    pub fdv_min: u64,                // 8 bytes - Min FDV
    pub fdv_max: u64,                // 8 bytes - Max FDV

    pub min_commitment: u64,         // 8 bytes - Min USDC per commitment
    pub bucket_count: u16,           // 2 bytes - Number of demand buckets (1..=200)

    pub commitment_start: i64,       // 8 bytes
    pub commitment_end: i64,         // 8 bytes

    pub score_merkle_root: [u8; 32], // 32 bytes - For score verification in commit

    // Settlement results (set by Arcium callback)
    pub clearing_fdv: Option<u64>,          // 9 bytes
    pub marginal_fill_rate: Option<u64>,    // 9 bytes - In basis points (10000 = 100%) for marginal bucket
    pub clearing_bucket_low: Option<u64>,   // 9 bytes - Low bound of clearing bucket
    pub clearing_bucket_high: Option<u64>,  // 9 bytes - High bound of clearing bucket
    pub total_raised: Option<u64>,          // 9 bytes

    pub total_users: u32,            // 4 bytes - Total # of commitments
    pub total_committed: u64,        // 8 bytes - Total USDC committed
    pub settlement_nonce: u64,       // 8 bytes

    pub status: SaleStatus,          // 1 byte

    // Authority control flags
    pub is_paused: bool,             // 1 byte
    pub claims_enabled: bool,        // 1 byte

    // Arcium encrypted auction state (MXEEncryptedStruct<STATE_LEN>: nonce + ciphertexts)
    // Must be contiguous: nonce then ciphertexts, for account() reference
    pub state_nonce: u128,           // 16 bytes - Nonce for MXE encryption
    pub encrypted_state: [[u8; 32]; STATE_LEN],  // STATE_LEN * 32 bytes

    // Track how many bids have been checked post-settlement
    pub bids_checked: u32,           // 4 bytes
    pub arcium_settled: bool,        // 1 byte - Whether Arcium settlement callback received

    #[max_len(100)]
    pub name: String,
    pub created_at: i64,             // 8 bytes
    pub bump: u8,                    // 1 byte
}


#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum SaleStatus {
    Initialized = 0,
    Active = 1,
    CommitmentEnded = 2,
    Ready = 3,          // Commitment window closed, ready for settlement
    Settling = 4,       // Settlement MPC in progress
    Settled = 5,        // Settlement complete, checking bids
    CheckingBids = 6,   // Batch checking individual bids
    Finalized = 7,      // All bids checked, claims enabled by authority
    Cancelled = 8,
    Refunding = 9,
}
