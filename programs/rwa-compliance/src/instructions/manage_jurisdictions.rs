use anchor_lang::prelude::*;

use crate::errors::ComplianceError;
use crate::state::Config;

#[derive(Accounts)]
pub struct ManageJurisdictions<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        has_one = authority @ ComplianceError::Unauthorized,
    )]
    pub config: Account<'info, Config>,
}

pub fn handler(ctx: Context<ManageJurisdictions>, country_code: u8, allowed: bool) -> Result<()> {
    ctx.accounts.config.set_jurisdiction(country_code, allowed);
    Ok(())
}
