use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::ProgramConfig;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeConfigArgs {
    pub treasury_authority: Pubkey,
    pub vps_authority: Pubkey,
    pub dispute_authority: Pubkey,
    pub link_authority: Pubkey,
    pub kyc_authority: Pubkey,
    pub max_wager_amount: Option<u64>,
    pub min_wager_lamports: Option<u64>,
    pub max_platform_fee_lamports: Option<u64>,
    pub er_session_fee_lamports: Option<u64>,
    pub dispute_bond_lamports: Option<u64>,
    pub dispute_ttl_secs: Option<i64>,
    pub crank_max_slot_delay: Option<u64>,
    pub crank_max_seconds_early: Option<i64>,
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = ProgramConfig::LEN,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, ProgramConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_config_handler(
    ctx: Context<InitializeConfig>,
    args: InitializeConfigArgs,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.treasury_authority = args.treasury_authority;
    config.vps_authority = args.vps_authority;
    config.dispute_authority = args.dispute_authority;
    config.link_authority = args.link_authority;
    config.kyc_authority = args.kyc_authority;

    config.max_wager_amount = args.max_wager_amount.unwrap_or(MAX_WAGER_AMOUNT);
    config.min_wager_lamports = args.min_wager_lamports.unwrap_or(MIN_WAGER_LAMPORTS);
    config.max_platform_fee_lamports = args
        .max_platform_fee_lamports
        .unwrap_or(MAX_PLATFORM_FEE_LAMPORTS);
    config.er_session_fee_lamports = args
        .er_session_fee_lamports
        .unwrap_or(ER_SESSION_FEE_LAMPORTS);
    config.dispute_bond_lamports = args.dispute_bond_lamports.unwrap_or(DISPUTE_BOND_LAMPORTS);
    config.dispute_ttl_secs = args.dispute_ttl_secs.unwrap_or(DISPUTE_TTL_SECS);
    config.crank_max_slot_delay = args.crank_max_slot_delay.unwrap_or(CRANK_MAX_SLOT_DELAY);
    config.crank_max_seconds_early = args
        .crank_max_seconds_early
        .unwrap_or(CRANK_MAX_SECONDS_EARLY);

    config.bump = ctx.bumps.config;
    config.reserved = [0u8; 64];

    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct UpdateConfigArgs {
    pub new_authority: Option<Pubkey>,
    pub new_treasury_authority: Option<Pubkey>,
    pub new_vps_authority: Option<Pubkey>,
    pub new_dispute_authority: Option<Pubkey>,
    pub new_link_authority: Option<Pubkey>,
    pub new_kyc_authority: Option<Pubkey>,
    pub new_max_wager_amount: Option<u64>,
    pub new_min_wager_lamports: Option<u64>,
    pub new_max_platform_fee_lamports: Option<u64>,
    pub new_er_session_fee_lamports: Option<u64>,
    pub new_dispute_bond_lamports: Option<u64>,
    pub new_dispute_ttl_secs: Option<i64>,
    pub new_crank_max_slot_delay: Option<u64>,
    pub new_crank_max_seconds_early: Option<i64>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.authority == authority.key() @ GameErrorCode::UnauthorizedAccess
    )]
    pub config: Account<'info, ProgramConfig>,

    pub authority: Signer<'info>,
}

pub fn update_config_handler(ctx: Context<UpdateConfig>, args: UpdateConfigArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    if let Some(new_auth) = args.new_authority {
        config.authority = new_auth;
    }
    if let Some(new_treasury) = args.new_treasury_authority {
        config.treasury_authority = new_treasury;
    }
    if let Some(new_vps) = args.new_vps_authority {
        config.vps_authority = new_vps;
    }
    if let Some(new_dispute) = args.new_dispute_authority {
        config.dispute_authority = new_dispute;
    }
    if let Some(new_link) = args.new_link_authority {
        config.link_authority = new_link;
    }
    if let Some(new_kyc) = args.new_kyc_authority {
        config.kyc_authority = new_kyc;
    }

    if let Some(val) = args.new_max_wager_amount {
        config.max_wager_amount = val;
    }
    if let Some(val) = args.new_min_wager_lamports {
        config.min_wager_lamports = val;
    }
    if let Some(val) = args.new_max_platform_fee_lamports {
        config.max_platform_fee_lamports = val;
    }
    if let Some(val) = args.new_er_session_fee_lamports {
        config.er_session_fee_lamports = val;
    }
    if let Some(val) = args.new_dispute_bond_lamports {
        config.dispute_bond_lamports = val;
    }
    if let Some(val) = args.new_dispute_ttl_secs {
        config.dispute_ttl_secs = val;
    }
    if let Some(val) = args.new_crank_max_slot_delay {
        config.crank_max_slot_delay = val;
    }
    if let Some(val) = args.new_crank_max_seconds_early {
        config.crank_max_seconds_early = val;
    }

    Ok(())
}
