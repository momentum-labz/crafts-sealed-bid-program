use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    // Range validation errors
    #[msg("Invalid raise range: min must be <= max")]
    InvalidRaiseRange,
    
    #[msg("Invalid supply range: min must be <= max")]
    InvalidSupplyRange,
    
    #[msg("Invalid FDV range: min must be <= max")]
    InvalidFdvRange,
    
    #[msg("Invalid timing: start must be < end")]
    InvalidTiming,

    #[msg("Commitment window must be at least 15 minutes")]
    CommitmentWindowTooShort,

    #[msg("Invalid timestamps")]
    InvalidTimestamps,
    
    #[msg("Invalid parameters")]
    InvalidParameters,
    
    // Sale state errors
    #[msg("Sale is not active")]
    SaleNotActive,
    
    #[msg("Sale not initialized")]
    SaleNotInitialized,
    
    #[msg("Sale not settled")]
    SaleNotSettled,
    
    #[msg("Sale not in proposed state")]
    SaleNotProposed,
    
    #[msg("Sale is not in correct status for this operation")]
    InvalidSaleStatus,
    
    #[msg("Outside commitment window")]
    OutsideCommitmentWindow,
    
    #[msg("Commitment window closed")]
    CommitmentWindowClosed,
    
    #[msg("Commitment window has not ended yet")]
    CommitmentWindowNotEnded,
    
    #[msg("Sale has not been cancelled")]
    SaleNotCancelled,
    
    // Validation errors
    #[msg("Invalid amount")]
    InvalidAmount,
    
    #[msg("Insufficient funds")]
    InsufficientFunds,
    
    #[msg("Insufficient tokens")]
    InsufficientTokens,
    
    #[msg("Invalid score")]
    InvalidScore,
    
    // Merkle proof errors
    #[msg("Invalid merkle proof")]
    InvalidMerkleProof,
    
    #[msg("Invalid score proof")]
    InvalidScoreProof,
    
    // Arcium errors
    #[msg("Invalid Arcium proof")]
    InvalidArciumProof,
    
    // Clearing errors
    #[msg("No clearing price found")]
    NoClearingPriceFound,
    
    #[msg("Insufficient demand")]
    InsufficientDemand,
    
    #[msg("Invalid clearing FDV - must be within sale FDV range")]
    InvalidClearingFdv,
    
    #[msg("Invalid marginal allocation")]
    InvalidMarginalAllocation,

    #[msg("Invalid fill rate - must be between 1 and 10000 basis points")]
    InvalidFillRate,

    // Processing errors
    #[msg("Already processed")]
    AlreadyProcessed,
    
    #[msg("Already verified")]
    AlreadyVerified,
    
    #[msg("Wrong status")]
    WrongStatus,
    
    // Optimistic settlement errors
    #[msg("Verification incomplete - not all users verified")]
    VerificationIncomplete,
    
    #[msg("Invalid running sum - settlement proposal incorrect")]
    InvalidRunningSum,

    #[msg("Running sum exceeds raise_max - over-allocation detected")]
    RunningSumExceedsMax,

    #[msg("Sale undersubscribed - running_sum below raise_min. Users can claim refunds via claim_refund")]
    RunningSumBelowMin,

    #[msg("Running sum does not match expected value based on fill_rate")]
    RunningSumMismatch,

    #[msg("Settlement already proposed")]
    SettlementAlreadyProposed,
    
    // Claim errors
    #[msg("Already claimed")]
    AlreadyClaimed,
    
    // Cancellation errors
    #[msg("Cannot cancel settled sale")]
    CannotCancelSettledSale,
    
    #[msg("Cannot cancel during verification - users already verified or sale not undersubscribed")]
    CannotCancelDuringVerification,
    
    // Arithmetic errors
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    
    #[msg("Division by zero")]
    DivisionByZero,
    
    // Chunk errors
    #[msg("Chunk too large")]
    ChunkTooLarge,

    #[msg("Invalid chunk index")]
    InvalidChunkIndex,

    // Batch processing errors
    #[msg("Batch size exceeds maximum allowed (10)")]
    BatchSizeTooLarge,

    #[msg("Batch cannot be empty")]
    BatchEmpty,

    // Capacity errors
    #[msg("Maximum user capacity exceeded for this sale")]
    CapacityExceeded,

    // Drand errors
    #[msg("Encrypted bid required for this sale")]
    EncryptedBidRequired,

    #[msg("Invalid drand round")]
    InvalidDrandRound,

    #[msg("Invalid drand signature")]
    InvalidDrandSignature,

    #[msg("Encrypted bids not enabled for this sale")]
    EncryptedBidsNotEnabled,

    #[msg("Bid not revealed yet")]
    BidNotRevealed,

    #[msg("Bid already revealed")]
    BidAlreadyRevealed,

    #[msg("All bids must be revealed before settlement")]
    BidsNotRevealed,

    #[msg("Commitment window not closed")]
    CommitmentWindowNotClosed,

    #[msg("Sale not in revealing status")]
    SaleNotRevealing,

    #[msg("Hash commitment verification failed - revealed bid doesn't match commitment")]
    HashCommitmentMismatch,
}