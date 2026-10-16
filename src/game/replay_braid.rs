use braid_chess::MovePayload;
use std::collections::BTreeMap;
use tracing::{info, warn};
use xfchess_rules::{new_game, ParsedPgnGame};

pub fn braid_move_log_to_parsed_pgn(
    moves: &[MovePayload],
    white_name: &str,
    black_name: &str,
    result: &str,
) -> Option<ParsedPgnGame> {
    braid_move_log_to_parsed_pgn_rated(moves, white_name, black_name, None, None, result)
}

#[allow(clippy::too_many_arguments)]
pub fn braid_move_log_to_parsed_pgn_rated(
    moves: &[MovePayload],
    white_name: &str,
    black_name: &str,
    white_elo: Option<u32>,
    black_elo: Option<u32>,
    result: &str,
) -> Option<ParsedPgnGame> {
    if moves.is_empty() {
        return None;
    }

    // Rules positions are independent of the AI process.
    let mut engine = new_game();
    let mut san_moves: Vec<String> = Vec::with_capacity(moves.len());

    for (idx, payload) in moves.iter().enumerate() {
        let san = match engine.san(&payload.uci) {
            Ok(san) => san,
            Err(e) => {
                warn!("[replay-braid] Invalid ply {}: {e}", idx + 1);
                return None;
            }
        };
        if engine.play_uci(&payload.uci).is_err() {
            return None;
        }
        san_moves.push(san);
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
    tags.insert("Result".to_string(), result.to_string());
    if let Some(elo) = white_elo {
        tags.insert("WhiteElo".to_string(), elo.to_string());
    }
    if let Some(elo) = black_elo {
        tags.insert("BlackElo".to_string(), elo.to_string());
    }

    info!(
        "[replay-braid] Assembled ParsedPgnGame: {} half-moves",
        san_moves.len()
    );
    Some(ParsedPgnGame {
        tags,
        moves: san_moves,
        result: result.to_string(),
        per_ply_annotations: Vec::new(),
    })
}

pub fn braid_move_log_to_pgn_text(
    moves: &[MovePayload],
    white_name: &str,
    black_name: &str,
    result: &str,
) -> Option<String> {
    let pgn = braid_move_log_to_parsed_pgn(moves, white_name, black_name, result)?;

    Some(xfchess_rules::pgn_to_string(&pgn))
}
