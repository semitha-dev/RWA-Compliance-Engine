use anchor_lang::prelude::*;

use crate::errors::ComplianceError;
use crate::events::InvestorUpserted;
use crate::state::config::investor_status;
use crate::state::{Config, InvestorRecord};

#[derive(Accounts)]
#[instruction(owner: Pubkey)]
pub struct UpsertInvestor<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,

    #[account(
        seeds = [Config::SEED_PREFIX, config.mint.as_ref()],
        bump = config.bump,
        constraint = config.agent == agent.key() @ ComplianceError::NotAgent,
    )]
    pub config: Account<'info, Config>,

    #[account(
        init_if_needed,
        payer = agent,
        space = InvestorRecord::SIZE,
        seeds = [InvestorRecord::SEED_PREFIX, config.mint.as_ref(), owner.as_ref()],
        bump
    )]
    pub investor_record: Account<'info, InvestorRecord>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<UpsertInvestor>,
    owner: Pubkey,
    country_code: u8,
    kyc_expiry: i64,
    lockup_until: i64,
    max_balance: u64,
    accredited: bool,
) -> Result<()> {
    let record = &mut ctx.accounts.investor_record;
    record.mint = ctx.accounts.config.mint;
    record.owner = owner;
    record.country_code = country_code;
    record.kyc_expiry = kyc_expiry;
    record.lockup_until = lockup_until;
    record.max_balance = max_balance;
    record.bump = ctx.bumps.investor_record;

    record.set_flag(investor_status::ALLOWLISTED, true);
    record.set_flag(investor_status::REVOKED, false);
    record.set_flag(investor_status::ACCREDITED, accredited);

    emit!(InvestorUpserted {
        mint: record.mint,
        owner: record.owner,
        country_code: record.country_code,
        kyc_expiry: record.kyc_expiry,
        lockup_until: record.lockup_until,
        max_balance: record.max_balance,
        status: record.status,
    });

    Ok(())
}
