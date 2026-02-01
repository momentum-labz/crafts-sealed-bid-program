use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct BidStorage {
    pub sale: Pubkey,
    pub chunk_index: u32,
    pub total_chunks: u32,

    #[max_len(1000)]  
    pub bids: Vec<RevealedBid>,

    pub is_finalized: bool, 
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct RevealedBid {
    pub user: Pubkey,
    pub amount: u64,
    pub max_fdv: u64,
}