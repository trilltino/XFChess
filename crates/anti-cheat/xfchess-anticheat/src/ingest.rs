use crate::error::{AcError, AcResult};
use crate::types::{GameContext, GameRecord, GameResult, MoveRecord, PlayerRef, TimeControl};
#[cfg(test)]
use anyhow::Context as _;

pub struct MoveRow {
    pub move_number: i64,
    pub move_uci: String,
    pub fen_after: Option<String>,
    pub player: String,
    pub timestamp: i64,
    pub blurred: bool,
    pub think_ms: Option<u32>,
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

pub fn build_game_record(rows: &[MoveRow], meta: &GameMeta) -> AcResult<GameRecord> {
    if rows.is_empty() {
        return Err(AcError::InsufficientMoves(meta.game_id.clone(), 0));
    }

    let context = match meta.tournament_id {
        Some(tid) => GameContext::Tournament {
            tournament_id: tid,
            round: meta.tournament_round.unwrap_or(1),
        },
        None => GameContext::Pvp {
            wager_sol: meta.stake_amount,
        },
    };

    let result = match meta.winner.as_deref() {
        Some("white") => GameResult::WhiteWin,
        Some("black") => GameResult::BlackWin,
        _ => GameResult::Draw,
    };

    let mut moves: Vec<MoveRecord> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let ply = u32::try_from(i).map_err(|_| AcError::InvalidInput("too many moves".into()))?;
        if row.move_number != i64::from(ply) + 1 {
            return Err(AcError::InvalidInput(
                "moves must be contiguous and start at one".into(),
            ));
        }
        let player = if i % 2 == 0 {
            &meta.player_white
        } else {
            &meta.player_black
        };
        if &row.player != player {
            return Err(AcError::InvalidInput(format!(
                "incorrect player for move {}",
                row.move_number
            )));
        }
        let prev_ts = if i == 0 {
            row.timestamp
        } else {
            rows[i - 1].timestamp
        };
        let timestamp_ms = u64::try_from(row.timestamp)
            .ok()
            .and_then(|timestamp| timestamp.checked_mul(1000))
            .ok_or_else(|| AcError::InvalidInput("move timestamp is out of range".into()))?;
        let elapsed = row
            .timestamp
            .checked_sub(prev_ts)
            .filter(|elapsed| *elapsed >= 0)
            .ok_or_else(|| AcError::InvalidInput("move timestamps must be ordered".into()))?;
        let latency_ms = u64::try_from(elapsed)
            .ok()
            .and_then(|elapsed| elapsed.checked_mul(1000))
            .map(|elapsed| u32::try_from(elapsed).unwrap_or(u32::MAX))
            .ok_or_else(|| AcError::InvalidInput("move duration is out of range".into()))?;

        moves.push(MoveRecord {
            ply,
            move_uci: row.move_uci.clone(),
            fen_after: row.fen_after.clone().unwrap_or_default(),
            signed_at_ms: timestamp_ms,
            latency_ms,
            blurred: row.blurred,
            think_ms: row.think_ms,
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
        ended_at_ms: u64::try_from(meta.end_time.unwrap_or(0))
            .ok()
            .and_then(|time| time.checked_mul(1000))
            .ok_or_else(|| AcError::InvalidInput("game end timestamp is out of range".into()))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(n_moves: usize) -> (Vec<MoveRow>, GameMeta) {
        let rows = (0..n_moves)
            .map(|i| MoveRow {
                move_number: i as i64 + 1,
                move_uci: "e2e4".into(),
                fen_after: Some("startpos".into()),
                player: if i % 2 == 0 {
                    "WHITE_PUBKEY"
                } else {
                    "BLACK_PUBKEY"
                }
                .into(),
                timestamp: 1_700_000_000 + i as i64 * 3,
                blurred: false,
                think_ms: None,
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
    fn builds_correctly() -> anyhow::Result<()> {
        let (rows, meta) = meta(40);
        let rec = build_game_record(&rows, &meta).context("required operation value")?;
        assert_eq!(rec.moves.len(), 40);
        assert_eq!(rec.result, GameResult::WhiteWin);
        assert_eq!(rec.white.elo, 1500);
        Ok(())
    }

    #[test]
    fn empty_rows_errors() {
        let (_, meta) = meta(0);
        assert!(build_game_record(&[], &meta).is_err());
    }

    #[test]
    fn ingestion_normalizes_real_move_numbers_and_rejects_corrupt_rows() -> anyhow::Result<()> {
        let (mut rows, metadata) = meta(4);
        let record = build_game_record(&rows, &metadata)?;
        assert_eq!(
            record.moves.iter().map(|mv| mv.ply).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert_eq!(record.moves[1].latency_ms, 3_000);
        rows[1].move_number = 1;
        assert!(build_game_record(&rows, &metadata).is_err());
        rows[1].move_number = 2;
        rows[1].player = metadata.player_white.clone();
        assert!(build_game_record(&rows, &metadata).is_err());
        rows[1].player = metadata.player_black.clone();
        rows[1].timestamp = -1;
        assert!(build_game_record(&rows, &metadata).is_err());
        rows[1].timestamp = i64::MAX;
        assert!(build_game_record(&rows, &metadata).is_err());
        Ok(())
    }
}
