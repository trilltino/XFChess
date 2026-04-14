//! Instruction for initializing and verifying player profiles.

use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_lang::solana_program::system_instruction;

use anchor_lang::Discriminator;

#[derive(Accounts)]
#[instruction(username: String)]
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

pub fn handler(ctx: Context<InitProfile>, username: String) -> Result<()> {
    // Validate username format
    validate_username(&username)?;
    
    let profile_info = &ctx.accounts.player_profile;
    let player = &ctx.accounts.player;
    let system_program = &ctx.accounts.system_program;
    let record = &mut ctx.accounts.username_record;

    // 1. Manually Handle Profile Account (Creation or Allocation)
    if profile_info.data_is_empty() {
        let (pda, bump) = Pubkey::find_program_address(
            &[PROFILE_SEED, player.key().as_ref()], 
            ctx.program_id
        );
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
            &[player.to_account_info(), profile_info.to_account_info(), system_program.to_account_info()],
            &[&[PROFILE_SEED, player.key().as_ref(), &[bump]]],
        )?;
    } else {
        // Already exists - verify ownership
        require!(profile_info.owner == ctx.program_id, crate::errors::GameErrorCode::UnauthorizedAccess);
        
        // 2. Ensure enough space (Realloc if needed for legacy accounts)
        let required_space = 8 + PlayerProfile::INIT_SPACE;
        if profile_info.data_len() < required_space {
            profile_info.resize(required_space)?;
            
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


    // 2. Write Data using Manual Deserialization-like access
    let mut data = profile_info.try_borrow_mut_data()?;
    let mut profile = PlayerProfile::default();
    
    // Set initial values
    profile.authority = player.key();
    profile.elo_rating = 120000.0;
    profile.rd = 35000.0;
    profile.volatility = 600.0;
    profile.created_at = Clock::get()?.unix_timestamp;
    profile.is_verified = false;
    profile.username = username.clone();
    profile.username_set = true;

    // Write Discriminator
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

    msg!("Profile forced re-init success for {}", player.key());
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
