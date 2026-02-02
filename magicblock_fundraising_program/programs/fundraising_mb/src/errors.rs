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

    // Settlement errors
    #[msg("Verification incomplete - not all users verified")]
    VerificationIncomplete,

    #[msg("Invalid running sum - settlement proposal incorrect")]
    InvalidRunningSum,

    #[msg("Running sum exceeds raise_max - over-allocation detected")]
    RunningSumExceedsMax,

    #[msg("Sale undersubscribed - running_sum below raise_min")]
    RunningSumBelowMin,

    // Claim errors
    #[msg("Already claimed")]
    AlreadyClaimed,

    // Cancellation errors
    #[msg("Cannot cancel settled sale")]
    CannotCancelSettledSale,

    #[msg("Cannot cancel during verification")]
    CannotCancelDuringVerification,

    // Arithmetic errors
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Division by zero")]
    DivisionByZero,

    // Batch processing errors
    #[msg("Batch size exceeds maximum allowed")]
    BatchSizeTooLarge,

    #[msg("Batch cannot be empty")]
    BatchEmpty,

    // Capacity errors
    #[msg("Maximum user capacity exceeded for this sale")]
    CapacityExceeded,

    // PER / MagicBlock errors
    #[msg("Bid not yet submitted")]
    BidNotSubmitted,

    #[msg("Bid already submitted")]
    BidAlreadySubmitted,

    #[msg("Settlement not yet computed")]
    SettlementNotComputed,

    #[msg("Bids not yet zeroed")]
    BidsNotZeroed,

    #[msg("Max FDV out of range")]
    MaxFdvOutOfRange,

    #[msg("Unauthorized - only authority can perform this action")]
    Unauthorized,
}
