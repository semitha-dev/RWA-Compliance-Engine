use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("4on4ihN7YpJoTErX7VYBBB86ae71noFD5PpXEZwG8ks5");

/// On-chain compliance registry and control plane for an RWA security token.
///
/// This program owns the KYC/jurisdiction/lockup rules that the sibling
/// `rwa-transfer-hook` program enforces on every Token-2022 transfer. It also
/// holds the mint's Permanent Delegate and Freeze authorities (as the `Config`
/// PDA) so that thaw-on-KYC and regulatory clawback can be performed without a
/// human-held keypair ever being the sole authority over investor funds.
#[program]
pub mod rwa_compliance {
    use super::*;

    /// Creates the `Config` PDA for a mint. Must be called once, after the
    /// Token-2022 mint has been created with Default Account State = Frozen,
    /// Permanent Delegate = this PDA, and Freeze Authority = this PDA.
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        agent: Pubkey,
        lockup_until: i64,
        max_holders: u32,
    ) -> Result<()> {
        initialize_config::handler(ctx, agent, lockup_until, max_holders)
    }

    /// Adds or removes a permitted jurisdiction from the mint-wide allowlist bitmap.
    pub fn manage_jurisdictions(
        ctx: Context<ManageJurisdictions>,
        country_code: u8,
        allowed: bool,
    ) -> Result<()> {
        manage_jurisdictions::handler(ctx, country_code, allowed)
    }

    /// Creates or updates an investor's KYC record. Idempotent: re-running with new
    /// parameters overwrites the previous record (e.g. to extend KYC expiry).
    pub fn upsert_investor(
        ctx: Context<UpsertInvestor>,
        owner: Pubkey,
        country_code: u8,
        kyc_expiry: i64,
        lockup_until: i64,
        max_balance: u64,
        accredited: bool,
    ) -> Result<()> {
        upsert_investor::handler(
            ctx,
            owner,
            country_code,
            kyc_expiry,
            lockup_until,
            max_balance,
            accredited,
        )
    }

    /// Marks an investor record as revoked, immediately disqualifying it from the
    /// allowlist check performed by the transfer hook on every subsequent transfer.
    pub fn revoke_investor(ctx: Context<RevokeInvestor>) -> Result<()> {
        revoke_investor::handler(ctx)
    }

    /// Thaws a holder's token account. Gated on a valid, allowlisted `InvestorRecord`
    /// so a freshly created account (frozen by default) cannot receive tokens until
    /// KYC has actually been recorded on-chain.
    pub fn thaw_investor_account(ctx: Context<ThawInvestorAccount>) -> Result<()> {
        thaw_investor_account::handler(ctx)
    }

    /// Administrative freeze of a single token account, independent of the investor
    /// record (e.g. suspected compromise, court order) without revoking KYC.
    pub fn freeze_investor_account(ctx: Context<FreezeInvestorAccount>) -> Result<()> {
        freeze_investor_account::handler(ctx)
    }

    /// Regulatory clawback: moves tokens between two accounts using the mint's
    /// Permanent Delegate authority, bypassing normal holder-initiated transfer.
    pub fn force_transfer(
        ctx: Context<ForceTransfer>,
        amount: u64,
        reason_code: u8,
    ) -> Result<()> {
        force_transfer::handler(ctx, amount, reason_code)
    }

    /// Emergency stop switch. When paused, the transfer hook rejects all transfers.
    pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
        set_paused::handler(ctx, paused)
    }

    /// Step 1 of a 2-step authority transfer: current authority nominates a successor.
    pub fn initiate_authority_transfer(
        ctx: Context<InitiateAuthorityTransfer>,
        new_authority: Pubkey,
    ) -> Result<()> {
        transfer_authority::initiate_handler(ctx, new_authority)
    }

    /// Step 2: the nominated successor accepts, becoming the new authority. Splitting
    /// this into two steps prevents an authority transfer to an unreachable/incorrect
    /// key from permanently locking the config.
    pub fn accept_authority_transfer(ctx: Context<AcceptAuthorityTransfer>) -> Result<()> {
        transfer_authority::accept_handler(ctx)
    }
}
