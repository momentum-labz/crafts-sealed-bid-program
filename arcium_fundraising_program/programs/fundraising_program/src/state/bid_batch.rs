use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RevealedBidBatch {
    pub sale: Pubkey,                    // 32 bytes
    pub batch_index: u16,                // 2 bytes
    pub bid_count: u16,                  // 2 bytes
    pub verified: bool,                  // 1 byte
    #[max_len(50)]
    pub bids: Vec<RevealedBidData>,      // Variable (50 * 52 = 2600 bytes max)
    pub bump: u8,                        // 1 byte
}


#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct RevealedBidData {
    pub bidder: Pubkey,                  // 32 bytes
    pub amount: u64,                     // 8 bytes
    pub max_fdv: u64,                    // 8 bytes
    pub score: u32,                      // 4 bytes
}

impl RevealedBidBatch {
    pub const MAX_BIDS_PER_BATCH: usize = 50;

    pub fn is_full(&self) -> bool {
        self.bids.len() >= Self::MAX_BIDS_PER_BATCH
    }

    pub fn add_bid(&mut self, bid: RevealedBidData) -> Result<()> {
        require!(
            !self.is_full(),
            crate::errors::ErrorCode::ChunkTooLarge
        );
        self.bids.push(bid);
        self.bid_count = self.bids.len() as u16;
        Ok(())
    }
}