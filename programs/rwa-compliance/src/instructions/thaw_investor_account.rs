use anchor_lang::prelude::*;
use anchor_spl::token_2022::{thaw_account, ThawAccount, Token2022};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::errors::ComplianceError;
use crate::events::InvestorAccountThawed;
use crate::state::{Config, InvestorRecord};

/// Thaws a holder's token account, but only once a valid, allowlisted `InvestorRecord`
/// exists for them. Because the mint's `Default Account State` extension is configured
/// to `Frozen`, every freshly-created associated token account starts frozen and is
/// structurally incapable of receiving tokens until this instruction is called — the
/// KYC gate is enforced by account state, not just by application logic.
#[derive(Accounts)]
pub struct ThawInvestorAccount<'info> {
    pub agent: Signer<'info>,

    #[account(
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        constraint = config.agent == agent.key() @ ComplianceError::NotAgent,
    )]
    pub config: Account<'info, Config>,

    #[account(
        seeds = [InvestorRecord::SEED_PREFIX, config.mint.as_ref(), investor_record.owner.as_ref()],
        bump = investor_record.bump,
        constraint = investor_record.is_allowlisted() @ ComplianceError::NotAllowlisted,
        constraint = !investor_record.is_revoked() @ ComplianceError::InvestorRevoked,
    )]
    pub investor_record: Account<'info, InvestorRecord>,

    #[account(
        constraint = mint.key() == config.mint @ ComplianceError::MintMismatch,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        constraint = token_account.owner == investor_record.owner @ ComplianceError::Unauthorized,
        constraint = token_account.mint == mint.key() @ ComplianceError::MintMismatch,
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,
}

pub fn handler(ctx: Context<ThawInvestorAccount>) -> Result<()> {
    let mint_key = ctx.accounts.config.mint;
    let bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[Config::SEED_PREFIX, mint_key.as_ref(), &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    thaw_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        ThawAccount {
            account: ctx.accounts.token_account.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.config.to_account_info(),
        },
        signer_seeds,
    ))?;

    emit!(InvestorAccountThawed {
        mint: mint_key,
        owner: ctx.accounts.investor_record.owner,
        token_account: ctx.accounts.token_account.key(),
    });

    Ok(())
}
