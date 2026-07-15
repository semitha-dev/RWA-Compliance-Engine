use anchor_lang::prelude::*;

#[error_code]
pub enum ComplianceError {
    #[msg("Program is paused")]
    Paused,
    #[msg("Global lockup has not yet expired")]
    GlobalLockupActive,
    #[msg("Investor is not allowlisted")]
    NotAllowlisted,
    #[msg("Investor KYC has expired")]
    KycExpired,
    #[msg("Investor account is frozen")]
    InvestorFrozen,
    #[msg("Investor has been revoked")]
    InvestorRevoked,
    #[msg("Investor-specific lockup has not yet expired")]
    InvestorLockupActive,
    #[msg("Destination jurisdiction is not permitted for this mint")]
    JurisdictionNotAllowed,
    #[msg("Transfer would exceed investor's maximum allowed balance")]
    BalanceCapExceeded,
    #[msg("Maximum holder count would be exceeded")]
    MaxHoldersExceeded,
    #[msg("Only the pending authority can accept an authority transfer")]
    NotPendingAuthority,
    #[msg("Only the current authority may perform this action")]
    Unauthorized,
    #[msg("Only a designated compliance agent may perform this action")]
    NotAgent,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Arithmetic underflow")]
    Underflow,
    #[msg("Country code must be a valid ISO-3166 numeric-3 style index (0-255)")]
    InvalidCountryCode,
    #[msg("Config account does not match the expected mint")]
    MintMismatch,
}
