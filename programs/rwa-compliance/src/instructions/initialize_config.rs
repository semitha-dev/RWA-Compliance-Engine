use anchor_lang::prelude::*;

use crate::events::ConfigInitialized;
use crate::state::Config;

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: mint identity is only used to derive/bind the Config PDA; the actual
    /// Token-2022 mint account is created and configured separately via the SPL CLI/SDK.
    pub mint: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        space = Config::SIZE,
        seeds = [Config::SEED_PREFIX, mint.key().as_ref()],
        bump
    )]
    pub config: Account<'info, Config>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeConfig>,
    agent: Pubkey,
    lockup_until: i64,
    max_holders: u32,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.pending_authority = Pubkey::default();
    config.agent = agent;
    config.mint = ctx.accounts.mint.key();
    config.paused = false;
    config.lockup_until = lockup_until;
    config.max_holders = max_holders;
    config.holder_count = 0;
    config.allowed_jurisdictions = [0u8; 32];
    config.bump = ctx.bumps.config;

    emit!(ConfigInitialized {
        mint: config.mint,
        authority: config.authority,
        agent: config.agent,
    });

    Ok(())
}
