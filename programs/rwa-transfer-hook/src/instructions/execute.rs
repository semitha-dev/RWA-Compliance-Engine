use std::cell::RefMut;

use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{transfer_hook::TransferHookAccount, BaseStateWithExtensionsMut, PodStateWithExtensionsMut},
    pod::PodAccount,
};
use anchor_spl::token_interface::{Mint, TokenAccount};

use rwa_compliance::state::config::investor_status;
use rwa_compliance::state::{Config, InvestorRecord};

use crate::errors::HookError;
use crate::events::TransferChecked as TransferCheckedEvent;

#[derive(Accounts)]
pub struct Execute<'info> {
    #[account(
        token::mint = mint,
        constraint = source_token.mint == mint.key() @ HookError::MintMismatch,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        constraint = destination_token.mint == mint.key() @ HookError::MintMismatch,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: source token account owner; may be a wallet or a PDA owned by another
    /// program, so it cannot be typed more strongly than an UncheckedAccount here.
    pub owner: UncheckedAccount<'info>,

    /// CHECK: the ExtraAccountMetaList PDA itself; Token-2022 requires it be present
    /// but this hook never reads its contents directly.
    #[account(seeds = [b"extra-account-metas", mint.key().as_ref()], bump)]
    pub extra_account_meta_list: UncheckedAccount<'info>,

    #[account(
        seeds = [Config::SEED_PREFIX, mint.key().as_ref()],
        bump = config.bump,
        constraint = config.mint == mint.key() @ HookError::MintMismatch,
    )]
    pub config: Account<'info, Config>,

    #[account(
        seeds = [InvestorRecord::SEED_PREFIX, mint.key().as_ref(), source_token.owner.as_ref()],
        bump = source_investor.bump,
    )]
    pub source_investor: Account<'info, InvestorRecord>,

    #[account(
        seeds = [InvestorRecord::SEED_PREFIX, mint.key().as_ref(), destination_token.owner.as_ref()],
        bump = destination_investor.bump,
    )]
    pub destination_investor: Account<'info, InvestorRecord>,
}

pub fn handler(ctx: Context<Execute>, amount: u64) -> Result<()> {
    check_is_transferring(&ctx.accounts.source_token)?;
    check_is_transferring(&ctx.accounts.destination_token)?;

    let config = &ctx.accounts.config;
    let source = &ctx.accounts.source_investor;
    let destination = &ctx.accounts.destination_investor;
    let clock = Clock::get()?;
    let now = clock.unix_timestamp;

    require!(!config.paused, HookError::Paused);
    require!(now >= config.lockup_until, HookError::GlobalLockupActive);

    require!(source.is_allowlisted(), HookError::SourceNotAllowlisted);
    require!(!source.is_revoked(), HookError::SourceRevoked);
    require!(!source.has_flag(investor_status::FROZEN), HookError::SourceFrozen);
    require!(source.kyc_valid(now), HookError::SourceKycExpired);
    require!(source.lockup_expired(now), HookError::SourceLockupActive);

    require!(destination.is_allowlisted(), HookError::DestinationNotAllowlisted);
    require!(!destination.is_revoked(), HookError::DestinationRevoked);
    require!(!destination.has_flag(investor_status::FROZEN), HookError::DestinationFrozen);
    require!(destination.kyc_valid(now), HookError::DestinationKycExpired);

    require!(
        config.is_jurisdiction_allowed(destination.country_code),
        HookError::JurisdictionNotAllowed
    );

    if destination.max_balance > 0 {
        let new_balance = ctx
            .accounts
            .destination_token
            .amount
            .checked_add(amount)
            .ok_or(HookError::Overflow)?;
        require!(new_balance <= destination.max_balance, HookError::BalanceCapExceeded);
    }

    emit!(TransferCheckedEvent {
        mint: ctx.accounts.mint.key(),
        source_owner: ctx.accounts.source_token.owner,
        destination_owner: ctx.accounts.destination_token.owner,
        amount,
    });

    Ok(())
}

/// Guards against direct invocation of `execute`: this instruction must only ever run
/// as a CPI from Token-2022 in the middle of an active `TransferChecked`. Token-2022 sets
/// the `transferring` bit on the TransferHookAccount extension of BOTH the source and
/// destination token accounts before making the CPI, and clears it immediately after --
/// so checking this flag (on both sides) is what stops an attacker from calling this
/// instruction directly to bypass every compliance check above.
fn check_is_transferring(token_account: &InterfaceAccount<TokenAccount>) -> Result<()> {
    let account_info = token_account.to_account_info();
    let mut account_data: RefMut<&mut [u8]> = account_info.try_borrow_mut_data()?;

    // `get_extension_mut` (rather than a non-mut accessor) is used deliberately to match
    // the field-access API that `PodStateWithExtensionsMut` actually exposes -- the value
    // is only read here, never written.
    let mut account = PodStateWithExtensionsMut::<PodAccount>::unpack(*account_data)
        .map_err(|_| HookError::NotCurrentlyTransferring)?;

    let extension = account
        .get_extension_mut::<TransferHookAccount>()
        .map_err(|_| HookError::NotCurrentlyTransferring)?;

    require!(bool::from(extension.transferring), HookError::NotCurrentlyTransferring);

    Ok(())
}
