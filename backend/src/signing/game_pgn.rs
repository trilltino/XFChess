//! Shared PGN assembly for finished games.
//!
//! Used by both the manual `/game/finalize` route (`routes::main::finalize_game`)
//! and the automatic settlement worker (`tasks::settlement_worker::finalize_on_chain`)
//! so a game gets an identical, fully-tagged PGN regardless of which path
//! finalized it on-chain.

use crate::db::repository::GameRepository;
use crate::signing::elo_cache::EloCache;
use nimzovich_engine::{PgnAssembler, PgnResult};

/// Assemble a PGN for a finished game and persist it via `set_pgn_text`.
///
/// `white`/`black` are wallet pubkey strings; `white_username`/`black_username`
/// are the resolved display names (falls back to the pubkey when no username
/// is set). ELO ratings are looked up live from `elo_cache` so the PGN reflects
/// each wallet's rating at finalize time — the same source of truth the ratings
/// UI reads from, not a stale value carried from earlier in the request.
pub async fn assemble_and_store_pgn(
    repo: &GameRepository,
    elo_cache: &EloCache,
    game_id_str: &str,
    white: &str,
    black: &str,
    white_username: Option<&str>,
    black_username: Option<&str>,
    winner: Option<&str>,
) {
    let moves = match repo.get_moves(game_id_str).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("[PGN] Failed to load moves for game {}: {}", game_id_str, e);
            return;
        }
    };

    let white_name = white_username.unwrap_or(white);
    let black_name = black_username.unwrap_or(black);
    let date = chrono::Utc::now().format("%Y.%m.%d").to_string();

    let mut assembler = PgnAssembler::new();
    assembler
        .tag("Event", &format!("{} vs {}", white_name, black_name))
        .tag("Site", "XFChess")
        .tag("Date", &date)
        .tag("White", white_name)
        .tag("Black", black_name);

    // ELO tags are best-effort — an RPC/cache miss just omits them rather
    // than failing PGN assembly for the whole game.
    if let Ok(elo) = elo_cache.get_elo(white).await {
        assembler.tag("WhiteElo", &format!("{}", elo.elo_rating.round() as i64));
    }
    if let Ok(elo) = elo_cache.get_elo(black).await {
        assembler.tag("BlackElo", &format!("{}", elo.elo_rating.round() as i64));
    }

    for mv in moves {
        if let Some(san) = mv.move_san {
            assembler.add_move(san);
        }
    }

    let result = match winner {
        Some("white") => PgnResult::WhiteWins,
        Some("black") => PgnResult::BlackWins,
        _ => PgnResult::Draw,
    };
    assembler.set_result(result);

    let pgn = assembler.to_string();
    if let Err(e) = repo.set_pgn_text(game_id_str, &pgn).await {
        tracing::error!("[PGN] Failed to store PGN for game {}: {}", game_id_str, e);
    }
}
