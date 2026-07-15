use anchor_lang::prelude::*;

use crate::errors::ComplianceError;
use crate::events::{AuthorityTransferAccepted, AuthorityTransferInitiated};
use crate::state::Config;

#[derive(Accounts)]
pub struct InitiateAuthorityTransfer<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        has_one = authority @ ComplianceError::Unauthorized,
    )]
    pub config: Account<'info, Config>,
}

pub fn initiate_handler(
    ctx: Context<InitiateAuthorityTransfer>,
    new_authority: Pubkey,
) -> Result<()> {
    ctx.accounts.config.pending_authority = new_authority;
    emit!(AuthorityTransferInitiated {
        mint: ctx.accounts.config.mint,
        current_authority: ctx.accounts.config.authority,
        pending_authority: new_authority,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct AcceptAuthorityTransfer<'info> {
    pub pending_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        constraint = config.pending_authority == pending_authority.key() @ ComplianceError::NotPendingAuthority,
    )]
    pub config: Account<'info, Config>,
}

pub fn accept_handler(ctx: Context<AcceptAuthorityTransfer>) -> Result<()> {
    let previous_authority = ctx.accounts.config.authority;
    ctx.accounts.config.authority = ctx.accounts.pending_authority.key();
    ctx.accounts.config.pending_authority = Pubkey::default();

    emit!(AuthorityTransferAccepted {
        mint: ctx.accounts.config.mint,
        previous_authority,
        new_authority: ctx.accounts.config.authority,
    });

    Ok(())
}
