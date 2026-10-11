use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

/// Records one board of the current Swiss round. Pairings are computed off-chain,
/// so — like `record_match_result` for single elimination — only the tournament
/// authority may report, naming the board's white and black players. A bye is
/// reported as its own board after the round's real ones, with `Bye` and the bye
/// player passed as both `white` and `black`.
#[derive(Accounts)]
#[instruction(tournament_id: u64, round: u8, board: u16)]
pub struct RecordSwissResult<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Account<'info, TournamentPlayersShard>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Account<'info, TournamentPlayersShard>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Account<'info, TournamentPlayersShard>>,

    pub authority: Signer<'info>,

    /// CHECK: only its key is used; it must be a registered player.
    pub white: UncheckedAccount<'info>,
    /// CHECK: only its key is used; it must be a registered player.
    pub black: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwissMatchResult {
    WhiteWin,
    BlackWin,
    Draw,
    /// The player passed as both `white` and `black` sat out the round: a full point.
    Bye,
}

/// Board slots per round: every pairing plus at most one bye, so ⌈players / 2⌉.
pub fn board_slots(num_registered_players: u16) -> u16 {
    num_registered_players.max(2).div_ceil(2)
}

pub fn handler(
    ctx: Context<RecordSwissResult>,
    tournament_id: u64,
    round: u8,
    board: u16,
    result: SwissMatchResult,
) -> Result<()> {
    let t = &mut ctx.accounts.tournament;
    require!(
        t.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    let white = ctx.accounts.white.key();
    let black = ctx.accounts.black.key();
    if result == SwissMatchResult::Bye {
        require_keys_eq!(white, black, GameErrorCode::InvalidArgument);
    } else {
        require_keys_neq!(white, black, GameErrorCode::InvalidArgument);
    }

    require!(
        t.status == TournamentStatus::Active,
        GameErrorCode::InvalidGameStatus
    );
    require!(
        matches!(t.tournament_type, TournamentType::Swiss { .. }),
        GameErrorCode::InvalidGameStatus
    );
    require!(round == t.current_round, GameErrorCode::InvalidGameStatus);
    require!(round < t.total_rounds, GameErrorCode::InvalidGameStatus);
    require!(
        board < board_slots(t.num_registered_players),
        GameErrorCode::InvalidArgument
    );
    require!(
        !crate::tournament_ix::matches::round_bitmap::is_set(&t.round_boards_reported, board),
        GameErrorCode::BoardAlreadyRecorded
    );

    let mut shards: Vec<&mut TournamentPlayersShard> =
        vec![&mut ctx.accounts.tournament_players_shard_0];
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_mut() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_mut() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_mut() {
        shards.push(s);
    }

    let (white_points, black_points) = match result {
        SwissMatchResult::WhiteWin => (2u8, 0u8),
        SwissMatchResult::BlackWin => (0, 2),
        SwissMatchResult::Draw => (1, 1),
        SwissMatchResult::Bye => (2, 0),
    };
    let white_standing = find_standing(&mut shards, &white)?;
    white_standing.score = white_standing.score.saturating_add(white_points);
    if result != SwissMatchResult::Bye {
        white_standing.color_balance = white_standing.color_balance.saturating_add(1);
        let black_standing = find_standing(&mut shards, &black)?;
        black_standing.score = black_standing.score.saturating_add(black_points);
        black_standing.color_balance = black_standing.color_balance.saturating_sub(1);
    }

    crate::tournament_ix::matches::round_bitmap::set(&mut t.round_boards_reported, board);

    msg!(
        "Swiss result recorded: round {} board {} {} (W) vs {} (B) {:?}",
        round,
        board,
        white,
        black,
        result
    );

    Ok(())
}

fn find_standing<'a>(
    shards: &'a mut [&mut TournamentPlayersShard],
    player: &Pubkey,
) -> Result<&'a mut SwissStanding> {
    shards
        .iter_mut()
        .flat_map(|shard| shard.swiss_standings.iter_mut())
        .find(|standing| standing.player == *player)
        .ok_or_else(|| GameErrorCode::PlayerNotFound.into())
}
