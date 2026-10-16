use xfchess_rules::*;

#[test]
fn starting_position_perft_and_san() {
    let game = Game::default();
    assert_eq!(game.legal_moves().len(), 20);
    let nodes: usize = game
        .legal_moves()
        .iter()
        .map(|mv| {
            let mut child = game.clone();
            child.play_uci(mv).unwrap();
            child.legal_moves().len()
        })
        .sum();
    assert_eq!(nodes, 400);
    assert_eq!(game.san("e2e4").unwrap(), "e4");
    assert_eq!(game.fen(), STARTING_FEN);
}

#[test]
fn castling_en_passant_and_pins() {
    let mut game = Game::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    assert!(!game.is_legal_uci("e1a1"));
    assert!(!game.is_legal_uci("e1h1"));
    assert_eq!(game.san("e1g1").unwrap(), "O-O");
    game.play_uci("e1g1").unwrap();
    assert_eq!(game.board[5], ROOK_ID);
    assert_eq!(game.board[6], KING_ID);
    assert_eq!(game.compact_board().castling, 12);
    let mut ep = Game::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2").unwrap();
    ep.play_uci("e5d6").unwrap();
    assert_eq!(ep.board[35], 0);
    assert_eq!(ep.board[43], PAWN_ID);
    let pinned = Game::from_fen("4r1k1/8/8/8/8/8/4R3/4K3 w - - 0 1").unwrap();
    assert!(!pinned.is_legal_uci("e2d2"));
    assert!(pinned.is_legal_uci("e2e8"));
}

#[test]
fn all_promotions_and_strict_validation() {
    let start = "7k/P7/8/8/8/8/8/7K w - - 0 1";
    for (suffix, id) in [
        ('q', QUEEN_ID),
        ('r', ROOK_ID),
        ('b', BISHOP_ID),
        ('n', KNIGHT_ID),
    ] {
        let mut game = Game::from_fen(start).unwrap();
        let uci = format!("a7a8{suffix}");
        assert!(game.san(&uci).unwrap().contains('='));
        game.play_uci(&uci).unwrap();
        assert_eq!(game.board[56], id);
    }
    let game = Game::from_fen(start).unwrap();
    for bad in [
        "a7a8", "a7a8k", "e2e4q", "a0a1", "i1a1", "0000", "é2e4", "a7a8qq",
    ] {
        assert!(!game.is_legal_uci(bad), "{bad}");
    }
    for bad in ["", "8/8/8/8/8/8/8/8 w - - 0 1", "garbage"] {
        assert!(Game::from_fen(bad).is_err());
    }
}

#[test]
fn mate_and_stalemate() {
    let mate =
        Game::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3").unwrap();
    assert!(mate.is_check());
    assert!(mate.is_checkmate());
    assert!(mate.legal_moves().is_empty());
    let stale = Game::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert!(stale.is_stalemate());
    assert!(!stale.is_check());
}

#[test]
fn compact_board_matches_contract_layout_and_legacy_fen() {
    let mut game = Game::default();
    game.play_uci("e2e4").unwrap();
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
        compact.to_fen(),
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
}

#[test]
fn pgn_preserves_annotations_and_validates_mainline() {
    let input="[Event \"Escaped \\\"name\\\"\"]\n[Result \"1-0\"]\n\n{Opening comment} 1. e4 $3 {great [%cal Ge2e4] [%csl Ye4]} (1. d4 d5) e5 2. Nf3 1-0";
    let pgn = parse_pgn(input).unwrap();
    assert_eq!(pgn.moves, vec!["e4", "e5", "Nf3"]);
    assert_eq!(pgn.result, "1-0");
    assert_eq!(pgn.per_ply_annotations[1].quality, MoveQuality::Brilliant);
    assert_eq!(pgn.per_ply_annotations[1].arrows, vec![(4, 1, 4, 3, 0)]);
    assert_eq!(pgn.per_ply_annotations[1].highlights, vec![(4, 3, 1)]);
    let roundtrip = parse_pgn(&pgn_to_string(&pgn)).unwrap();
    assert_eq!(roundtrip, pgn);
    assert_eq!(pgn.snapshots().unwrap().len(), 4);
    assert!(parse_pgn("1. e5 *").is_err());
}

#[test]
fn custom_start_black_move_and_underpromotion_roundtrip() {
    let input = "[SetUp \"1\"]\n[FEN \"7k/8/8/8/8/8/p7/7K b - - 0 8\"]\n\n8... a1=N *";
    let pgn = parse_pgn(input).unwrap();
    let snapshots = pgn.snapshots().unwrap();
    assert_eq!(
        Game::from_fen(snapshots.last().unwrap()).unwrap().board[0],
        -KNIGHT_ID
    );
    assert!(pgn_to_string(&pgn).contains("8... a1=N"));
    assert_eq!(
        parse_pgn(&pgn_to_string(&pgn))
            .unwrap()
            .snapshots()
            .unwrap(),
        snapshots
    );
}
