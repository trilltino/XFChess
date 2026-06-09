# Nimzovich Engine — Technical Analysis & Nimzowitsch Analyzer Design

## Table of Contents

1. [Current Engine Overview](#1-current-engine-overview)
2. [Data Structures](#2-data-structures)
3. [Evaluation Function](#3-evaluation-function)
4. [Move Generation](#4-move-generation)
5. [Search Algorithm](#5-search-algorithm)
6. [Move Ordering](#6-move-ordering)
7. [On-Chain Path](#7-on-chain-path)
8. [Strengths & Gaps](#8-strengths--gaps)
9. [Nimzowitsch Analyzer Design](#9-nimzowitsch-analyzer-design)
10. [Integration with XFChess](#10-integration-with-xfchess)

---

## 1. Current Engine Overview

The `nimzovich_engine` crate is a classic **minimax AI** with modern alpha-beta enhancements, written in `no_std`-compatible Rust for dual use: the game client AI and Solana on-chain move validation.

| Property | Value |
|---|---|
| Search depth | 6 ply (iterative deepening 1→6) |
| Nodes/second | ~500K–1M |
| Estimated strength | ~1800–2000 ELO |
| Evaluation terms | Material + PST (tapered MG/EG) + Mobility |
| TT size | 2M entries (~80MB) on native, 64K on WASM |
| Special paths | `CompactBoard` (68 bytes, stack-only) for on-chain |

---

## 2. Data Structures

### Board Representation

The central `Game` struct uses a `board: [i8; 64]` with signed encoding:

```
Positive (1–6) = white  P/N/B/R/Q/K
Negative (−1–−6) = black P/N/B/R/Q/K
Zero = empty
```

This lets color checks collapse to `piece.signum()` — no separate color array needed.

Twelve piece-type bitboards sit alongside the mailbox for fast occupancy queries:

```
white_pawns, white_knights, white_bishops, white_rooks, white_queens, white_kings
black_pawns, black_knights, black_bishops, black_rooks, black_queens, black_kings
occupied_white = OR(all white)
occupied_black = OR(all black)
occupied        = occupied_white | occupied_black
```

Sliding attack masks (`[[BitSet; 8]; 64]`) are pre-baked — one 64-bit mask per direction per square — enabling O(1) blocker lookups via `trailing_zeros()`.

### The `KK` Move Struct

```rust
pub struct KK {
    pub score:       i16,   // ordering heuristic value
    pub src:         i8,    // source square (0–63)
    pub dst:         i8,    // destination square (0–63)
    pub nxt_dir_idx: u8,    // hi nibble = promotion piece, lo nibble = direction index
}
```

At 5 bytes it fits 200 moves in ≈1 KB — good cache behaviour during move ordering.

### Transposition Table

Two million `TTE` entries, each holding 5 collision-resolution slots:

```
TTE.h[5]          → 5 × Guide2
Guide2.key[24]    → 192-bit Zobrist hash
Guide2.res        → HashResult (up to 11 sub-entries per depth)
Guide2.pri        → depth × 10 + move_counter  (replacement policy)
```

The 24-byte hash is compact but raises birthday-paradox collision risk around position 65,000 in a long search tree — acceptable for practical play at depth 6.

### Search Heuristics Tables

```
killer_moves[MAX_DEPTH+1][2]    — 2 killers per ply (sibling-branch refutations)
history_table[64][64]           — quiet move bonus (updated on beta cutoff)
cap_history[7][64][7]           — capture bonus by (attacker_type, dst, victim_type)
conthist[64][64]                — continuation history (2-ply context)
```

---

## 3. Evaluation Function

### Material

Centipawn values used throughout:

```
Pawn   = 100    Knight = 300    Bishop = 300
Rook   = 500    Queen  = 900    King   = 18,000
```

### Piece-Square Tables (Tapered)

Separate MG and EG tables per piece type. Interpolation:

```
phase_value per piece: P=0, N=1, B=1, R=2, Q=4
phase = clamp(Σ piece_phase_values, 0, 24)

score = (MG_score × phase + EG_score × (24 − phase)) / 24
```

Key positional tendencies baked into the tables:

| Piece | Middlegame bias | Endgame bias |
|---|---|---|
| Pawn | Advancement (+50 at rank 7), edge penalty | — |
| Knight | Centralisation (+15–20 in d4/d5/e4/e5), rim penalty (−50) | — |
| Bishop | Long diagonal (+10–20), edge penalty | — |
| Rook | 7th/8th rank bonus (+5–10), back rank small penalty | — |
| Queen | Centre control (+5), edge penalty | — |
| King | Kingside castle preferred (+20–30), centre avoided (−20–50) | Centre heavily favoured (+30–40) |

### Mobility Bonus

```
mobility = white_legal_moves − black_legal_moves
bonus = (mobility × 3 × (24 − phase) + mobility × 2 × phase) / 24
```

Mobility matters more in the endgame (coefficient 3 vs 2 in the middlegame).

### What Is Missing

- Passed pawn evaluation (advancement bonus, rook behind passer)
- Isolated / doubled pawn penalties
- Backward pawn penalty
- Bishop pair bonus
- Outpost squares (true fixed-point evaluation)
- Blockade detection
- Rook open/semi-open file bonus
- King tropism in endgame
- Pawn shield / storm terms for king safety

---

## 4. Move Generation

### Two-Phase Architecture

**Phase 1 — Precomputation (init-time, ~1–2 ms)**

For all 64 squares, build lists of pseudo-legal destinations by piece type into `game.rook[64]`, `game.bishop[64]`, etc. Knights and kings are fully precomputed; sliding pieces store per-direction lists.

**Phase 2 — Runtime filtering**

Intersect precomputed lists with occupancy bitboards:
- Captures: destination in `occupied_opponent`
- Quiets: destination in `~occupied`
- Promotions: pawn reaches rank 1 or 8 → generate 4 variants (Q/R/B/N)
- En passant: pawn on correct rank, `en_passant_target` set
- Castling: path empty, king not currently in or passing through check

### Sliding Piece Optimisation

```
mask = sliding_attack_masks[sq][dir]   // pre-baked ray
blockers = mask & occupied
first_blocker = bit_scan(blockers)     // trailing_zeros / leading_zeros
targets = mask ^ (ray_beyond(first_blocker))
targets &= ~occupied_own
```

This replaces loop-per-square with a handful of bitwise operations.

### Attack Detection (`attack.rs`)

`is_square_attacked(game, sq, by_color)` — called for:
- Legal-move validation (is king in check after move?)
- Castling path clearance
- Quiescence check detection

---

## 5. Search Algorithm

### Iterative Deepening

```
for depth in 1..=MAX_DEPTH (6):
    if elapsed > 90% of time budget: abort
    score = alphabeta(depth, α, β)
    if score outside aspiration window:
        re-search with (−∞, +∞)
    if |score| > KING_VALUE / 2: announce mate
```

Aspiration windows use `base=63, mul=3` — widen on fail-low/fail-high.

### Alpha-Beta (Negamax PVS)

```
alphabeta(depth, α, β):
    probe TT → early return or best-move hint

    if depth ≤ 4 and eval − RFP_margin ≥ β:
        return eval  (Reverse Futility Pruning)

    if depth ≥ 2 and no_pawns and eval ≥ β:
        do null-move with r = (626×depth + 11)/320 + …  (NMP)

    for move in ordered_moves():
        if depth ≤ 2 and SEE(move) < threshold: skip
        if depth ≤ 4 and quiet and move_count > LMP_limit: skip

        new_depth = depth − 1
        if check_extension: new_depth += 1

        if move_count > 3:
            r = ln(depth)×ln(move_count)×0.613 + 1.225   (LMR)
            new_depth = max(1, new_depth − r)

        score = PVS(-β, -α, new_depth)

        if score ≥ β: store killer, update history, break  (β cutoff)

    store TT; return best
```

### Quiescence Search

Extends at depth=0 with captures (and checks if in check). Delta pruning skips captures that cannot raise alpha:

```
if stand_pat + captured_value + 300 < α: skip
```

Capped at 4 additional plies to avoid runaway capture chains.

### Tuned Parameters (SPSA)

All margins in the pruning conditions were tuned by SPSA rather than hand-crafted:

| Technique | Key values |
|---|---|
| RFP | depth ≤ 4, margin = 4 + 37×depth + 70×improving |
| LMR | log formula, base 1.225, mul 0.613 |
| NMP | r = (626×depth + 11)/320 + (eval−β)/200 |
| Futility | depth ≤ 2, margin = 209 + 209×depth |
| SEE quiet | −76 threshold at depth ≤ 2 |

---

## 6. Move Ordering

Staged generation (each stage is lazy — only generated if needed):

```
Stage 1   TT best move             (from previous iteration / probe)
Stage 2   Good captures            (MVV-LVA score > 5000: 10000 + victim×10 − attacker)
Stage 3   Killer moves             (cutoff moves from sibling nodes at same depth)
Stage 4   Quiet moves              (history heuristic + centre bonus)
Stage 5   Bad captures             (score < 5000, rarely reached due to pruning)
```

History is updated on every beta cutoff:

```
bonus = depth²
history[src][dst] += bonus   (move that cut)
history[src][dst] -= bonus   (moves searched before the cut, that didn't cut)
```

---

## 7. On-Chain Path

For Solana program validation a `no_std`, allocation-free variant exists:

**`CompactBoard` (68 bytes, stack-only)**

```rust
pub struct CompactBoard {
    squares:     [i8; 64],
    castling:    u8,
    ep_target:   i8,
    side_to_move: i8,
    _pad:        u8,
}
```

`bytemuck`-compatible, zero-copy serialisable. Built from a FEN string via `from_fen()`.

**`OnChainGame`** adds the 12-bitboard layout on top without heap allocation.

**`on_chain_attack.rs`** provides `is_in_check_fast()`, `bishop_attacks()`, `rook_attacks()`, `queen_attacks()` — pure bitboard arithmetic, no loops, no allocations.

---

## 8. Strengths & Gaps

### Strengths

- Modern search: PVS + LMR + RFP + NMP + SEE pruning + killers + history
- Clean modular layout (16+ focused modules)
- Dual path: full AI for the client, `no_std` 68-byte path for on-chain validation
- Tapered evaluation smoothly transitions MG → EG
- SPSA-tuned parameters rather than guessed margins

### Evaluation Gaps (relevant to the Nimzowitsch Analyzer)

| Missing term | Nimzowitsch concept impacted |
|---|---|
| Passed pawn detection | "The passed pawn is a criminal" — must be restrained or exploited |
| Blockade detection | Blockading piece on stop square of passed pawn |
| Isolated / backward pawn penalty | Weak squares doctrine |
| Outpost evaluation | Knight on fixed strong square protected by pawn |
| Open / semi-open rook file bonus | Rook activity on open files |
| Pawn chain evaluation | Phalanx vs. chain weakness |
| Piece immobility score | "Immobilisation of the enemy pieces" |
| Overprotection bonus | Nimzowitsch's principle of overprotecting key squares |
| Bishop pair bonus | Standard positional bonus not yet present |
| King tropism | Endgame-specific king-to-passer or king-to-weak-pawn proximity |

---

## 9. Nimzowitsch Analyzer Design

This section explores a **post-game analysis layer** tuned to principles from *My System* (1925). It is not a replacement for the AI search — it is a **position scanner** that annotates moves and positions after the game ends.

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                NimzoAnalyzer                        │
│                                                     │
│  input:  Vec<(FEN, Move)>  — full game PGN          │
│  output: Vec<AnnotatedMove>                         │
│                                                     │
│  Phases:                                            │
│    1. position_scan()   — per-position features     │
│    2. move_audit()      — compare played vs. best   │
│    3. theme_detector()  — classify findings by      │
│                           Nimzowitsch principle     │
│    4. report_builder()  — structured report         │
└─────────────────────────────────────────────────────┘
```

### Core Feature Detectors

#### 1. Passed Pawn Detection

A pawn on file `f`, rank `r` is passed if no enemy pawn exists on files `f−1`, `f`, `f+1` for any rank ahead.

```rust
fn passed_pawns(board: &CompactBoard, color: i8) -> BitSet {
    let own_pawns   = pawns_for(board, color);
    let enemy_pawns = pawns_for(board, -color);
    let mut result  = BitSet::empty();

    for sq in own_pawns.iter() {
        let front_span = fill_forward_adjacent_files(sq, color);
        if front_span & enemy_pawns == BitSet::empty() {
            result.insert(sq);
        }
    }
    result
}
```

**Analysis rule**: If a player has a passed pawn and does not advance it to the 6th/7th rank or place a rook behind it within N moves, flag as "missed passed pawn activation".

#### 2. Blockade Detection

A passed pawn is properly blockaded when a **piece** (ideally a knight) occupies its stop square (one square in front of the passer).

```rust
fn is_blockaded(board: &CompactBoard, passer_sq: u8, color: i8) -> Option<i8> {
    let stop_sq = advance_one(passer_sq, color);
    let blocker = board.squares[stop_sq as usize];
    if blocker != 0 && blocker.signum() != color { Some(blocker) } else { None }
}
```

**Analysis rules**:
- If a player's passed pawn is blockaded by an **opponent knight**, annotate: "Textbook Nimzowitsch blockade — knight immobilises passer".
- If a player has a passed pawn with a clear stop square and does not occupy it, flag: "Missed blockade opportunity".
- If a blockading piece is exchanged off, flag: "Blockade lifted — passer becomes dangerous".

#### 3. Outpost Detection

An outpost square for color `c` on square `sq` requires:
- `sq` is in enemy half (`rank ≥ 5` for white, `rank ≤ 4` for black)
- A friendly pawn on file `f±1` behind `sq` guards it
- No enemy pawn can attack `sq` (no enemy pawn on files `f±1` ahead)

```rust
fn outpost_squares(board: &CompactBoard, color: i8) -> BitSet {
    let mut result = BitSet::empty();
    for sq in 0u8..64 {
        if in_own_half(sq, color) { continue; }
        if pawn_guards(board, sq, color) && !pawn_can_challenge(board, sq, -color) {
            result.insert(sq);
        }
    }
    result
}
```

**Analysis rules**:
- If an outpost exists and no friendly knight occupies it, flag: "Missed knight outpost on `sq`".
- If a knight reaches an outpost and the engine score does not improve, note: "Outpost established but not exploited".
- If the opponent trades off the outpost knight unnecessarily, flag: "Outpost conceded".

#### 4. Piece Immobility

A piece is immobile if it has zero or one legal move from its current square (excluding captures that lose material).

```rust
fn immobile_pieces(board: &CompactBoard, color: i8) -> Vec<(u8, i8)> {
    let game = OnChainGame::from_compact(board);
    generate_moves(&game, color)
        .group_by(|m| m.src)
        .filter(|(_, moves)| moves.len() <= 1)
        .map(|(sq, _)| (sq, board.squares[sq as usize]))
        .collect()
}
```

**Analysis rules**:
- If an opponent piece has been immobile for ≥3 consecutive positions, flag: "Piece immobilisation — Nimzowitsch principle achieved".
- If your own piece is immobile for ≥2 positions, flag: "Own piece locked — consider activating or trading".

#### 5. Overprotection Audit

A square is overprotected if the count of defenders exceeds the count of attackers by ≥2.

```rust
fn overprotected_squares(board: &CompactBoard, color: i8) -> Vec<(u8, u8, u8)> {
    // returns (square, defender_count, attacker_count)
    key_squares(board).filter_map(|sq| {
        let d = count_defenders(board, sq, color);
        let a = count_attackers(board, sq, -color);
        if d >= a + 2 { Some((sq, d, a)) } else { None }
    }).collect()
}
```

**Analysis rules**:
- If a central pawn is underprotected and the engine's top move would have overprotected it, flag: "Overprotection missed — Nimzowitsch recommends accumulating defenders before the crisis".

#### 6. Pawn Chain & Backward Pawn Analysis

```rust
fn backward_pawns(board: &CompactBoard, color: i8) -> BitSet {
    // A pawn is backward if it cannot advance (blocked or would be captured)
    // and no friendly pawn is behind it on the same or adjacent file.
    ...
}
```

**Analysis rules**:
- Backward pawn on a semi-open file flagged as "chronic weakness".
- Isolated pawn flanked by open files flagged as "isolated island — restrict rook activity".

### AnnotatedMove Output

```rust
pub struct AnnotatedMove {
    pub ply:            u32,
    pub played_move:    Move,
    pub engine_best:    Option<Move>,
    pub centipawn_loss: i16,
    pub themes:         Vec<NimzoTheme>,
    pub note:           String,
}

pub enum NimzoTheme {
    PassedPawnMissed { sq: u8, side: Color },
    BlockadeOpportunity { passer_sq: u8, stop_sq: u8 },
    BlockadeAchieved { passer_sq: u8, blocker_piece: i8 },
    BlockadeLifted { passer_sq: u8 },
    OutpostMissed { sq: u8 },
    OutpostAchieved { sq: u8, piece: i8 },
    PieceImmobilised { sq: u8, piece: i8, plies: u32 },
    OverprotectionMissed { sq: u8 },
    BackwardPawnWeakness { sq: u8 },
    IsolatedPawn { sq: u8 },
    GoodBlockader { sq: u8 },   // Knight on stop square = textbook
    BadBlockader { sq: u8 },    // Bishop/Queen = bad blockader per Nimzowitsch
}
```

### Analysis Pipeline

```rust
pub fn analyse_game(positions: &[(CompactBoard, Move)]) -> GameReport {
    let mut annotations = Vec::with_capacity(positions.len());

    for (i, (board, played)) in positions.iter().enumerate() {
        let engine_move = find_best_move_shallow(board, ANALYSIS_DEPTH);

        let features  = PositionFeatures::compute(board);
        let prev_feat = if i > 0 { Some(PositionFeatures::compute(&positions[i-1].0)) } else { None };

        let themes = detect_themes(&features, prev_feat.as_ref(), played, &engine_move);
        let cpl    = engine_move.score - eval_after_move(board, played);

        annotations.push(AnnotatedMove {
            ply: i as u32,
            played_move: *played,
            engine_best: Some(engine_move),
            centipawn_loss: cpl as i16,
            themes,
            note: themes_to_note(&themes),
        });
    }

    GameReport { annotations, summary: build_summary(&annotations) }
}
```

`ANALYSIS_DEPTH` for the post-game pass can be higher than the live search (10–12 ply) since there is no time pressure.

### Blockader Quality Scoring

Nimzowitsch was explicit: a **knight** is the ideal blockader (it leaps, so it is not blocked by the pawn it restrains), a **bishop** is poor (diagonal mobility wasted), a **queen** is terrible (too valuable to tie down).

```rust
fn blockader_score(piece: i8) -> i32 {
    match piece.abs() {
        KNIGHT => 100,   // ideal
        KING   => 80,    // acceptable in endgame
        ROOK   => 60,    // workable
        BISHOP => 20,    // bad — diagonal lines wasted
        QUEEN  => -20,   // Nimzowitsch explicitly condemns this
        _      => 0,
    }
}
```

---

## 10. Integration with XFChess

### Proposed Data Flow

```
Game ends (Solana finalize_game)
         │
         ▼
Backend /api/analyse/:game_id
  1. Fetch full move history from DB
  2. Replay positions via CompactBoard::from_fen + make_move
  3. Run NimzoAnalyzer::analyse_game (blocking task / Tokio spawn_blocking)
  4. Store AnnotatedGame in analysis table
  5. Return JSON report
         │
         ▼
Web frontend GameReview component
  - Timeline of moves, colour-coded by theme
  - Click move → show board + annotation bubble
  - Summary panel: "You missed 3 outpost opportunities, had 2 immobilised pieces"
```

### Backend Changes

- New table `game_analysis` (`game_id TEXT, ply INT, themes JSON, cpl INT`)
- New route `GET /api/games/:id/analysis` → `GameReport` JSON
- New background task in `src/tasks/` triggered on game archive
- Expose `nimzovich_engine::analyser` as a crate feature so the binary is not pulled into the Solana program build

### Crate Feature Flag

```toml
# crates/nimzovich_engine/Cargo.toml
[features]
default  = []
analyser = []          # pulls in the NimzoAnalyzer module
on-chain = []          # CompactBoard path only, minimal binary size
```

### Frontend Display

Suggested annotation markers (matching chess.com / Lichess conventions):

| Symbol | Meaning |
|---|---|
| `??` | Blunder (CPL > 200) |
| `?` | Mistake (CPL 80–200) |
| `?!` | Inaccuracy (CPL 20–80) |
| `⛨` | Outpost achieved |
| `⚓` | Blockade established |
| `⚠` | Passed pawn ignored |
| `🔒` | Piece immobilised |
| `★` | Overprotection correct |

---

## Implementation Roadmap

| Phase | Work | Effort |
|---|---|---|
| **P1** | `passed_pawns()`, `blockade_detection()` detectors in `on_chain_moves.rs` | 1–2 days |
| **P2** | `outpost_squares()`, `immobile_pieces()`, `backward_pawns()` | 1–2 days |
| **P3** | `NimzoTheme` enum, `AnnotatedMove`, `analyse_game()` pipeline (feature-gated) | 2–3 days |
| **P4** | Backend route + DB table + background task | 1 day |
| **P5** | Web frontend `GameReview` component | 2–3 days |
| **P6** | Tune `ANALYSIS_DEPTH` and timing; stress-test on 1,000 archived games | 1 day |

**Total estimated effort: 8–12 days** to a usable v1 analysis layer.

---

## Key Insight

The engine is already named after Nimzowitsch but currently implements only the most basic positional scoring (PST + mobility). The real *My System* concepts — blockade, outpost, immobilisation, overprotection — are entirely absent from the evaluation function.

Adding them to the **analyser** (post-game, no time pressure) is far simpler than adding them to the **live search** because:
- No pruning interactions to worry about
- Can scan arbitrary ply-distance patterns (e.g. "pawn has been blockaded for 8 moves")
- Can afford heavier computation per position
- Failures are educational annotations, not blunders

The `CompactBoard` / `OnChainGame` infrastructure already exists and is the right foundation — all new detectors build on top of it without touching the search stack.
