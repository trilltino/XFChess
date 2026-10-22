use anyhow::Context as _;
use xfchess_rules::*;

#[test]
fn starting_position_perft_and_san() -> anyhow::Result<()> {
    let game = Game::default();
    assert_eq!(game.legal_moves().len(), 20);
    let nodes: usize = game
        .legal_moves()
        .iter()
        .map(|mv| {
            let mut child = game.clone();
            child
                .play_uci(mv)
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("required operation value")?;
            Ok::<_, anyhow::Error>(child.legal_moves().len())
        })
        .sum::<anyhow::Result<usize>>()?;
    assert_eq!(nodes, 400);
    assert_eq!(
        game.san("e2e4")
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?,
        "e4"
    );
    assert_eq!(game.fen(), STARTING_FEN);
    Ok(())
}

#[test]
fn castling_en_passant_and_pins() -> anyhow::Result<()> {
    let mut game = Game::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert!(!game.is_legal_uci("e1a1"));
    assert!(!game.is_legal_uci("e1h1"));
    assert_eq!(
        game.san("e1g1")
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?,
        "O-O"
    );
    game.play_uci("e1g1")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert_eq!(game.board[5], ROOK_ID);
    assert_eq!(game.board[6], KING_ID);
    assert_eq!(game.compact_board().castling, 12);
    let mut ep = Game::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    ep.play_uci("e5d6")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert_eq!(ep.board[35], 0);
    assert_eq!(ep.board[43], PAWN_ID);
    let pinned = Game::from_fen("4r1k1/8/8/8/8/8/4R3/4K3 w - - 0 1")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert!(!pinned.is_legal_uci("e2d2"));
    assert!(pinned.is_legal_uci("e2e8"));
    Ok(())
}

#[test]
fn all_promotions_and_strict_validation() -> anyhow::Result<()> {
    let start = "7k/P7/8/8/8/8/8/7K w - - 0 1";
    for (suffix, id) in [
        ('q', QUEEN_ID),
        ('r', ROOK_ID),
        ('b', BISHOP_ID),
        ('n', KNIGHT_ID),
    ] {
        let mut game = Game::from_fen(start)
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?;
        let uci = format!("a7a8{suffix}");
        assert!(game
            .san(&uci)
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?
            .contains('='));
        game.play_uci(&uci)
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?;
        assert_eq!(game.board[56], id);
    }
    let game = Game::from_fen(start)
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    for bad in [
        "a7a8", "a7a8k", "e2e4q", "a0a1", "i1a1", "0000", "é2e4", "a7a8qq",
    ] {
        assert!(!game.is_legal_uci(bad), "{bad}");
    }
    for bad in ["", "8/8/8/8/8/8/8/8 w - - 0 1", "garbage"] {
        assert!(Game::from_fen(bad).is_err());
    }
    Ok(())
}

#[test]
fn mate_and_stalemate() -> anyhow::Result<()> {
    let mate = Game::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert!(mate.is_check());
    assert!(mate.is_checkmate());
    assert!(mate.legal_moves().is_empty());
    let stale = Game::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert!(stale.is_stalemate());
    assert!(!stale.is_check());
    Ok(())
}

#[test]
fn compact_board_matches_contract_layout_and_legacy_fen() -> anyhow::Result<()> {
    let mut game = Game::default();
    game.play_uci("e2e4")
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    let compact = game.compact_board();
    let bytes = compact.to_bytes();
    assert_eq!(bytes.len(), 68);
    assert_eq!(bytes[0], 4);
    assert_eq!(bytes[60], (-6i8) as u8);
    assert_eq!(bytes[64], 15);
    assert_eq!(bytes[65], 20);
    assert_eq!(bytes[66], 255);
    assert_eq!(bytes[67], 0);
    assert_eq!(
        compact.to_fen().map_err(anyhow::Error::msg)?,
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
    Ok(())
}

#[test]
fn invalid_compact_boards_return_errors() {
    let board = Game::default().compact_board();
    for piece in [i8::MIN, -7, 7, i8::MAX] {
        let mut invalid = board;
        invalid.squares[0] = piece;
        assert!(invalid.to_fen().is_err());
    }
    let mut invalid = board;
    invalid.castling = 16;
    assert!(invalid.to_fen().is_err());
    invalid = board;
    invalid.ep_target = 64;
    assert!(invalid.to_fen().is_err());
    invalid = board;
    invalid.side_to_move = 0;
    assert!(invalid.to_fen().is_err());
}

#[test]
fn pgn_preserves_annotations_and_validates_mainline() -> anyhow::Result<()> {
    let input="[Event \"Escaped \\\"name\\\"\"]\n[Result \"1-0\"]\n\n{Opening comment} 1. e4 $3 {great [%cal Ge2e4] [%csl Ye4]} (1. d4 d5) e5 2. Nf3 1-0";
    let pgn = parse_pgn(input)
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert_eq!(pgn.moves, vec!["e4", "e5", "Nf3"]);
    assert_eq!(pgn.result, "1-0");
    assert_eq!(pgn.per_ply_annotations[1].quality, MoveQuality::Brilliant);
    assert_eq!(pgn.per_ply_annotations[1].arrows, vec![(4, 1, 4, 3, 0)]);
    assert_eq!(pgn.per_ply_annotations[1].highlights, vec![(4, 3, 1)]);
    let roundtrip = parse_pgn(&pgn_to_string(&pgn))
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert_eq!(roundtrip, pgn);
    assert_eq!(
        pgn.snapshots()
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?
            .len(),
        4
    );
    assert!(parse_pgn("1. e5 *").is_err());
    Ok(())
}

#[test]
fn custom_start_black_move_and_underpromotion_roundtrip() -> anyhow::Result<()> {
    let input = "[SetUp \"1\"]\n[FEN \"7k/8/8/8/8/8/p7/7K b - - 0 8\"]\n\n8... a1=N *";
    let pgn = parse_pgn(input)
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    let snapshots = pgn
        .snapshots()
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("required operation value")?;
    assert_eq!(
        Game::from_fen(snapshots.last().context("required operation value")?)
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?
            .board[0],
        -KNIGHT_ID
    );
    assert!(pgn_to_string(&pgn).contains("8... a1=N"));
    assert_eq!(
        parse_pgn(&pgn_to_string(&pgn))
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?
            .snapshots()
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?,
        snapshots
    );
    Ok(())
}
