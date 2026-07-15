use anchor_lang::prelude::*;

use crate::errors::ComplianceError;
use crate::events::PausedStateChanged;
use crate::state::Config;

#[derive(Accounts)]
pub struct SetPaused<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        has_one = authority @ ComplianceError::Unauthorized,
    )]
    pub config: Account<'info, Config>,
}

pub fn handler(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    ctx.accounts.config.paused = paused;
    emit!(PausedStateChanged {
        mint: ctx.accounts.config.mint,
        paused,
    });
    Ok(())
}
