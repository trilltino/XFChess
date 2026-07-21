//! Instruction for initializing and verifying player profiles.

use crate::account_ix::profile_init;
use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_lang::solana_program::system_instruction;

use anchor_lang::Discriminator;

#[derive(Accounts)]
#[instruction(username: String, country: String, date_of_birth: i64)]
pub struct InitProfile<'info> {
    /// CHECK: Seeds and ownership are verified manually in the handler to allow re-initialization.
    #[account(mut)]
    pub player_profile: AccountInfo<'info>,

    /// UsernameRecord PDA ensures uniqueness
    #[account(
        init_if_needed,
        payer = player,
        space = 8 + UsernameRecord::LEN,
        seeds = [USERNAME_SEED, username.as_bytes()],
        bump
    )]
    pub username_record: Account<'info, UsernameRecord>,

    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Seconds in 18 years (18 * 365.25 days).
const EIGHTEEN_YEARS_SECS: i64 = 567_648_000;

pub fn handler(
    ctx: Context<InitProfile>,
    username: String,
    country: String,
    date_of_birth: i64,
) -> Result<()> {
    // Validate username format
    validate_username(&username)?;

    // Enforce 18+ age gate: DOB must be at least 18 years before now.
    let now = Clock::get()?.unix_timestamp;
    require!(
        date_of_birth > 0 && now - date_of_birth >= EIGHTEEN_YEARS_SECS,
        crate::errors::GameErrorCode::UnderagePlayer
    );

    let profile_info = &ctx.accounts.player_profile;
    let player = &ctx.accounts.player;
    let system_program = &ctx.accounts.system_program;
    let record = &mut ctx.accounts.username_record;

    // 1. Manually Handle Profile Account (Creation or Allocation)
    if profile_info.data_is_empty() {
        let (pda, bump) =
            Pubkey::find_program_address(&[PROFILE_SEED, player.key().as_ref()], ctx.program_id);
        if profile_info.key() != pda {
            return err!(crate::errors::GameErrorCode::UnauthorizedAccess);
        }

        let space = 8 + PlayerProfile::INIT_SPACE;
        let lamports = Rent::get()?.minimum_balance(space);

        invoke_signed(
            &system_instruction::create_account(
                &player.key(),
                &profile_info.key(),
                lamports,
                space as u64,
                ctx.program_id,
            ),
            &[
                player.to_account_info(),
                profile_info.to_account_info(),
                system_program.to_account_info(),
            ],
            &[&[PROFILE_SEED, player.key().as_ref(), &[bump]]],
        )?;
    } else {
        // Already exists - verify ownership
        require!(
            profile_info.owner == ctx.program_id,
            crate::errors::GameErrorCode::UnauthorizedAccess
        );

        // 2. Ensure enough space (Realloc if needed for legacy accounts)
        let required_space = 8 + PlayerProfile::INIT_SPACE;
        if profile_info.data_len() < required_space {
            profile_info.realloc(required_space, false)?;

            // Adjust lamports for rent exemption
            let rent = Rent::get()?;
            let new_minimum_balance = rent.minimum_balance(required_space);
            let lamports_diff = new_minimum_balance.saturating_sub(profile_info.lamports());

            if lamports_diff > 0 {
                invoke(
                    &system_instruction::transfer(
                        &player.key(),
                        &profile_info.key(),
                        lamports_diff,
                    ),
                    &[
                        player.to_account_info(),
                        profile_info.to_account_info(),
                        system_program.to_account_info(),
                    ],
                )?;
            }
        }
    }

    let profile = {
        let data = profile_info.try_borrow_data()?;
        let mut profile = profile_init::load_or_new_profile(&data, player.key(), now)?;
        profile_init::update_identity_fields(
            &mut profile,
            username.clone(),
            country,
            date_of_birth,
        );
        profile
    };

    // Write Discriminator
    let mut data = profile_info.try_borrow_mut_data()?;
    let disc = PlayerProfile::DISCRIMINATOR;
    data[..8].copy_from_slice(&disc);

    // Serialize state
    let mut writer = &mut data[8..];
    profile.serialize(&mut writer)?;

    // 3. Handle Username Record
    if record.owner == Pubkey::default() {
        record.owner = player.key();
        record.created_at = Clock::get()?.unix_timestamp;
    } else {
        require!(record.owner == player.key(), UsernameError::UsernameTaken);
    }

    Ok(())
}

#[derive(Accounts)]
pub struct VerifyProfile<'info> {
    #[account(
        mut,
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    /// CHECK: The authority who can verify profiles (e.g. the VPS master key)
    #[account(signer, address = crate::constants::kyc_authority::ID @ crate::errors::GameErrorCode::UnauthorizedAccess)]
    pub admin: AccountInfo<'info>,
    /// CHECK: We just need their pubkey to form the seed
    pub player: AccountInfo<'info>,
}

pub fn verify_handler(ctx: Context<VerifyProfile>) -> Result<()> {
    let profile = &mut ctx.accounts.player_profile;
    profile.is_verified = true;
    Ok(())
}
