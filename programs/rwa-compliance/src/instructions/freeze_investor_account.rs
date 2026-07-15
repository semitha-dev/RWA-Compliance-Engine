use anchor_lang::prelude::*;
use anchor_spl::token_2022::{freeze_account, FreezeAccount, Token2022};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::errors::ComplianceError;
use crate::events::InvestorAccountFrozen;
use crate::state::Config;

/// Agent-triggered emergency/administrative freeze of a single holder's token account,
/// independent of the investor-record status flags. Used for e.g. a court order or a
/// suspected compromised wallet, without having to revoke the underlying KYC record.
#[derive(Accounts)]
pub struct FreezeInvestorAccount<'info> {
    pub agent: Signer<'info>,

    #[account(
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        constraint = config.agent == agent.key() @ ComplianceError::NotAgent,
    )]
    pub config: Account<'info, Config>,

    #[account(
        constraint = mint.key() == config.mint @ ComplianceError::MintMismatch,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        constraint = token_account.mint == mint.key() @ ComplianceError::MintMismatch,
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,
}

pub fn handler(ctx: Context<FreezeInvestorAccount>) -> Result<()> {
    let mint_key = ctx.accounts.config.mint;
    let bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[Config::SEED_PREFIX, mint_key.as_ref(), &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    let owner = ctx.accounts.token_account.owner;

    freeze_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        FreezeAccount {
            account: ctx.accounts.token_account.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.config.to_account_info(),
        },
        signer_seeds,
    ))?;

    emit!(InvestorAccountFrozen {
        mint: mint_key,
        owner,
        token_account: ctx.accounts.token_account.key(),
    });

    Ok(())
}
