use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use spl_tlv_account_resolution::{account::ExtraAccountMeta, seeds::Seed, state::ExtraAccountMetaList};
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

use crate::events::ExtraAccountMetaListInitialized;

/// Seeds for the compliance program's InvestorRecord PDA, duplicated here (rather than
/// imported) because ExtraAccountMeta::new_external_pda_with_seeds needs a `&[Seed]`
/// evaluated entirely from on-chain account data at CPI-resolution time -- there is no
/// cross-crate constant we can share for the literal seed prefix bytes.
const INVESTOR_SEED_PREFIX: &[u8] = b"investor";
const CONFIG_SEED_PREFIX: &[u8] = b"config";

/// Index, within the *base* `accounts` vec passed to `execute`, of the mint account.
/// Token-2022 always supplies [source, mint, destination, owner, extra_account_meta_list]
/// as the first five accounts before any extra accounts we register here.
const MINT_ACCOUNT_INDEX: u8 = 1;
const SOURCE_ACCOUNT_INDEX: u8 = 0;
const DESTINATION_ACCOUNT_INDEX: u8 = 2;

/// Index, within the *extra* accounts this hook appends, of the `compliance_program`
/// account itself. `ExtraAccountMeta` indices for `AccountKey`/`AccountData`/program-index
/// references count starting after the base accounts Token-2022 always supplies (5 of
/// them: source, mint, destination, owner, extra_account_meta_list), so the first extra
/// account we add -- `compliance_program` -- lands at index 5.
const COMPLIANCE_PROGRAM_EXTRA_INDEX: u8 = 5;

/// Byte offset of the `owner: Pubkey` field within a (Pod)Account's base 165-byte layout:
/// mint (0..32), owner (32..64). Used to derive each side's InvestorRecord PDA directly
/// from the token account's own data, so the accounts a malicious client supplies cannot
/// diverge from the wallet that actually owns the token account being debited/credited.
const TOKEN_ACCOUNT_OWNER_OFFSET: u8 = 32;
const PUBKEY_LEN: u8 = 32;

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    /// CHECK: initialized and owned by this program via the `init` constraint below;
    /// its contents are the TLV-encoded ExtraAccountMetaList, not an Anchor account type.
    #[account(
        init,
        payer = payer,
        space = ExtraAccountMetaList::size_of(InitializeExtraAccountMetaList::extra_account_metas().len()).unwrap(),
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,

    /// CHECK: must equal `rwa_compliance::ID`; enforced by the `address` constraint. This
    /// account is never read -- it exists only as an entry in the extra-account list so
    /// later entries can derive PDAs owned by this program via `new_external_pda_with_seeds`.
    #[account(address = rwa_compliance::ID)]
    pub compliance_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitializeExtraAccountMetaList<'info> {
    /// The extra accounts Token-2022 must append to every `execute` CPI it makes into
    /// this hook, on top of the five base accounts it always supplies. Order matters:
    /// it must match the order `execute`'s `Accounts` struct expects them.
    pub fn extra_account_metas() -> Vec<ExtraAccountMeta> {
        vec![
            // 0: fixed reference to the compliance program, so later entries can derive
            // PDAs owned by it via `new_external_pda_with_seeds`.
            ExtraAccountMeta::new_with_pubkey(&rwa_compliance::ID, false, false).unwrap(),
            // 1: this mint's Config PDA (rules: paused, lockup, jurisdictions), owned by
            // the compliance program -- an external PDA relative to this hook program.
            ExtraAccountMeta::new_external_pda_with_seeds(
                COMPLIANCE_PROGRAM_EXTRA_INDEX,
                &[
                    Seed::Literal { bytes: CONFIG_SEED_PREFIX.to_vec() },
                    Seed::AccountKey { index: MINT_ACCOUNT_INDEX },
                ],
                false,
                false,
            )
            .unwrap(),
            // 2: source investor record, keyed by [b"investor", mint, source_token.owner].
            // The owner pubkey is read directly out of the source token account's own
            // data (offset 32) rather than trusted as a separately-supplied account.
            ExtraAccountMeta::new_external_pda_with_seeds(
                COMPLIANCE_PROGRAM_EXTRA_INDEX,
                &[
                    Seed::Literal { bytes: INVESTOR_SEED_PREFIX.to_vec() },
                    Seed::AccountKey { index: MINT_ACCOUNT_INDEX },
                    Seed::AccountData {
                        account_index: SOURCE_ACCOUNT_INDEX,
                        data_index: TOKEN_ACCOUNT_OWNER_OFFSET,
                        length: PUBKEY_LEN,
                    },
                ],
                false,
                false,
            )
            .unwrap(),
            // 3: destination investor record, same pattern keyed off the destination
            // token account's own owner bytes -- this is what makes the recipient-side
            // compliance check unspoofable by a client assembling the instruction.
            ExtraAccountMeta::new_external_pda_with_seeds(
                COMPLIANCE_PROGRAM_EXTRA_INDEX,
                &[
                    Seed::Literal { bytes: INVESTOR_SEED_PREFIX.to_vec() },
                    Seed::AccountKey { index: MINT_ACCOUNT_INDEX },
                    Seed::AccountData {
                        account_index: DESTINATION_ACCOUNT_INDEX,
                        data_index: TOKEN_ACCOUNT_OWNER_OFFSET,
                        length: PUBKEY_LEN,
                    },
                ],
                false,
                false,
            )
            .unwrap(),
        ]
    }
}

pub fn handler(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
    let extra_account_metas = InitializeExtraAccountMetaList::extra_account_metas();

    let mut data = ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?;
    ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, &extra_account_metas)
        .map_err(|_| error!(anchor_lang::error::ErrorCode::AccountDidNotDeserialize))?;

    emit!(ExtraAccountMetaListInitialized {
        mint: ctx.accounts.mint.key(),
    });

    Ok(())
}
