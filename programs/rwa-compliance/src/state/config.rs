use anchor_lang::prelude::*;

/// Bitflags for `InvestorRecord.status`.
pub mod investor_status {
    pub const ALLOWLISTED: u8 = 1 << 0;
    pub const ACCREDITED: u8 = 1 << 1;
    pub const FROZEN: u8 = 1 << 2;
    pub const REVOKED: u8 = 1 << 3;
}

#[account]
pub struct Config {
    /// Current authority able to administer this mint's compliance rules.
    pub authority: Pubkey,
    /// Set when a two-step authority transfer is in flight; Pubkey::default() otherwise.
    pub pending_authority: Pubkey,
    /// Day-to-day compliance operator (KYC upsert, freeze/thaw). Distinct from authority
    /// so an issuer can delegate operations without handing over admin control.
    pub agent: Pubkey,
    /// The mint this config governs. Stored so PDA seeds can be validated against it.
    pub mint: Pubkey,
    /// Emergency stop: when true, all transfers are rejected by the hook.
    pub paused: bool,
    /// Unix timestamp before which no transfers are permitted at all (issuance lockup).
    pub lockup_until: i64,
    /// Maximum distinct token accounts allowed to hold a non-zero balance, 0 = unlimited.
    pub max_holders: u32,
    /// Current count of distinct holders; maintained by thaw/freeze bookkeeping.
    pub holder_count: u32,
    /// 256-bit bitmap keyed by ISO-3166-numeric-derived country_code (0-255).
    /// Bit set = jurisdiction permitted to hold/receive this token.
    pub allowed_jurisdictions: [u8; 32],
    pub bump: u8,
}

impl Config {
    pub const SEED_PREFIX: &'static [u8] = b"config";

    pub const SIZE: usize = 8 // discriminator
        + 32 // authority
        + 32 // pending_authority
        + 32 // agent
        + 32 // mint
        + 1  // paused
        + 8  // lockup_until
        + 4  // max_holders
        + 4  // holder_count
        + 32 // allowed_jurisdictions
        + 1; // bump

    pub fn is_jurisdiction_allowed(&self, country_code: u8) -> bool {
        let byte_index = (country_code / 8) as usize;
        let bit_index = country_code % 8;
        (self.allowed_jurisdictions[byte_index] >> bit_index) & 1 == 1
    }

    pub fn set_jurisdiction(&mut self, country_code: u8, allowed: bool) {
        let byte_index = (country_code / 8) as usize;
        let bit_index = country_code % 8;
        if allowed {
            self.allowed_jurisdictions[byte_index] |= 1 << bit_index;
        } else {
            self.allowed_jurisdictions[byte_index] &= !(1 << bit_index);
        }
    }
}
