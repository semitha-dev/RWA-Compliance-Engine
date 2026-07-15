use anchor_lang::prelude::*;
use spl_discriminator::SplDiscriminate;
use spl_transfer_hook_interface::instruction::{ExecuteInstruction, InitializeExtraAccountMetaListInstruction};

pub mod errors;
pub mod events;
pub mod instructions;

use instructions::*;

declare_id!("CT3MMR4YUu3K37d8gfMXH2KrUkgE215XWKx2rzzj1jL1");

/// Token-2022 Transfer Hook enforcing on-chain KYC / jurisdiction / lockup compliance
/// on every transfer of the RWA security token.
///
/// This program is deliberately read-only: it never mutates the `rwa-compliance`
/// registry, and Token-2022 converts every account in the CPI to read-only regardless
/// (a mint's own transfer hook cannot be given signer/write authority over the sender's
/// context). All administrative state changes -- KYC upserts, freezes, clawback -- happen
/// through `rwa-compliance` directly; this program only ever answers "is this transfer
/// allowed", and answers by rejecting the whole transaction on any violation.
#[program]
pub mod rwa_transfer_hook {
    use super::*;

    /// Creates the `ExtraAccountMetaList` PDA so Token-2022 knows which extra accounts
    /// (the mint's Config PDA and both parties' InvestorRecord PDAs) to resolve and
    /// attach automatically on every future transfer of this mint. Called once, after
    /// the mint is created and after `rwa_compliance::initialize_config`.
    #[instruction(discriminator = InitializeExtraAccountMetaListInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        initialize_extra_account_meta_list::handler(ctx)
    }

    /// Invoked by Token-2022 as a CPI on every transfer; never intended to be called
    /// directly by a client. See `check_is_transferring` in `instructions::execute` for
    /// how direct invocation is detected and rejected.
    #[instruction(discriminator = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn execute(ctx: Context<Execute>, amount: u64) -> Result<()> {
        execute::handler(ctx, amount)
    }
}
