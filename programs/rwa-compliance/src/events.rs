use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub agent: Pubkey,
}

#[event]
pub struct InvestorUpserted {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub country_code: u8,
    pub kyc_expiry: i64,
    pub lockup_until: i64,
    pub max_balance: u64,
    pub status: u8,
}

#[event]
pub struct InvestorRevoked {
    pub mint: Pubkey,
    pub owner: Pubkey,
}

#[event]
pub struct InvestorAccountFrozen {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub token_account: Pubkey,
}

#[event]
pub struct InvestorAccountThawed {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub token_account: Pubkey,
}

#[event]
pub struct ForceTransferExecuted {
    pub mint: Pubkey,
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub reason_code: u8,
}

#[event]
pub struct PausedStateChanged {
    pub mint: Pubkey,
    pub paused: bool,
}

#[event]
pub struct AuthorityTransferInitiated {
    pub mint: Pubkey,
    pub current_authority: Pubkey,
    pub pending_authority: Pubkey,
}

#[event]
pub struct AuthorityTransferAccepted {
    pub mint: Pubkey,
    pub previous_authority: Pubkey,
    pub new_authority: Pubkey,
}
