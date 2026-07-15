use anchor_lang::prelude::*;

use crate::errors::ComplianceError;
use crate::events::InvestorRevoked;
use crate::state::config::investor_status;
use crate::state::{Config, InvestorRecord};

#[derive(Accounts)]
pub struct RevokeInvestor<'info> {
    pub agent: Signer<'info>,

    #[account(
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        constraint = config.agent == agent.key() @ ComplianceError::NotAgent,
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [InvestorRecord::SEED_PREFIX, config.mint.as_ref(), investor_record.owner.as_ref()],
        bump = investor_record.bump,
    )]
    pub investor_record: Account<'info, InvestorRecord>,
}

pub fn handler(ctx: Context<RevokeInvestor>) -> Result<()> {
    let record = &mut ctx.accounts.investor_record;
    record.set_flag(investor_status::REVOKED, true);
    record.set_flag(investor_status::ALLOWLISTED, false);

    emit!(InvestorRevoked {
        mint: record.mint,
        owner: record.owner,
    });

    Ok(())
}
