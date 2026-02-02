use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Sale {
    pub authority: Pubkey,           // 32 bytes
    pub token_mint: Pubkey,          // 32 bytes
    pub usdc_mint: Pubkey,           // 32 bytes

    pub usdc_vault: Pubkey,          // 32 bytes
    pub token_vault: Pubkey,         // 32 bytes

    pub usdc_treasury: Pubkey,       // 32 bytes - Fixed destination for withdraw_raised
    pub token_treasury: Pubkey,      // 32 bytes - Fixed destination for withdraw_unsold

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

    // Settlement state
    pub clearing_fdv: Option<u64>,   // 9 bytes
    pub total_cleared_commits: Option<u64>, // 9 bytes
    pub fill_rate: Option<u64>,      // 9 bytes - Pro-rata fill rate (basis points)
    pub marginal_user: Option<Pubkey>, // 33 bytes
    pub marginal_allocation: Option<u64>, // 9 bytes
    pub marginal_user_verified: bool, // 1 byte
    pub verification_count: u32,     // 4 bytes
    pub total_users: u32,            // 4 bytes
    pub total_committed: u64,        // 8 bytes
    pub running_sum: u64,            // 8 bytes

    pub status: SaleStatus,          // 1 byte

    // Authority control flags
    pub is_paused: bool,             // 1 byte
    pub claims_enabled: bool,        // 1 byte

    // MagicBlock PER configuration (replaces drand fields)
    pub per_validator: Pubkey,       // 32 bytes - PER validator to delegate to
    pub settlement_computed: bool,   // 1 byte - Settlement computed in PER
    pub bids_zeroed: bool,           // 1 byte - All max_fdv values zeroed before undelegate

    #[max_len(100)]
    pub name: String,                // Variable
    pub created_at: i64,             // 8 bytes
    pub bump: u8,                    // 1 byte
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum SaleStatus {
    Initialized = 0,
    Active = 1,
    Delegated = 2,       // SealedBids delegated to PER
    Computing = 3,       // Settlement being computed in PER
    Allocated = 4,       // Allocations written, bids zeroed
    Settled = 5,         // All accounts undelegated, settlement verified
    Finalized = 6,
    Cancelled = 7,
    Refunding = 8,
}
