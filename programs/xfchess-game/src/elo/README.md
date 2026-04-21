# ELO Rating System

ELO rating calculation and tournament ranking for competitive chess.

## Overview

The ELO rating system is a method for calculating the relative skill levels of players in zero-sum games. XFChess uses ELO ratings to:
- Track player skill progression
- Seed tournament brackets fairly
- Calculate expected match outcomes
- Provide competitive balance

## ELO Algorithm

XFChess uses the standard ELO formula:

```
Expected Score = 1 / (1 + 10^((R_B - R_A) / 400))

New Rating = Old Rating + K × (Actual Score - Expected Score)
```

Where:
- **R_A, R_B** - Current ratings of players A and B
- **K** - K-factor (maximum rating change per match)
- **Actual Score** - 1 for win, 0.5 for draw, 0 for loss

## K-Factor

The K-factor determines how much a rating can change after a match:
- **K = 32** - For new players (first 30 games)
- **K = 24** - For players with 30-100 games
- **K = 16** - For established players (100+ games)

## Components

- ELO calculation based on match results
- Tournament ranking by ELO
- Rating updates after each match
- Rating history tracking

## Example: ELO Calculation

This example shows how to calculate ELO rating changes after a match.

```rust
/// Calculates expected score for player A against player B
/// 
/// # Arguments
/// * `rating_a` - Player A's current rating
/// * `rating_b` - Player B's current rating
/// 
/// # Returns
/// Expected score for player A (0.0 to 1.0)
pub fn calculate_expected_score(rating_a: i32, rating_b: i32) -> f64 {
    let rating_difference = (rating_b - rating_a) as f64;
    let expected = 1.0 / (1.0 + 10_f64.powf(rating_difference / 400.0));
    expected
}

/// Calculates new ELO rating after a match
/// 
/// # Arguments
/// * `current_rating` - Player's current rating
/// * `opponent_rating` - Opponent's current rating
/// * `actual_score` - Player's actual score (1.0 for win, 0.5 for draw, 0.0 for loss)
/// * `k_factor` - K-factor for rating adjustment
/// 
/// # Returns
/// New ELO rating
pub fn calculate_new_rating(
    current_rating: i32,
    opponent_rating: i32,
    actual_score: f64,
    k_factor: f64,
) -> i32 {
    let expected_score = calculate_expected_score(current_rating, opponent_rating);
    let rating_change = k_factor * (actual_score - expected_score);
    let new_rating = (current_rating as f64 + rating_change).round() as i32;
    
    // Clamp rating to reasonable bounds
    new_rating.max(100).min(3000)
}

/// Determines K-factor based on games played
/// 
/// # Arguments
/// * `games_played` - Number of games player has played
/// 
/// # Returns
/// K-factor value
pub fn get_k_factor(games_played: u32) -> f64 {
    if games_played < 30 {
        32.0 // New players
    } else if games_played < 100 {
        24.0 // Developing players
    } else {
        16.0 // Established players
    }
}
```

## Example: Recording Match Result with ELO Update

This instruction records a match result and updates both players' ELO ratings.

```rust
use anchor_lang::prelude::*;

pub struct EloRating {
    pub rating: u32,
}

impl EloRating {
    pub const K_FACTOR: f64 = 32.0;
    
    pub fn calculate_expected_score(&self, opponent: &EloRating) -> f64 {
        let rating_diff = self.rating as f64 - opponent.rating as f64;
        1.0 / (1.0 + 10.0_f64.powf(-rating_diff / 400.0))
    }
    
    pub fn update_rating(&mut self, opponent: &EloRating, actual_score: f64) {
        let expected = self.calculate_expected_score(opponent);
        let change = (Self::K_FACTOR * (actual_score - expected)).round() as i32;
        
        self.rating = (self.rating as i32 + change).max(0) as u32;
    }
}

// Example usage
fn update_player_ratings(
    winner_rating: &mut EloRating,
    loser_rating: &mut EloRating,
) {
    // Winner gets 1.0, loser gets 0.0
    winner_rating.update_rating(loser_rating, 1.0);
    loser_rating.update_rating(winner_rating, 0.0);
}
```

## Example: Recording Match Result with ELO

```rust
#[derive(Accounts)]
pub struct RecordMatchWithElo<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    
    #[account(mut)]
    pub tournament_match: Account<'info, TournamentMatch>,
    
    #[account(mut)]
    pub player_white_entry: Account<'info, PlayerEntry>,
    
    #[account(mut)]
    pub player_black_entry: Account<'info, PlayerEntry>,
    
    pub authority: Signer<'info>,
}

pub fn record_match_with_elo(
    ctx: Context<RecordMatchWithElo>,
    winner: Pubkey,
) -> Result<()> {
    let tournament_match = &mut ctx.accounts.tournament_match;
    let player_white_entry = &mut ctx.accounts.player_white_entry;
    let player_black_entry = &mut ctx.accounts.player_black_entry;
    
    // Record match result
    tournament_match.winner = Some(winner);
    
    // Update ELO ratings
    let winner_is_white = winner == ctx.accounts.player_white_entry.player;
    
    if winner_is_white {
        let mut white_rating = EloRating {
            rating: player_white_entry.elo_rating,
        };
        let mut black_rating = EloRating {
            rating: player_black_entry.elo_rating,
        };
        
        update_player_ratings(&mut white_rating, &mut black_rating);
        
        player_white_entry.elo_rating = white_rating.rating;
        player_black_entry.elo_rating = black_rating.rating;
    } else {
        let mut white_rating = EloRating {
            rating: player_white_entry.elo_rating,
        };
        let mut black_rating = EloRating {
            rating: player_black_entry.elo_rating,
        };
        
        update_player_ratings(&mut black_rating, &mut white_rating);
        
        player_white_entry.elo_rating = white_rating.rating;
        player_black_entry.elo_rating = black_rating.rating;
    }
    
    // Update tournament standings
    update_tournament_standings(
        &mut ctx.accounts.tournament,
        &ctx.accounts.player_white_entry,
        &ctx.accounts.player_black_entry,
    );
    
    Ok(())
}

fn update_tournament_standings(
    tournament: &mut Tournament,
    player1: &PlayerEntry,
    player2: &PlayerEntry,
) {
    // Update tournament standings based on ELO and match results
    // Implementation depends on tournament structure
}
```

## Example: Tournament Ranking Calculation

```rust
pub struct TournamentRanking {
    pub player: Pubkey,
    pub elo_rating: u32,
    pub wins: u8,
    pub losses: u8,
    pub score: f64, // Calculated score for ranking
}

pub fn calculate_tournament_rankings(
    tournament: &Tournament,
    player_entries: &[PlayerEntry],
) -> Vec<TournamentRanking> {
    let mut rankings: Vec<TournamentRanking> = player_entries
        .iter()
        .map(|entry| TournamentRanking {
            player: entry.player,
            elo_rating: entry.elo_rating,
            wins: 0, // Calculate from match results
            losses: 0, // Calculate from match results
            score: entry.elo_rating as f64, // Base score on ELO
        })
        .collect();
    
    // Sort by score (descending)
    rankings.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    
    rankings
}
```
