//! Staged move picker for alpha-beta search
//!
//! Yields moves in order of expected quality without sorting the entire list upfront:
//!   1. TT move (validated against the generated move list)
//!   2. Captures (sorted by MVV-LVA)
//!   3. Killer moves (validated against the generated move list, non-captures)
//!   4. Quiet moves (sorted by history heuristics)
//!
//! Every yielded move comes from the position's own generated move list.
//! TT moves and killers are only used to *reorder* that list — they are never
//! trusted directly, because TT entries can describe transpositions with
//! different legality and killer slots are shared across plies/colors
//! (killer_moves is indexed by depth, so a black reply stored by a deeper
//! node can sit in the slot the root reads). `make_move` executes blindly,
//! so yielding an unvalidated move corrupts the search with illegal moves.

use crate::board::pos_to_square;
use crate::constants::*;
use crate::types::*;

/// Stages of move generation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    TtMove,
    GoodCaptures,
    Killers,
    Quiets,
    BadCaptures,
    Done,
}

/// Staged move picker — yields one move at a time.
/// All yielded moves are members of the move list passed to [`build_picker`].
pub struct MovePicker {
    stage: Stage,
    /// TT move, only if present in the move list.
    tt_move: Option<KK>,
    /// SEE-nonnegative captures from the move list, MVV-LVA sorted.
    captures: Vec<KK>,
    /// Killer moves found in the move list (at most 2, non-captures).
    killers: Vec<KK>,
    /// Remaining quiet moves, history-sorted.
    quiets: Vec<KK>,
    /// SEE-losing captures, searched last.
    bad_captures: Vec<KK>,
    current: usize,
}

impl MovePicker {
    /// Yield the next best move, or `None` when exhausted.
    pub fn next_move(&mut self) -> Option<KK> {
        loop {
            match self.stage {
                Stage::TtMove => {
                    self.stage = Stage::GoodCaptures;
                    if let Some(tt) = self.tt_move {
                        return Some(tt);
                    }
                }
                Stage::GoodCaptures => {
                    if self.current < self.captures.len() {
                        let mv = self.captures[self.current];
                        self.current += 1;
                        return Some(mv);
                    }
                    self.current = 0;
                    self.stage = Stage::Killers;
                }
                Stage::Killers => {
                    if self.current < self.killers.len() {
                        let mv = self.killers[self.current];
                        self.current += 1;
                        return Some(mv);
                    }
                    self.current = 0;
                    self.stage = Stage::Quiets;
                }
                Stage::Quiets => {
                    if self.current < self.quiets.len() {
                        let mv = self.quiets[self.current];
                        self.current += 1;
                        return Some(mv);
                    }
                    self.current = 0;
                    self.stage = Stage::BadCaptures;
                }
                Stage::BadCaptures => {
                    if self.current < self.bad_captures.len() {
                        let mv = self.bad_captures[self.current];
                        self.current += 1;
                        return Some(mv);
                    }
                    self.stage = Stage::Done;
                }
                Stage::Done => return None,
            }
        }
    }
}

/// Score all moves and build a staged picker.
///
/// Capture/quiet classification is done from the board (the only reliable
/// source) — never from move scores, which mix history and positional bonuses.
pub fn build_picker(
    game: &Game,
    moves: Vec<KK>,
    depth: i32,
    tt_move: Option<KK>,
) -> MovePicker {
    let d_idx = depth.max(0) as usize;
    let killer_slots = if d_idx <= MAX_DEPTH {
        game.killer_moves[d_idx]
    } else {
        [None; 2]
    };
    let tt_key = tt_move.map(|m| (m.src, m.dst));

    let mut validated_tt: Option<KK> = None;
    let mut captures: Vec<KK> = Vec::new();
    let mut bad_captures: Vec<KK> = Vec::new();
    let mut killers: Vec<KK> = Vec::new();
    let mut quiets: Vec<KK> = Vec::new();

    for mut mv in moves {
        let is_capture = game.board[mv.dst as usize] != 0
            || (game.board[mv.src as usize].abs() == PAWN_ID
                && game.en_passant_target == Some(mv.dst));

        // TT move: validated by membership in the generated list, yielded first.
        if tt_key == Some((mv.src, mv.dst)) {
            validated_tt = Some(mv);
            continue;
        }

        if is_capture {
            // MVV-LVA: most valuable victim, least valuable attacker.
            let attacker = FIGURE_VALUE[game.board[mv.src as usize].abs() as usize] as i32;
            let victim_sq = game.board[mv.dst as usize];
            let victim = if victim_sq != 0 {
                FIGURE_VALUE[victim_sq.abs() as usize] as i32
            } else {
                FIGURE_VALUE[PAWN_ID as usize] as i32 // en passant
            };
            let score = victim * 10 - attacker;
            mv.score = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            // SEE-losing captures go to the back of the queue.
            if crate::see::see(game, mv, 0) {
                captures.push(mv);
            } else {
                bad_captures.push(mv);
            }
            continue;
        }

        // Killer: validated by membership in the generated list, yielded after
        // captures. Not also kept in quiets (no duplicate search).
        if killer_slots
            .iter()
            .flatten()
            .any(|k| k.src == mv.src && k.dst == mv.dst)
        {
            killers.push(mv);
            continue;
        }

        // Quiet: history + small centralization tiebreak.
        let history = game.history_table[mv.src as usize][mv.dst as usize] as i32 / 128;
        let (col, row) = pos_to_square(mv.dst);
        let center_dist = ((col - 3).abs() + (row - 3).abs()) as i32;
        let score = history + (8 - center_dist) * 2;
        mv.score = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        quiets.push(mv);
    }

    captures.sort_by(|a, b| b.score.cmp(&a.score));
    quiets.sort_by(|a, b| b.score.cmp(&a.score));
    bad_captures.sort_by(|a, b| b.score.cmp(&a.score));

    MovePicker {
        stage: Stage::TtMove,
        tt_move: validated_tt,
        captures,
        killers,
        quiets,
        bad_captures,
        current: 0,
    }
}
