//! Staged move picker for alpha-beta search
//!
//! Yields moves in order of expected quality without sorting the entire list upfront:
//!   1. TT move (if provided)
//!   2. Good captures (SEE-positive, sorted by MVV-LVA)
//!   3. Killer moves (not captures)
//!   4. Quiet moves (sorted by history heuristics)
//!   5. Bad captures (SEE-negative)

use crate::board::pos_to_square;
use crate::constants::*;
use crate::types::*;

/// Stages of move generation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    TtMove,
    InitCaptures,
    GoodCaptures,
    Killers,
    Quiets,
    BadCaptures,
    Done,
}

/// Staged move picker — yields one move at a time.
pub struct MovePicker {
    stage: Stage,
    tt_move: Option<KK>,
    moves: Vec<KK>,
    good_captures: Vec<KK>,
    bad_captures: Vec<KK>,
    killers: [Option<KK>; 2],
    quiets: Vec<KK>,
    current: usize,
}

impl MovePicker {
    pub fn new(moves: Vec<KK>, tt_move: Option<KK>, killers: [Option<KK>; 2]) -> Self {
        let mut picker = Self {
            stage: Stage::TtMove,
            tt_move,
            moves,
            good_captures: Vec::new(),
            bad_captures: Vec::new(),
            killers,
            quiets: Vec::new(),
            current: 0,
        };
        picker.classify_moves();
        picker
    }

    /// Classify all moves into categories and sort each.
    fn classify_moves(&mut self) {
        let tt_src_dst = self.tt_move.map(|m| (m.src, m.dst));

        for mv in self.moves.drain(..) {
            // Skip TT move (already yielded first)
            if tt_src_dst == Some((mv.src, mv.dst)) {
                continue;
            }

            // Skip killers (yielded separately)
            if self.killers.iter().any(|k| {
                k.map_or(false, |k| k.src == mv.src && k.dst == mv.dst)
            }) {
                self.quiets.push(mv);
                continue;
            }

            let is_capture = mv.score != 0; // score is set by move generator for captures
            if is_capture {
                // Use SEE to classify as good or bad capture
                // For simplicity, use the score heuristic; full SEE requires game ref
                // which we don't have here. The caller should pre-score captures.
                if mv.score > 5000 {
                    self.good_captures.push(mv);
                } else {
                    self.bad_captures.push(mv);
                }
            } else {
                self.quiets.push(mv);
            }
        }

        // Sort good captures by MVV-LVA (score already set by order_moves)
        self.good_captures.sort_by(|a, b| b.score.cmp(&a.score));

        // Sort quiets by history score
        self.quiets.sort_by(|a, b| b.score.cmp(&a.score));

        // Sort bad captures (least bad first, but usually skipped in pruning)
        self.bad_captures.sort_by(|a, b| b.score.cmp(&a.score));
    }

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
                    if self.current < self.good_captures.len() {
                        let mv = self.good_captures[self.current];
                        self.current += 1;
                        return Some(mv);
                    }
                    self.current = 0;
                    self.stage = Stage::Killers;
                }
                Stage::Killers => {
                    self.stage = Stage::Quiets;
                    for k in self.killers.iter().flatten() {
                        return Some(*k);
                    }
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
                _ => self.stage = Stage::Done,
            }
        }
    }

    /// Number of moves in all stages (excluding TT move if already returned).
    pub fn len(&self) -> usize {
        self.good_captures.len()
            + self.killers.iter().flatten().count()
            + self.quiets.len()
            + self.bad_captures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Score all moves and build a staged picker.
///
/// This is the main entry point replacing the old `order_moves()` + iterate pattern.
pub fn build_picker(
    game: &Game,
    moves: Vec<KK>,
    depth: i32,
    tt_move: Option<KK>,
) -> MovePicker {
    let d_idx = depth.max(0) as usize;
    let killers = if d_idx <= MAX_DEPTH {
        game.killer_moves[d_idx]
    } else {
        [None; 2]
    };

    // Pre-score all moves (same logic as old order_moves)
    let mut scored_moves = moves;
    for mv in scored_moves.iter_mut() {
        let mut score = 0i32;

        // 1. MVV-LVA for captures
        let captured = game.board[mv.dst as usize];
        if captured != 0 {
            let attacker_value = FIGURE_VALUE[game.board[mv.src as usize].abs() as usize] as i32;
            let victim_value = FIGURE_VALUE[captured.abs() as usize] as i32;
            score += 10000 + (victim_value * 10 - attacker_value);
        }

        // 2. Killer bonus
        for killer in killers.iter().flatten() {
            if killer.src == mv.src && killer.dst == mv.dst {
                score += 5000;
                break;
            }
        }

        // 3. History heuristic
        let history = game.history_table[mv.src as usize][mv.dst as usize] as i32;
        score += history / 128;

        // 4. Center control
        let (col, row) = pos_to_square(mv.dst);
        let center_dist = ((col - 3).abs() + (row - 3).abs()) as i32;
        score += (8 - center_dist) * 2;

        mv.score = score.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }

    MovePicker::new(scored_moves, tt_move, killers)
}
