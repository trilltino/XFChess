use xfchess_rules::Game;

pub struct ValidMove {
    pub uci: [u8; 5],
    pub next_board: [u8; 68],
}

pub fn generate_100_move_sequence() -> anyhow::Result<Vec<ValidMove>> {
    const SHUFFLE: [[u8; 5]; 4] = [*b"g1f3\0", *b"g8f6\0", *b"f3g1\0", *b"f6g8\0"];
    const FINISH: [[u8; 5]; 4] = [*b"f2f3\0", *b"e7e5\0", *b"g2g4\0", *b"d8h4\0"];
    let mut game = Game::default();
    let mut moves = Vec::with_capacity(100);

    // Return the knights home before the final Fool's Mate sequence.
    for uci in SHUFFLE.into_iter().cycle().take(96).chain(FINISH) {
        let text = std::str::from_utf8(&uci[..4])?;
        game.play_uci(text)
            .map_err(|error| anyhow::anyhow!("illegal benchmark move {text}: {error}"))?;
        moves.push(ValidMove {
            uci,
            next_board: game.compact_board().to_bytes(),
        });
    }
    Ok(moves)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_has_100_legal_moves_and_finishes_with_fools_mate() -> anyhow::Result<()> {
        let moves = generate_100_move_sequence()?;
        assert_eq!(moves.len(), 100);
        assert_eq!(moves[0].uci, *b"g1f3\0");
        assert_eq!(moves[95].uci, *b"f6g8\0");
        assert_eq!(moves[99].uci, *b"d8h4\0");
        Ok(())
    }
}
