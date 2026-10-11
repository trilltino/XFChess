use super::*;

#[derive(Resource, Default)]
pub struct CachedGamePgn {
    pub pgn: Option<nimzovich_engine::ParsedPgnGame>,
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
    use crate::game::components::PieceType;
    use nimzovich_engine::{do_move_with_promo, game_to_fen, new_game_no_tt};

    let mut game = new_game_no_tt();
    for rec in &history.moves {
        let src = rec.from.1 as i8 * 8 + rec.from.0 as i8;
        let dst = rec.to.1 as i8 * 8 + rec.to.0 as i8;
        let is_promo = rec.piece_type == PieceType::Pawn && (rec.to.1 == 7 || rec.to.1 == 0);
        let promo: i8 = if is_promo { 5 } else { 0 };
        do_move_with_promo(&mut game, src, dst, true, promo);
    }
    game_to_fen(&game)
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
) -> nimzovich_engine::ParsedPgnGame {
    use crate::game::components::PieceType;
    use nimzovich_engine::{do_move_with_promo, move_to_san, new_game_no_tt};
    use std::collections::BTreeMap;

    // No search ever runs on this Game (just replay + SAN), so skip the
    // multi-GB transposition table `new_game` would otherwise allocate.
    let mut game = new_game_no_tt();
    let mut san_moves: Vec<String> = Vec::with_capacity(history.moves.len());

    for rec in &history.moves {
        let src = rec.from.1 as i8 * 8 + rec.from.0 as i8;
        let dst = rec.to.1 as i8 * 8 + rec.to.0 as i8;
        // Default promo to queen (5); MoveRecord doesn't store promoted piece type
        let is_promo = rec.piece_type == PieceType::Pawn && (rec.to.1 == 7 || rec.to.1 == 0);
        let promo: i8 = if is_promo { 5 } else { 0 };

        let san = move_to_san(&mut game, src, dst, promo);
        san_moves.push(san);
        do_move_with_promo(&mut game, src, dst, true, promo);
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

    nimzovich_engine::ParsedPgnGame {
        tags,
        moves: san_moves,
        result: result_str.to_string(),
        per_ply_annotations: Vec::new(),
    }
}

pub fn pgn_to_string(pgn: &nimzovich_engine::ParsedPgnGame) -> String {
    let mut out = String::new();
    for (k, v) in nimzovich_engine::ordered_tags(&pgn.tags) {
        out.push_str(&format!("[{} \"{}\"]\n", k, v));
    }
    out.push('\n');

    let mut line = String::new();
    let flush = |line: &mut String, out: &mut String| {
        if !line.is_empty() {
            out.push_str(line.trim_end());
            out.push('\n');
            line.clear();
        }
    };
    for (ply, mv) in pgn.moves.iter().enumerate() {
        let token = if ply.is_multiple_of(2) {
            format!("{}. {}", ply / 2 + 1, mv)
        } else {
            mv.clone()
        };
        if !line.is_empty() && line.len() + 1 + token.len() > 80 {
            flush(&mut line, &mut out);
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&token);
    }
    if !line.is_empty() {
        if line.len() + 1 + pgn.result.len() > 80 {
            out.push_str(line.trim_end());
            out.push('\n');
            out.push_str(&pgn.result);
        } else {
            out.push_str(line.trim_end());
            out.push(' ');
            out.push_str(&pgn.result);
        }
    } else {
        out.push_str(&pgn.result);
    }
    out.push('\n');
    out
}
