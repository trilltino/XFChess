use super::*;

#[derive(Resource, Default)]
pub struct CachedGamePgn {
    pub pgn: Option<xfchess_rules::ParsedPgnGame>,
    pub pgn_string: String,
    pub final_fen: String,
    pub braid_pgn_ready: bool,
}

pub fn cache_pgn_on_game_over(
    history: Res<MoveHistory>,
    game_over: Res<GameOverState>,
    game_mode: Res<crate::core::GameMode>,
    players: Res<crate::game::resources::player::Players>,
    mut cached: ResMut<CachedGamePgn>,
) {
    let pgn_result = match game_over.winner() {
        Some(PieceColor::White) => "1-0",
        Some(PieceColor::Black) => "0-1",
        None => "1/2-1/2",
    };
    let white_name = players.current(PieceColor::White).name.clone();
    let black_name = players.current(PieceColor::Black).name.clone();
    let pgn = build_pgn(&history, pgn_result, &white_name, &black_name);
    cached.pgn_string = pgn_to_string(&pgn);
    cached.final_fen = build_final_fen(&history);
    cached.pgn = Some(pgn);
    // Offline PGN is final immediately. Online games wait for the authoritative move log
    // before enabling Review and Save.
    let is_online = matches!(
        *game_mode,
        crate::core::GameMode::OnlineMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );
    cached.braid_pgn_ready = !is_online;
}

pub(super) fn build_final_fen(history: &MoveHistory) -> String {
    let mut game = xfchess_rules::Game::default();
    for rec in &history.moves {
        if let Err(e) = game.play_uci(&rec.uci()) {
            warn!("PGN move history: {e}");
            break;
        }
    }
    game.fen()
}

pub(super) fn elo_tier(elo: u32) -> Option<&'static str> {
    match elo {
        0..=1199 => None,
        1200..=1399 => Some("Beginner"),
        1400..=1599 => Some("Intermediate"),
        1600..=1799 => Some("Advanced"),
        1800..=1999 => Some("Expert"),
        2000..=2199 => Some("Master"),
        2200..=2399 => Some("International Master"),
        _ => Some("Grandmaster"),
    }
}

pub(super) fn build_pgn(
    history: &MoveHistory,
    result_str: &str,
    white_name: &str,
    black_name: &str,
) -> xfchess_rules::ParsedPgnGame {
    use std::collections::BTreeMap;
    let mut game = xfchess_rules::Game::default();
    let mut san_moves = Vec::with_capacity(history.moves.len());
    for rec in &history.moves {
        match game.san(&rec.uci()) {
            Ok(san) => san_moves.push(san),
            Err(e) => {
                warn!("PGN move history: {e}");
                break;
            }
        }
        if let Err(e) = game.play_uci(&rec.uci()) {
            warn!("PGN move history: {e}");
            break;
        }
    }

    let mut tags = BTreeMap::new();
    tags.insert(
        "Event".to_string(),
        format!("{} vs {}", white_name, black_name),
    );
    tags.insert("Site".to_string(), "XFChess".to_string());
    tags.insert(
        "Date".to_string(),
        chrono::Local::now().format("%Y.%m.%d").to_string(),
    );
    tags.insert("White".to_string(), white_name.to_string());
    tags.insert("Black".to_string(), black_name.to_string());
    tags.insert("Result".to_string(), result_str.to_string());

    xfchess_rules::ParsedPgnGame {
        tags,
        moves: san_moves,
        result: result_str.to_string(),
        per_ply_annotations: Vec::new(),
    }
}

pub fn pgn_to_string(pgn: &xfchess_rules::ParsedPgnGame) -> String {
    xfchess_rules::pgn_to_string(pgn)
}
