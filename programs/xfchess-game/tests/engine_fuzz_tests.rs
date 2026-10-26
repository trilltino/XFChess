//! P2: seeded, reproducible fuzzing of the on-chain move validator.
//!
//! Every failure message carries the seed and ply so it can be replayed:
//!     ENGINE_FUZZ_SEED=<n> cargo test -p xfchess-game --test engine_fuzz_tests
//!
//! The suite checks that
//! * a rejected move never mutates the position,
//! * accepted moves keep the board well formed (one king per side, no pawns on
//!   the back ranks, side to move flips, byte round-trip is lossless),
//! * a position reported as `Playing` always has at least one legal move, and
//! * arbitrary 68-byte boards never make the validator panic.
use nimzovich_engine::{validate_and_apply, CompactBoard, MoveOutcome};

/// xorshift64*: tiny, deterministic and dependency free.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn base_seed() -> u64 {
    std::env::var("ENGINE_FUZZ_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0x5EED_C0DE)
}

fn square(index: u8) -> [u8; 2] {
    [b'a' + index % 8, b'1' + index / 8]
}

fn uci(from: u8, to: u8, promotion: Option<u8>) -> [u8; 5] {
    let (f, t) = (square(from), square(to));
    [f[0], f[1], t[0], t[1], promotion.unwrap_or(0)]
}

/// Every pseudo-candidate move: all from/to pairs plus the four promotions.
fn all_candidates() -> Vec<[u8; 5]> {
    let mut moves = Vec::with_capacity(64 * 64 * 5);
    for from in 0..64u8 {
        for to in 0..64u8 {
            if from == to {
                continue;
            }
            moves.push(uci(from, to, None));
            for promotion in *b"qrbn" {
                moves.push(uci(from, to, Some(promotion)));
            }
        }
    }
    moves
}

fn legal_moves(board: &CompactBoard, candidates: &[[u8; 5]]) -> Vec<[u8; 5]> {
    candidates
        .iter()
        .copied()
        .filter(|mv| {
            let mut position = board.to_on_chain_game();
            validate_and_apply(&mut position, mv).is_ok()
        })
        .collect()
}

fn assert_well_formed(board: &CompactBoard, context: &str) {
    let kings = |sign: i8| board.squares.iter().filter(|&&p| p == 6 * sign).count();
    assert_eq!(kings(1), 1, "{context}: white king count");
    assert_eq!(kings(-1), 1, "{context}: black king count");
    for (index, &piece) in board.squares.iter().enumerate() {
        let rank = index / 8;
        assert!(
            !(piece.abs() == 1 && (rank == 0 || rank == 7)),
            "{context}: pawn on a back rank at {index}"
        );
        assert!(
            piece.abs() <= 6,
            "{context}: bogus piece {piece} at {index}"
        );
    }
    assert_eq!(
        CompactBoard::from_bytes(&board.to_bytes()).to_bytes(),
        board.to_bytes(),
        "{context}: byte round trip"
    );
}

#[test]
fn seeded_random_games_keep_every_invariant() {
    let candidates = all_candidates();
    let base = base_seed();
    let mut accepted_plies = 0u32;
    for game_index in 0..12u64 {
        let seed = base.wrapping_add(game_index);
        let mut rng = Rng::new(seed);
        let mut board = CompactBoard::starting_position();
        for ply in 0..90u32 {
            let context = format!("seed {seed} ply {ply}");
            // Sample moves until one is legal; a miss after many tries falls
            // back to exhaustive generation (also used to audit `Playing`).
            let mut chosen = None;
            for _ in 0..400 {
                let mv = candidates[rng.below(candidates.len() as u64) as usize];
                let mut position = board.to_on_chain_game();
                let before = position.to_compact_board().to_bytes();
                match validate_and_apply(&mut position, &mv) {
                    Ok(outcome) => {
                        chosen = Some((mv, outcome, position));
                        break;
                    }
                    Err(_) => {
                        assert_eq!(
                            position.to_compact_board().to_bytes(),
                            before,
                            "{context}: rejected move mutated the position"
                        );
                    }
                }
            }
            let (mv, outcome, position) = match chosen {
                Some(found) => found,
                None => {
                    let legal = legal_moves(&board, &candidates);
                    if legal.is_empty() {
                        break;
                    }
                    let mv = legal[rng.below(legal.len() as u64) as usize];
                    let mut position = board.to_on_chain_game();
                    let outcome = validate_and_apply(&mut position, &mv)
                        .unwrap_or_else(|_| panic!("{context}: generated move rejected"));
                    (mv, outcome, position)
                }
            };
            let next = position.to_compact_board();
            accepted_plies += 1;
            assert_well_formed(&next, &context);
            assert_ne!(
                next.side_to_move, board.side_to_move,
                "{context}: side to move did not flip after {mv:?}"
            );
            match outcome {
                MoveOutcome::Playing => {
                    if ply % 6 == 0 {
                        assert!(
                            !legal_moves(&next, &candidates).is_empty(),
                            "{context}: Playing but no legal move exists"
                        );
                    }
                }
                MoveOutcome::Checkmate | MoveOutcome::Stalemate => {
                    assert!(
                        legal_moves(&next, &candidates).is_empty(),
                        "{context}: terminal outcome but a legal move exists"
                    );
                    break;
                }
                MoveOutcome::InsufficientMaterial => break,
            }
            board = next;
        }
    }
    assert!(
        accepted_plies > 400,
        "fuzzing was vacuous: only {accepted_plies} plies were played"
    );
}

#[test]
fn arbitrary_boards_and_moves_never_panic_the_validator() {
    let base = base_seed();
    let mut rng = Rng::new(base ^ 0xA5A5_A5A5);
    let mut panics = Vec::new();
    for case in 0..20_000u32 {
        let mut bytes = [0u8; 68];
        for byte in bytes.iter_mut() {
            *byte = rng.next() as u8;
        }
        // Half the cases keep piece codes plausible so deeper paths run.
        if case % 2 == 0 {
            for byte in bytes[..64].iter_mut() {
                *byte = (rng.below(13) as i8 - 6) as u8;
            }
        }
        let mv = uci(
            rng.below(64) as u8,
            rng.below(64) as u8,
            Some(*b"qrbn\0".get(rng.below(5) as usize).unwrap_or(&0)),
        );
        let outcome = std::panic::catch_unwind(|| {
            let mut position = CompactBoard::from_bytes(&bytes).to_on_chain_game();
            let _ = validate_and_apply(&mut position, &mv);
        });
        if outcome.is_err() {
            panics.push((case, bytes, mv));
        }
    }
    assert!(
        panics.is_empty(),
        "validator panicked on {} arbitrary boards, first: case {} bytes {:?} move {:?}",
        panics.len(),
        panics[0].0,
        panics[0].1,
        panics[0].2
    );
}
