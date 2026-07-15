use anchor_lang::prelude::*;
use anchor_spl::token_2022::{transfer_checked, Token2022, TransferChecked};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::errors::ComplianceError;
use crate::events::ForceTransferExecuted;
use crate::state::Config;

/// Regulatory clawback / forced transfer, authorized by the mint's Permanent Delegate
/// (this Config PDA). Used for court-ordered seizure, fixing erroneous transfers, or
/// migrating a lost investor's balance to a recovery wallet. This bypasses the normal
/// holder-initiated transfer path entirely and does not require the source account
/// owner's signature — by design, since a Permanent Delegate exists precisely to allow
/// the issuer to move tokens without holder cooperation.
#[derive(Accounts)]
pub struct ForceTransfer<'info> {
    pub authority: Signer<'info>,

    #[account(
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        has_one = authority @ ComplianceError::Unauthorized,
    )]
    pub config: Account<'info, Config>,

    #[account(
        constraint = mint.key() == config.mint @ ComplianceError::MintMismatch,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        constraint = from.mint == mint.key() @ ComplianceError::MintMismatch,
    )]
    pub from: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = to.mint == mint.key() @ ComplianceError::MintMismatch,
    )]
    pub to: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,
}

pub fn handler(ctx: Context<ForceTransfer>, amount: u64, reason_code: u8) -> Result<()> {
    let mint_key = ctx.accounts.config.mint;
    let bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[Config::SEED_PREFIX, mint_key.as_ref(), &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.from.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.to.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    emit!(ForceTransferExecuted {
        mint: mint_key,
        from: ctx.accounts.from.key(),
        to: ctx.accounts.to.key(),
        amount,
        reason_code,
    });

    Ok(())
}
