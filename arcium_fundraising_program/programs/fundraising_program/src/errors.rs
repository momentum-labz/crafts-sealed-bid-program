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
    #[msg("Arcium computation failed")]
    ArciumComputationFailed,

    #[msg("Arcium settlement not received")]
    ArciumSettlementNotReceived,

    #[msg("Bid not checked by MPC yet")]
    BidNotChecked,

    #[msg("Arcium cluster not set")]
    ClusterNotSet,

    // Clearing errors
    #[msg("No clearing price found")]
    NoClearingPriceFound,

    #[msg("Insufficient demand")]
    InsufficientDemand,

    #[msg("Invalid clearing FDV - must be within sale FDV range")]
    InvalidClearingFdv,

    #[msg("Invalid fill rate - must be between 1 and 10000 basis points")]
    InvalidFillRate,

    // Processing errors
    #[msg("Already processed")]
    AlreadyProcessed,

    #[msg("Already verified")]
    AlreadyVerified,

    #[msg("Wrong status")]
    WrongStatus,

    // Claim errors
    #[msg("Already claimed")]
    AlreadyClaimed,

    // Cancellation errors
    #[msg("Cannot cancel settled sale")]
    CannotCancelSettledSale,

    // Arithmetic errors
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Division by zero")]
    DivisionByZero,

    // Capacity errors
    #[msg("Maximum user capacity exceeded for this sale")]
    CapacityExceeded,

    // Encrypted bid errors
    #[msg("Encrypted bid required for this sale")]
    EncryptedBidRequired,

    #[msg("Commitment window not closed")]
    CommitmentWindowNotClosed,

    // Batch processing errors
    #[msg("Batch size exceeds maximum allowed")]
    BatchSizeTooLarge,

    #[msg("Batch cannot be empty")]
    BatchEmpty,

    #[msg("Invalid bucket count: must be between 1 and 200")]
    InvalidBucketCount,
}
