//! Convert a Braid move log into a PGN game and load it into PgnReplayState.
//!
//! After any multiplayer game ends the Braid move log stored by BraidIrohNode
//! contains the full ordered sequence of MovePayload values.  This module
//! converts that log into a ParsedPgnGame by:
//!   1. Parsing each UCI string via nimzovich_engine::parse_uci
//!   2. Generating SAN via nimzovich_engine::move_to_san (before applying the move)
//!   3. Applying the move so the engine stays in sync
//!
//! The resulting ParsedPgnGame can be inserted as ParsedPgnGameResource and the
//! existing replay UI plays it back immediately (no PGN text round-trip needed).

use braid_chess::MovePayload;
use nimzovich_engine::{
    do_move_with_promo, move_to_san, new_game, parse_uci, ParsedPgnGame,
};
use std::collections::BTreeMap;
use tracing::{info, warn};

/// Convert an ordered slice of [`MovePayload`] values into a [`ParsedPgnGame`].
///
/// Builds the SAN move list and PGN tags directly without going through a PGN text string.
/// Returns `None` if the move log is empty or any move fails to parse.
pub fn braid_move_log_to_parsed_pgn(
    moves: &[MovePayload],
    white_name: &str,
    black_name: &str,
    result: &str,
) -> Option<ParsedPgnGame> {
    if moves.is_empty() {
        return None;
    }

    let mut engine = new_game();
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
                warn!("[replay-braid] Failed to parse UCI '{}' at ply {}", payload.uci, idx + 1);
                return None;
            }
        };

        let san = move_to_san(&engine, src, dst, promo);
        let is_promo = promo != 0;
        do_move_with_promo(&mut engine, src, dst, is_promo, promo);
        san_moves.push(san);
    }

    let mut tags = BTreeMap::new();
    tags.insert("White".to_string(), white_name.to_string());
    tags.insert("Black".to_string(), black_name.to_string());
    tags.insert("Result".to_string(), result.to_string());

    info!("[replay-braid] Assembled ParsedPgnGame: {} half-moves", san_moves.len());
    Some(ParsedPgnGame {
        tags,
        moves: san_moves,
        result: result.to_string(),
        per_ply_annotations: Vec::new(),
    })
}

/// Build a PGN text string from a Braid move log.
///
/// Convenience wrapper around [`braid_move_log_to_parsed_pgn`] for cases where
/// the raw PGN text is needed (e.g., clipboard copy, file export).
pub fn braid_move_log_to_pgn_text(
    moves: &[MovePayload],
    white_name: &str,
    black_name: &str,
    result: &str,
) -> Option<String> {
    let pgn = braid_move_log_to_parsed_pgn(moves, white_name, black_name, result)?;

    let mut out = String::new();
    for (k, v) in &pgn.tags {
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
