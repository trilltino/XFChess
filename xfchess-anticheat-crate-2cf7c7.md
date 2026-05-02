# XFChess Anti-Cheat Crate — Phase 1 Plan (v2)

Build a new `crates/xfchess-anticheat` Rust crate that analyses **PvP and tournament games only**, runs post-game Stockfish checks co-located on the Hetzner signing-server VPS, and writes a TXT + JSON suspicion report — after first adding a signed move-log persistence layer in the signing server so the crate has a well-defined data source.

## Scope

**Runs for:** PvP matches and tournament games (not casual / practice / AI games).

**Phase 1 includes:**
- Signed move-log persistence in the signing server (prerequisite; see Step 0)
- Timing analysis from server-side signing timestamps
- Post-game Stockfish T1/T2/T3 + centipawn-loss (CPL)
- Opening-book filter (Polyglot) — used as a filter only, not as a signal
- Position-complexity weighting (skip forced moves, config-driven threshold)
- `expected_cpl_baseline_vs_elo` accuracy signal (IPR **descoped to Phase 2**)
- Weighted score → `Verdict { Clean, Review, Flag }`
- TXT report + JSON sidecar (schema defined in `types.rs` day 1)

**Explicitly deferred to Phase 2+:**
- Regan IPR (needs a cross-game probabilistic skill model; non-trivial)
- `opening_deviation` signal (weight 0, book used only as filter)
- Collusion / pairwise detection, IP/proxy checks, rolling multi-game z-score
- ML (Irwin/CNN-LSTM), engine fingerprinting, Tauri process telemetry
- On-chain Merkle anchoring of reports

## Step 0 — Lock the data source (do before scaffolding the crate)

**Findings from `@c:\Users\isich\XFChess\backend\src\signing\routes\main.rs:50-55` and `@c:\Users\isich\XFChess\backend\src\signing\routes\main.rs:177-220`:**

- `RecordMoveReq { game_id: u64, move_uci: String, next_fen: String, nonce: u64 }` is the authoritative shape.
- Moves are signed and submitted to the ER; **no local persistence** exists today.
- No server-captured timestamp is stored — currently only `SystemTime::now()` inside signature generation.

**Action:** add a minimal `moves` table + persistence hook inside `record_move`. This is the anti-cheat crate's data source.

```sql
-- backend/migrations/002_moves.sql
CREATE TABLE moves (
  game_id        INTEGER NOT NULL,
  ply            INTEGER NOT NULL,            -- 0-indexed
  mover          TEXT    NOT NULL,            -- wallet pubkey (base58)
  move_uci       TEXT    NOT NULL,
  next_fen       TEXT    NOT NULL,
  nonce          INTEGER NOT NULL,
  signed_at_ms   INTEGER NOT NULL,            -- monotonic-ish server wall clock
  prev_signed_ms INTEGER,                     -- for latency features
  session_sig    BLOB    NOT NULL,            -- signature bytes
  PRIMARY KEY (game_id, ply)
);
CREATE INDEX idx_moves_game_id ON moves(game_id);
```

Written synchronously inside `record_move` before/after ER submission (single small insert, negligible latency). This is the only backend change outside the anti-cheat hook.

## Locked `GameRecord` shape

```rust
// crates/xfchess-anticheat/src/types.rs
pub struct GameRecord {
    pub game_id: u64,
    pub context: GameContext,              // Pvp { wager, mode } | Tournament { tournament_id, round }
    pub white: PlayerRef,                  // pubkey + on-chain ELO snapshot
    pub black: PlayerRef,
    pub time_control: TimeControl,         // base_sec, inc_sec
    pub start_fen: String,                 // standard unless Chess960
    pub moves: Vec<MoveRecord>,            // ordered by ply
    pub result: GameResult,                // WhiteWin | BlackWin | Draw(Reason)
    pub ended_at_ms: u64,
}

pub struct MoveRecord {
    pub ply: u32,
    pub move_uci: String,
    pub next_fen: String,
    pub signed_at_ms: u64,
    pub latency_ms: u32,                   // signed_at_ms - prev_signed_ms
    pub session_sig: Vec<u8>,
}
```

`ingest.rs` is a pure function `build_game_record(rows: &[MoveRow], meta: GameMeta) -> GameRecord`.

## Crate layout (unchanged from v1)

```
crates/xfchess-anticheat/
  Cargo.toml
  src/
    lib.rs            // public API
    types.rs          // GameRecord + JSON report schema (serde)
    ingest.rs         // pure: DB rows -> GameRecord
    config.rs         // thresholds, weights, worker count, paths
    error.rs
    features/
      mod.rs
      timing.rs       // pure
      accuracy.rs     // pure (CPL + expected_cpl_baseline_vs_elo)
      complexity.rs   // pure (configurable delta, per-phase)
      opening.rs      // pure (Polyglot filter only)
    engine/
      mod.rs
      stockfish.rs    // UCI subprocess wrapper
      job_queue.rs    // bounded tokio mpsc, N workers
    scoring/
      mod.rs          // weighted sum -> Verdict
      weights.rs      // config-driven, loaded from TOML at startup
    report/
      mod.rs
      txt.rs          // human-readable report
      json.rs         // machine-readable sidecar (stable schema)
      store.rs        // writes both to disk + DB row
  tests/
    fixtures/         // labelled PGNs: clean human, synthetic engine-vs-human, booked-opening, endgame-forced
    timing_tests.rs
    accuracy_tests.rs
    complexity_tests.rs
    opening_tests.rs
    scoring_tests.rs
    golden_report.rs
```

## Public API

```rust
pub async fn analyse_game(game: GameRecord, cfg: &Config) -> Verdict;
pub async fn enqueue_game(game: GameRecord) -> JobId;
pub fn verdict_to_txt(v: &Verdict, g: &GameRecord) -> String;
pub fn verdict_to_json(v: &Verdict, g: &GameRecord) -> serde_json::Value;
```

## Signal model (Phase 1)

| Signal | Weight | Notes |
|---|---|---|
| `timing_anomaly` | 0.40 | sub-threshold latency on non-trivial positions |
| `expected_cpl_baseline_vs_elo` | 0.35 | player's avg CPL vs empirical CPL-by-ELO curve |
| `complexity_weighted_t1` | 0.25 | T1 rate on positions passing the complexity filter |
| `opening_deviation` | 0.00 | **dropped**; book used only to filter |

Thresholds: `score ≥ 0.60 → Review`, `≥ 0.80 → Flag`. Never auto-ban.

Complexity filter (configurable): skip positions where `eval(top1) − eval(top2) < delta_cp`, with `delta_cp` defaulting to **20 cp middlegame / 40 cp endgame** (per-phase config, not a single hardcoded value).

`expected_cpl_baseline_vs_elo`: a fitted curve mapping ELO → expected average CPL (literature values as seed, refined from your own clean games later). Signal = z-score of player's observed CPL against the curve at their stored on-chain ELO. This is implementable in Phase 1; IPR is not.

## Capacity sizing (revised, 15 s/game average)

Assumes depth-18 Stockfish ≈ 100 ms/move in quiet positions, 300–800 ms in complex middlegames → **~15 s CPU-time per contested game** (3× the earlier estimate).

| Hetzner box | vCPU | ~€/mo | SF workers | Games/day | Live games supported |
|---|---|---|---|---|---|
| **CX32 (current)** | 4 | ~€8 | 2–3 | **~15k** | ~400 |
| CPX41 | 8 | ~€25 | 6–7 | ~40k | ~1 500 |
| AX42 | 16 | ~€55 | 13 | ~80k | ~5 000 |
| AX102 | 32 | ~€120 | 26 | ~160k | ~12 000 |

**CX32 verdict:** fine for beta and early mainnet — 15k analysed games/day is well above realistic early volume. Reserve 1 core for signing/Iroh/Axum. Migrate to CPX41 when sustained queue depth > 5 min.

## Hot/cold path

```
Hetzner CX32
├── axum signing server (hot path)
│    ├── record_move  ─► insert into moves table (Step 0)
│    └── on game_end  ─► tokio::spawn(enqueue_game(GameRecord))
├── xfchess-anticheat workers (cold path)
│    ├── 2–3 Stockfish UCI subprocesses
│    └── writes reports/<game_id>.{txt,json} + DB verdict row
└── sqlite: moves + verdicts + features
```

## Report artefacts

Two files per `Review`/`Flag` game:
- `reports/<game_id>.txt` — human-readable (game meta, wager, ELOs, signal breakdown, per-move table, PGN, reviewer checklist).
- `reports/<game_id>.json` — stable schema (versioned), same content structured for future ML ingestion.

DB row keyed by `(game_id, engine_version, depth)` for idempotency.

## Build order

1. **Step 0** — add `moves` table migration + persistence hook in `record_move`. Deploy + verify row writes.
2. Scaffold crate, add to workspace `Cargo.toml`, declare deps (`tokio`, `shakmaty`, `serde`, `sqlx`, `tracing`, `thiserror`).
3. `types.rs` + JSON schema + `ingest.rs` against the `moves` table shape. Tests with synthetic rows.
4. Pure features in order: `opening.rs` → `complexity.rs` → `accuracy.rs` → `timing.rs`. Each ships with unit tests + fixture PGNs.
5. `scoring/` with config-driven weights, threshold tests, golden score tests on fixtures.
6. `engine/stockfish.rs` (UCI wrapper) + `job_queue.rs` (bounded mpsc, N workers).
7. `report/txt.rs` + `report/json.rs` + `store.rs` with golden-file tests.
8. Wire the `enqueue_game` hook at game-end in the signing server (tournament + PvP completion paths only).
9. Deploy to CX32, replay historical games, confirm signing p99 latency unchanged.

## Testing discipline

- Test-first on all pure modules.
- Fixture set (explicit labels):
  - 3× clean human games (real PGNs from a known-clean pool) → expect `Clean`.
  - 3× synthetic **engine-vs-human** games (Stockfish-assisted side labelled) → expect `Review`/`Flag` on assisted side.
  - 1× booked-opening game (20+ booked plies) → expect `Clean`.
  - 1× endgame with long forced sequences → expect `Clean` (complexity filter must trip).
  - 1× time-scramble game (both sides low on clock) → expect `Clean` despite elevated CPL.
- Golden-file tests for both TXT and JSON outputs.
- Integration test: `enqueue_game` → verdict end-to-end; bounded-queue back-pressure under burst load.

## Acceptance criteria

- Crate compiles clean in workspace, zero warnings on its own code.
- All unit + integration tests green.
- Signing-server p99 latency unchanged after the two hooks (measure before/after).
- Fixture set passes the expected verdicts above.
- CX32 sustains 10× expected peak load with queue depth < 5 min.

## Layman summary — what you actually get out of this

**In plain terms, once Phase 1 is shipped:**

- **Every ranked/wagered game you host gets a post-match "integrity check"**, automatically, on the same server you already run. Players don't see it, it doesn't slow anything down, and it only runs on PvP and tournament games.
- **The server builds a reliable record of every move**, including exactly when it was signed. This is your tamper-proof audit log — no "I didn't make that move" disputes, ever.
- **A chess engine (Stockfish) double-checks each game afterwards** and compares the player's choices to what a top engine would play, accounting for opening theory (booked moves ignored) and forced positions (obvious-only-move positions ignored).
- **It also compares move timing** — humans pause to think, engines respond fast and consistently. Anything suspicious gets flagged.
- **It knows the player's on-chain ELO**, so "a 1500-rated player suddenly playing at 2400 level" lights up, while a genuine strong player playing at their level doesn't.
- **For every game that looks off, you get a text report on disk** — PGN, player IDs, ELOs, wager, per-move breakdown, timing chart, reviewer checklist. A human (you or a mod) decides whether to act. **Nobody gets auto-banned.**
- **It's cheap.** Your current CX32 (~€8/mo) handles ~15 000 analysed games per day — far more than the platform will see for a long time. You only upgrade when the queue starts falling behind, and the next tier (CPX41 at ~€25/mo) already gives you 40 000/day.
- **What it does NOT do yet (and that's deliberate):** it does not try to detect two accounts colluding, it does not do fancy ML, and it does not ban anyone on its own. Those are Phase 2+ decisions, built on top of the data this phase collects.

In one sentence: **cheap, transparent, human-reviewed cheat detection on every ranked game, running alongside the server you already pay for, producing a printable report you can defend in a dispute.**
