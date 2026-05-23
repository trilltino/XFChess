use crate::error::{AcError, AcResult};
use crate::types::{GameContext, GameRecord, GameResult, MoveRecord, PlayerRef, TimeControl};

/// Raw DB row shape — matches the `moves` + `games` tables from migration 006.
pub struct MoveRow {
    pub move_number: i64,
    pub move_uci: String,
    pub fen_after: Option<String>,
    pub player: String,
    pub timestamp: i64,
}

pub struct GameMeta {
    pub game_id: String,
    pub player_white: String,
    pub player_black: String,
    pub white_elo: u32,
    pub black_elo: u32,
    pub stake_amount: f64,
    pub tournament_id: Option<u64>,
    pub tournament_round: Option<u32>,
    pub winner: Option<String>,
    pub end_time: Option<i64>,
    pub time_base_sec: u32,
    pub time_inc_sec: u32,
}

/// Pure function: DB rows → `GameRecord`.
pub fn build_game_record(rows: &[MoveRow], meta: &GameMeta) -> AcResult<GameRecord> {
    if rows.is_empty() {
        return Err(AcError::InsufficientMoves(meta.game_id.clone(), 0));
    }

    let context = match meta.tournament_id {
        Some(tid) => GameContext::Tournament {
            tournament_id: tid,
            round: meta.tournament_round.unwrap_or(1),
        },
        None => GameContext::Pvp { wager_sol: meta.stake_amount },
    };

    let result = match meta.winner.as_deref() {
        Some("white") => GameResult::WhiteWin,
        Some("black") => GameResult::BlackWin,
        _ => GameResult::Draw,
    };

    // Build move records with server-timestamp latencies
    let mut moves: Vec<MoveRecord> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let prev_ts = if i == 0 { row.timestamp } else { rows[i - 1].timestamp };
        let latency_ms = ((row.timestamp - prev_ts).max(0) * 1000) as u32;

        moves.push(MoveRecord {
            ply: row.move_number as u32,
            move_uci: row.move_uci.clone(),
            fen_after: row.fen_after.clone().unwrap_or_default(),
            signed_at_ms: (row.timestamp * 1000) as u64,
            latency_ms,
        });
    }

    Ok(GameRecord {
        game_id: meta.game_id.clone(),
        context,
        white: PlayerRef {
            pubkey: meta.player_white.clone(),
            elo: meta.white_elo,
        },
        black: PlayerRef {
            pubkey: meta.player_black.clone(),
            elo: meta.black_elo,
        },
        time_control: TimeControl {
            base_sec: meta.time_base_sec,
            inc_sec: meta.time_inc_sec,
        },
        start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".into(),
        moves,
        result,
        ended_at_ms: meta.end_time.unwrap_or(0) as u64 * 1000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(n_moves: usize) -> (Vec<MoveRow>, GameMeta) {
        let rows = (0..n_moves)
            .map(|i| MoveRow {
                move_number: i as i64,
                move_uci: "e2e4".into(),
                fen_after: Some("startpos".into()),
                player: "white".into(),
                timestamp: 1_700_000_000 + i as i64 * 3,
            })
            .collect();
        let meta = GameMeta {
            game_id: "test-1".into(),
            player_white: "WHITE_PUBKEY".into(),
            player_black: "BLACK_PUBKEY".into(),
            white_elo: 1500,
            black_elo: 1400,
            stake_amount: 0.1,
            tournament_id: None,
            tournament_round: None,
            winner: Some("white".into()),
            end_time: Some(1_700_001_000),
            time_base_sec: 600,
            time_inc_sec: 0,
        };
        (rows, meta)
    }

    #[test]
    fn builds_correctly() {
        let (rows, meta) = meta(40);
        let rec = build_game_record(&rows, &meta).unwrap();
        assert_eq!(rec.moves.len(), 40);
        assert_eq!(rec.result, GameResult::WhiteWin);
        assert_eq!(rec.white.elo, 1500);
    }

    #[test]
    fn empty_rows_errors() {
        let (_, meta) = meta(0);
        assert!(build_game_record(&[], &meta).is_err());
    }
}
