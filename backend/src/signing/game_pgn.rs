use crate::db::repository::GameRepository;
use crate::signing::elo_cache::EloCache;
use nimzovich_engine::{PgnAssembler, PgnResult};

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
