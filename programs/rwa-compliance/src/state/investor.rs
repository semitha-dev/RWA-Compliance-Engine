use anchor_lang::prelude::*;

use super::config::investor_status;

#[account]
pub struct InvestorRecord {
    /// The mint this record applies to (a holder needs one record per mint).
    pub mint: Pubkey,
    /// The wallet (token account owner) this record describes.
    pub owner: Pubkey,
    /// Coarse jurisdiction indicator, indexed against `Config.allowed_jurisdictions`.
    pub country_code: u8,
    /// Unix timestamp after which KYC is considered stale and transfers are rejected.
    pub kyc_expiry: i64,
    /// Unix timestamp before which this specific investor may not transfer
    /// (e.g. founder/team vesting cliff), independent of the mint-wide lockup.
    pub lockup_until: i64,
    /// Maximum token balance this investor may hold, 0 = unlimited.
    pub max_balance: u64,
    /// Bitflags: ALLOWLISTED | ACCREDITED | FROZEN | REVOKED (see investor_status).
    pub status: u8,
    pub bump: u8,
}

impl InvestorRecord {
    pub const SEED_PREFIX: &'static [u8] = b"investor";

    pub const SIZE: usize = 8 // discriminator
        + 32 // mint
        + 32 // owner
        + 1  // country_code
        + 8  // kyc_expiry
        + 8  // lockup_until
        + 8  // max_balance
        + 1  // status
        + 1; // bump

    pub fn has_flag(&self, flag: u8) -> bool {
        self.status & flag == flag
    }

    pub fn set_flag(&mut self, flag: u8, on: bool) {
        if on {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }

    pub fn is_allowlisted(&self) -> bool {
        self.has_flag(investor_status::ALLOWLISTED)
    }

    pub fn is_frozen(&self) -> bool {
        self.has_flag(investor_status::FROZEN)
    }

    pub fn is_revoked(&self) -> bool {
        self.has_flag(investor_status::REVOKED)
    }

    pub fn kyc_valid(&self, now: i64) -> bool {
        self.kyc_expiry == 0 || now < self.kyc_expiry
    }

    pub fn lockup_expired(&self, now: i64) -> bool {
        now >= self.lockup_until
    }
}
