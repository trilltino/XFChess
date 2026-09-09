use braid_chess::MovePayload;
use nimzovich_engine::{do_move_with_promo, move_to_san, new_game_no_tt, parse_uci, ParsedPgnGame};
use std::collections::BTreeMap;
use tracing::{info, warn};

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

    // No search ever runs on this Game (just replay + SAN), so skip the
    // multi-GB transposition table `new_game` would otherwise allocate.
    let mut engine = new_game_no_tt();
    let mut san_moves: Vec<String> = Vec::with_capacity(moves.len());

    for (idx, payload) in moves.iter().enumerate() {
        let uci = payload.uci.as_bytes();
        // UCI is 4 chars ("e2e4") or 5 chars ("e7e8q"); pad to exactly 5 bytes.
        let mut buf = [b' '; 5];
        let copy_len = uci.len().min(5);
        buf[..copy_len].copy_from_slice(&uci[..copy_len]);

        let (src, dst, promo) = match parse_uci(&buf) {
            Ok(t) => t,
            Err(_) => {
                warn!(
                    "[replay-braid] Failed to parse UCI '{}' at ply {}",
                    payload.uci,
                    idx + 1
                );
                return None;
            }
        };

        let san = move_to_san(&mut engine, src, dst, promo);
        let is_promo = promo != 0;
        do_move_with_promo(&mut engine, src, dst, is_promo, promo);
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

    let mut out = String::new();
    for (k, v) in nimzovich_engine::ordered_tags(&pgn.tags) {
        out.push_str(&format!("[{} \"{}\"]\n", k, v));
    }
    out.push('\n');

    for (i, san) in pgn.moves.iter().enumerate() {
        if i % 2 == 0 {
            out.push_str(&format!("{}. ", i / 2 + 1));
        }
        out.push_str(san);
        out.push(' ');
    }
    out.push_str(result);
    Some(out)
}
