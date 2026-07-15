use anchor_lang::prelude::*;

#[error_code]
pub enum HookError {
    #[msg("Execute was invoked outside of an active Token-2022 transfer")]
    NotCurrentlyTransferring,
    #[msg("Source or destination token account does not belong to the expected mint")]
    MintMismatch,
    #[msg("Program is paused")]
    Paused,
    #[msg("Global lockup has not yet expired")]
    GlobalLockupActive,
    #[msg("Source investor is not allowlisted")]
    SourceNotAllowlisted,
    #[msg("Destination investor is not allowlisted")]
    DestinationNotAllowlisted,
    #[msg("Source investor KYC has expired")]
    SourceKycExpired,
    #[msg("Destination investor KYC has expired")]
    DestinationKycExpired,
    #[msg("Source investor account is frozen")]
    SourceFrozen,
    #[msg("Destination investor account is frozen")]
    DestinationFrozen,
    #[msg("Source investor has been revoked")]
    SourceRevoked,
    #[msg("Destination investor has been revoked")]
    DestinationRevoked,
    #[msg("Source investor-specific lockup has not yet expired")]
    SourceLockupActive,
    #[msg("Destination jurisdiction is not permitted for this mint")]
    JurisdictionNotAllowed,
    #[msg("Transfer would exceed destination investor's maximum allowed balance")]
    BalanceCapExceeded,
    #[msg("Arithmetic overflow while evaluating transfer amount")]
    Overflow,
}
