# Puzzles & Puzzle Payouts — Backend Design

> Status: design / not yet implemented.
> Owner: see [CLAUDE.md](../CLAUDE.md). Companion to [PATH_TO_MAINNET.md](PATH_TO_MAINNET.md).

This document explains how to add a **puzzle pool** to XFChess, store it in the
backend on the VPS, serve puzzles to the game client, verify solutions
server-side, and (optionally) **pay users in SOL** for solving them. It is
deliberately detailed about the parts that are easy to get wrong when money is
involved.

The two **player** UI entry points already exist as stubs in
[`src/states/main_menu/new_menu.rs`](../src/states/main_menu/new_menu.rs):
"Solve Puzzles" (Play) and "Puzzle Rush (Earn)". This doc fills in everything
behind those two TODOs.

The **operator** UI is an **extension of the existing
[`tauri/tournament-admin`](../tauri/tournament-admin) desktop app** — a new
"Puzzles" page to index the pool by ELO and name, select puzzles, and fund them
with the VPS authority key, reusing the same funding model as
`fund_tournament_prize`. See **§9**.

---

## 1. What a "puzzle" actually is

A puzzle is **static data, not a live game**. This is the single most important
mental model and it comes straight from Lichess
([`reference/lila/modules/puzzle/src/main/Puzzle.scala`](../reference/lila/modules/puzzle/src/main/Puzzle.scala)):

```
id      — short stable id (Lichess uses 5 chars)
fen     — the starting position
line    — the ONE forced solution, as a list of UCI moves
rating  — how hard the puzzle is (a Glicko rating, like a player's)
themes  — tags: "mateIn2", "fork", "endgame", ...
plays   — how many times it's been attempted
```

The `line` is the forced sequence. The **first** move in `line` is the
opponent's "setup" move (the engine plays it to create the tactic); every move
**after** that is the player's job. A puzzle is **won** only if the player plays
the entire remaining `line` with zero wrong moves. One wrong move = loss.

Because a puzzle is just `fen + line`, the whole pool is just rows in a table.
There is no per-puzzle game server, no opponent, no matchmaking. The backend
only ever **serves a position** and later **checks the submitted moves**.

### The critical difference from Lichess

In Lichess, the **client tells the server "I won"** and the server trusts it for
rating purposes (see
[`PuzzleComplete.scala`](../reference/lila/modules/puzzle/src/main/PuzzleComplete.scala)
/ [`PuzzleFinisher.scala`](../reference/lila/modules/puzzle/src/main/PuzzleFinisher.scala)
— the `win` boolean is supplied by the client). That is fine when only an ELO
number is at stake.

**It is unacceptable when SOL is at stake.** Therefore in XFChess:

- The `line` (solution) **never leaves the VPS**. The client receives only
  `fen` + whose-move.
- The client submits the moves it played; **the backend** replays them against
  the stored `line` and decides win/loss.
- Only a **server-verified** win can trigger a payout.

This inverts the trust model: the server is authoritative, the client is a
renderer.

---

## 2. The pool: where the puzzles come from

**You do not need to build a puzzle generator.** Lichess publishes its entire
puzzle database as a free, open CSV (CC0 / ODbL) at
`https://database.lichess.org/#puzzles` — roughly 4–5 million puzzles. Each row:

```
PuzzleId,FEN,Moves,Rating,RatingDeviation,Popularity,NbPlays,Themes,GameUrl,OpeningTags
00sHx,q3k1nr/...,e8d7 a2e6 d7d8 f7f8,1760,80,94,4067,mate mateIn2 ...,https://...,
```

- `FEN` → our `fen`
- `Moves` → our `line` (already space-separated UCI; first move is the setup)
- `Rating` → our `rating`
- `Themes` → our `themes`

Import this once and you have a professional-grade, rating-calibrated pool on
day one. (Attribution required; ODbL is compatible with the project's AGPL.)

Later, if you want XFChess-native puzzles mined from your own games, that's a
separate offline batch job (Lichess's is `lichess-puzzler`, a Stockfish
pipeline — out of scope here). The serving/payout design below does not care
where the rows came from.

---

## 3. Storage: the backend schema

SQLite via SQLx, following the existing convention — a **new numbered
migration**, never edit an old one (see [backend/CLAUDE.md](../backend/CLAUDE.md)).
Next free number at time of writing is `018`.

### `backend/migrations/018_puzzles.sql`

```sql
-- Migration 018: puzzle pool + per-user solve records + selection sessions.

-- The pool. `line` is SERVER-ONLY and must never be serialised to a client
-- response. Treat it like a secret.
CREATE TABLE puzzles (
    id          TEXT PRIMARY KEY,        -- Lichess puzzle id (stable)
    name        TEXT,                    -- admin-assignable label (nullable);
                                         -- imported puzzles have none until named
    fen         TEXT NOT NULL,           -- starting position
    line        TEXT NOT NULL,           -- space-separated UCI, incl. setup move
    rating      INTEGER NOT NULL,        -- difficulty (Glicko rating / "ELO")
    rating_dev  INTEGER NOT NULL DEFAULT 80,
    themes      TEXT NOT NULL DEFAULT '',-- space-separated theme tags
    plays       INTEGER NOT NULL DEFAULT 0,
    nb_wins     INTEGER NOT NULL DEFAULT 0,
    featured    INTEGER NOT NULL DEFAULT 0,  -- admin "selected" flag
    enabled     INTEGER NOT NULL DEFAULT 1
);

-- Rating-band index drives ELO selection (see §5) and the admin ELO browser (§9).
CREATE INDEX idx_puzzles_rating ON puzzles(rating) WHERE enabled = 1;
-- Name index drives the admin name search (§9).
CREATE INDEX idx_puzzles_name ON puzzles(name) WHERE name IS NOT NULL;

-- One row per (user, puzzle) attempt. Prevents re-solving the same puzzle for
-- another payout, and feeds rating + anti-cheat.
CREATE TABLE puzzle_rounds (
    wallet      TEXT NOT NULL,
    puzzle_id   TEXT NOT NULL,
    win         INTEGER NOT NULL,        -- 1 = solved cleanly, 0 = failed
    rated       INTEGER NOT NULL DEFAULT 1,
    solve_ms    INTEGER,                 -- total think time, client claim (audited)
    paid_sig    TEXT,                    -- Solana tx signature if a payout fired
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (wallet, puzzle_id)
);
CREATE INDEX idx_puzzle_rounds_wallet ON puzzle_rounds(wallet, created_at);

-- Each user's puzzle rating (mirrors Lichess's per-perf Glicko).
CREATE TABLE puzzle_ratings (
    wallet      TEXT PRIMARY KEY,
    rating      INTEGER NOT NULL DEFAULT 1500,
    rating_dev  INTEGER NOT NULL DEFAULT 350,
    nb          INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL
);

-- An in-progress solve the server has committed to. This is what makes
-- server-side verification possible: we record which puzzle we served and a
-- nonce, so the solve submission can't be forged or replayed.
CREATE TABLE puzzle_challenges (
    nonce       TEXT PRIMARY KEY,        -- random, returned to client
    wallet      TEXT NOT NULL,
    puzzle_id   TEXT NOT NULL,
    mode        TEXT NOT NULL,           -- 'solve' | 'rush'
    rush_id     TEXT,                    -- groups a Puzzle Rush run
    issued_at   INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    consumed    INTEGER NOT NULL DEFAULT 0
);

-- Admin-funded bounties. An admin selects a puzzle (or rating band) in the
-- tournament-admin app and locks SOL against it, drawn from and signed by the
-- VPS authority key (see §9 funding). A solve pays from this budget.
CREATE TABLE puzzle_bounties (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    scope           TEXT NOT NULL,           -- 'puzzle' | 'band' | 'daily'
    puzzle_id       TEXT,                    -- set when scope='puzzle'
    band_lo         INTEGER,                 -- ELO band when scope='band'
    band_hi         INTEGER,
    reward_lamports INTEGER NOT NULL,        -- payout per successful solve
    budget_lamports INTEGER NOT NULL,        -- total SOL locked for this bounty
    spent_lamports  INTEGER NOT NULL DEFAULT 0,
    max_per_wallet  INTEGER NOT NULL DEFAULT 1,
    fund_sig        TEXT,                    -- VPS-signed funding tx signature
    vault_pubkey    TEXT,                    -- on-chain prize vault PDA (if used)
    created_by      TEXT NOT NULL,           -- admin token / authority pubkey
    status          TEXT NOT NULL DEFAULT 'active', -- 'active'|'exhausted'|'closed'
    created_at      INTEGER NOT NULL
);
CREATE INDEX idx_puzzle_bounties_active ON puzzle_bounties(status, scope);

-- A wagered Puzzle Rush run (the "Earn" mode).
CREATE TABLE puzzle_rush_runs (
    rush_id     TEXT PRIMARY KEY,
    wallet      TEXT NOT NULL,
    stake_lamports INTEGER NOT NULL,
    stake_sig   TEXT,                    -- proof the stake was paid in
    score       INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL,           -- 'open' | 'finished' | 'paid'
    payout_lamports INTEGER,
    payout_sig  TEXT,
    started_at  INTEGER NOT NULL,
    finished_at INTEGER
);
```

Why a `puzzle_challenges` table and a `nonce`? Because the solve endpoint must
be **stateful and single-use**. When we serve a puzzle we record "wallet W is
now allowed to submit a solution for puzzle P under nonce N, until time T." The
client echoes N back when submitting. This blocks:

- submitting a solution for a puzzle you were never served,
- replaying a winning submission twice,
- racing two solves of the same puzzle for two payouts.

It mirrors the move-replay nonce window already used elsewhere in the backend
(see the auth-hardening work tracked in memory).

---

## 4. Importing the pool

A one-off binary, matching the existing `src/bin/` pattern
([backend/CLAUDE.md](../backend/CLAUDE.md) lists `vps_admin`, `tournament_admin`).

### `backend/src/bin/import_puzzles.rs` (sketch)

```rust
// cargo run --bin import_puzzles -- ./lichess_db_puzzle.csv
//
// Streams the Lichess puzzle CSV into the `puzzles` table. Idempotent:
// INSERT OR REPLACE on the primary key.
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("csv path");
    let pool = SqlitePool::connect(&std::env::var("DATABASE_URL")?).await?;
    let mut rdr = csv::Reader::from_path(path)?;

    let mut tx = pool.begin().await?;
    let mut n = 0u64;
    for rec in rdr.records() {
        let r = rec?;
        // columns: 0=id 1=fen 2=moves 3=rating 4=ratingDev .. 7=themes
        sqlx::query(
            "INSERT OR REPLACE INTO puzzles
             (id, fen, line, rating, rating_dev, themes)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&r[0]).bind(&r[1]).bind(&r[2])
        .bind(r[3].parse::<i64>().unwrap_or(1500))
        .bind(r[4].parse::<i64>().unwrap_or(80))
        .bind(&r[7])
        .execute(&mut *tx).await?;
        n += 1;
        if n % 50_000 == 0 { tx.commit().await?; tx = pool.begin().await?; }
    }
    tx.commit().await?;
    println!("imported {n} puzzles");
    Ok(())
}
```

Run once during VPS provisioning. ~5M rows in SQLite is a few hundred MB — fine
on the Hetzner box. You can pre-filter the CSV (e.g. drop deviation > 100, or
ratings outside 800–2600) to shrink it.

---

## 5. Selecting the next puzzle

Lichess pre-bundles puzzles into rating-banded "paths" and walks a per-user
session through them
([`PuzzlePath.scala`](../reference/lila/modules/puzzle/src/main/PuzzlePath.scala),
[`PuzzleSelector.scala`](../reference/lila/modules/puzzle/src/main/PuzzleSelector.scala)).
That machinery exists to make MongoDB fast at their scale. **We don't need the
path abstraction** — a direct indexed query is plenty for one SQLite box:

> Pick a random enabled puzzle whose rating is within a window around the
> player's puzzle rating, that this wallet has **not** already played.

```sql
SELECT id, fen, line, rating, themes
FROM puzzles
WHERE enabled = 1
  AND rating BETWEEN :lo AND :hi
  AND id NOT IN (SELECT puzzle_id FROM puzzle_rounds WHERE wallet = :wallet)
ORDER BY RANDOM()
LIMIT 1;
```

- `:lo / :hi` = player rating ± window. Start window ≈ ±150; widen on retry if
  empty (mirrors Lichess's `compromise` back-off in `PuzzlePath.nextFor`).
- New players default to rating 1500 (see `puzzle_ratings`).
- For **Puzzle Rush**, don't pick randomly — serve a **fixed climbing ladder**
  of increasing rating, exactly like
  [`PuzzleStreak.scala`](../reference/lila/modules/puzzle/src/main/PuzzleStreak.scala)
  (its buckets run 1050 → 2799). Precompute the ladder when the run starts.

`ORDER BY RANDOM()` over a rating-banded index is acceptable here; if it ever
shows up in profiling, switch to "pick a random offset in the band."

---

## 6. The HTTP API

New route module `backend/src/signing/routes/puzzle.rs`, registered in
[`routes/mod.rs`](../backend/src/signing/routes/mod.rs) next to the others
(`pub mod puzzle;`). It follows the exact shape of
[`routes/history.rs`](../backend/src/signing/routes/history.rs):
`Router<AppState>`, a `GameRepository`-style repo, `State(state)` extractors.

```
GET  /puzzle/next?mode=solve
        -> { nonce, id, fen, color }          // NO line
POST /puzzle/solve
        body: { nonce, moves: ["e2e4", ...] }
        -> { win, rating, rating_diff, payout_sig? }

POST /puzzle/rush/start
        body: { stake_lamports }              // returns unsigned stake tx
        -> { rush_id, unsigned_tx }
GET  /puzzle/rush/next?rush_id=...
        -> { nonce, id, fen, color } | { rush_complete: true }
POST /puzzle/rush/solve
        body: { rush_id, nonce, moves: [...] }
        -> { correct, score, next? }
POST /puzzle/rush/finish
        body: { rush_id }
        -> { score, payout_lamports, payout_sig }
```

### Serving (`GET /puzzle/next`)

1. Auth the wallet (existing JWT/session middleware).
2. Run the selection query (§5). Load the full row **including `line`** —
   server-side only.
3. Create a `puzzle_challenges` row: random `nonce`, `wallet`, `puzzle_id`,
   `expires_at = now + 10min`.
4. Respond with `{ nonce, id, fen, color }`. **Strip `line` from the response
   struct so it cannot leak.** (Use a dedicated `PuzzleServeDto` that has no
   `line` field — don't reuse the DB row struct.)

### Verifying (`POST /puzzle/solve`) — the heart of it

```rust
// pseudocode
let ch = challenges.consume(nonce, wallet)?;        // single-use; errors if
                                                    // missing/expired/consumed
let puzzle = puzzles.get(ch.puzzle_id)?;            // has the secret `line`

// line[0] is the engine setup move; the player must match line[1..].
let expected: Vec<&str> = puzzle.line.split(' ').skip(1).collect();
let win = submitted_moves == expected;             // exact, in order

// Optional but recommended: also legality-check each move with
// chess-logic-on-chain so a malformed submission can't slip through.

rounds.upsert(wallet, puzzle.id, win, solve_ms);   // PK (wallet,puzzle) ⇒ no
                                                    // double credit
update_puzzle_rating(wallet, puzzle, win);         // Glicko, mirrors Finisher

let payout_sig = if win && eligible_for_payout(wallet, &puzzle) {
    Some(build_and_record_payout(wallet, reward_for(&puzzle)).await?)
} else { None };
```

The comparison is `submitted == expected[1..]`, exact and ordered. There is no
"close enough." `chess-logic-on-chain` (no_std, already in the workspace) can be
used to assert each submitted move is legal in the running position, hardening
against junk input — but the **win condition is the string match against the
secret line**.

---

## 7. Anti-cheat gate (mandatory before any payout)

This is the piece that decides whether "earn" attracts users or a bot swarm.
A puzzle is single-player and deterministic, so a script with an engine solves
every puzzle instantly and perfectly. The defenses, reusing what already exists:

1. **Already-played guard.** `puzzle_rounds` PK `(wallet, puzzle_id)` — a wallet
   can only ever be paid once per puzzle. (Free tier.)

2. **Think-time floor.** The backend records `issued_at` (when it served) and
   the solve timestamp; the gap is server-observed, not a client claim. This is
   the same model as
   [`migration 014_think_time.sql`](../backend/migrations/014_think_time.sql)
   ("think_ms is a client claim ... audited against the server-observed wall
   clock"). A 2200-rated puzzle solved in 400ms with no errors is an engine.
   Flag/withhold payout below a rating-scaled time floor.

3. **Dubious-rating check.** Lichess's `dubiousPuzzle` (in
   [`PuzzleFinisher.scala`](../reference/lila/modules/puzzle/src/main/PuzzleFinisher.scala))
   stops rating a player whose puzzle rating is implausibly high vs. their game
   rating. Port the same idea: a wallet whose solve pattern is statistically
   engine-like gets rated but **not paid**.

4. **IP / sybil gate + per-wallet daily cap.** Reuse the IP-based anti-cheat in
   `backend/src/signing/` and `tasks/anticheat_worker.rs`. Cap free-tier payouts
   per wallet **and per IP** per day so the faucet has a bounded daily cost.

5. **Make the wagered mode the real earner.** See §8 — staked Puzzle Rush can't
   be farmed for free because entry costs SOL.

The free "Solve Puzzles" tier should pay little or nothing (or only a small
**daily** bonus) — its job is acquisition (get a wallet connected), not to be a
SOL spigot.

---

## 8. Payouts — reuse, don't reinvent

The backend already builds-but-never-signs Solana transactions and already
settles game wagers and tournament prizes:

- `backend/src/signing/solana/instructions.rs` — instruction builders.
- `backend/src/tasks/settlement_worker.rs` — scans games, auto-submits payouts.
- `backend/src/tasks/fee_claimer.rs` — vault claims.
- `signing/routes/tournament.rs` + `tasks/tournament_scheduler.rs` — pooled
  prize distribution (the model the wagered mode copies).
- A fee/prize vault exists in `programs/xfchess-game/src/account_ix/`.
- The **global session key** (see memory `project_global_session.md`) means the
  payout needs no extra wallet popup.

### Model A — House-funded bounty (free tier, capped)

Solve a puzzle (server-verified) → backend builds a vault→player transfer →
records `paid_sig` on the `puzzle_rounds` row. This is a **faucet**: gate it hard
with §7 and a strict daily cap. Good for a single **Daily Puzzle** (§10), bad as
an uncapped reward on every solve.

### Model B — Wagered Puzzle Rush (the "Earn" button, recommended)

Self-funding and abuse-resistant:

1. `POST /puzzle/rush/start { stake_lamports }` → backend returns an **unsigned
   stake transaction** (client signs, stake lands in the prize vault); record a
   `puzzle_rush_runs` row (`status='open'`).
2. Serve the fixed climbing ladder (§5 / `PuzzleStreak`). Each solve advances
   `score`; first miss ends the run.
3. `POST /puzzle/rush/finish` → backend computes payout from `score` and the
   pooled stakes minus a rake, builds the payout tx (same path as tournament
   prize distribution), records `payout_sig`, sets `status='paid'`.

Economically this is pari-mutuel like the tournaments — growth here **pays** a
rake instead of costing a vault. This is the mode the "Puzzle Rush (Earn)"
button should launch.

> **Decision needed:** free-tier reward policy for Model A (nothing / daily-only
> / small per-solve cap), and the Model B payout curve + rake.

---

## 9. Admin: curation & funding (tournament-admin extension)

> **This is the operator's control surface.** Puzzle administration is an
> **extension of the existing
> [`tauri/tournament-admin`](../tauri/tournament-admin) app**, not a new tool.
> It reuses the same auth, the same `ApiClient`, the same `Layout`, and — most
> importantly — the **same VPS-authority funding model** that
> `fund_tournament_prize` already uses.

The operator needs to: **index the pool by ELO and by name, select puzzles, and
fund them with the VPS key.** Each maps directly onto an existing pattern in the
admin app.

### 9.1 How the VPS key funds things today (the model to copy)

The backend holds the VPS authority keypair in memory as
`state.vps_authority: Arc<Keypair>` (loaded from `VPS_AUTHORITY_KEY`, see
[`signing/mod.rs`](../backend/src/signing/mod.rs) and
[`signing/config.rs`](../backend/src/signing/config.rs)). It can sign and submit
**server-side, with no CLI step**. The canonical example is
[`fund_tournament_prize`](../backend/src/signing/routes/admin.rs) (admin.rs
~L506):

```rust
let authority = state.vps_authority.clone();           // the VPS key
let sig = tokio::task::spawn_blocking(move || {
    let rpc = make_rpc(&rpc_url);
    let ix = fund_sol_prize_ix(&program_id, id, &authority.pubkey(), amount);
    sign_and_submit(&rpc, &authority, &[ix])           // VPS key signs + submits
}).await??;
add_audit("fund_prize", &format!("tournament:{id}"), &format!("{amount} lamports"));
```

**Puzzle funding copies this exactly** — only the instruction changes
(`fund_puzzle_prize_ix` instead of `fund_sol_prize_ix`), or, for an off-chain
bounty budget, a plain SOL transfer from `vps_authority` into a puzzle vault.
The VPS key is the funding source and the signer; the admin app only triggers it.

### 9.2 Backend admin endpoints (`routes/admin.rs` or a new `routes/puzzle_admin.rs`)

All behind the existing admin `X-API-Key` middleware, all `add_audit(...)`-logged
like every other admin mutation:

```
GET  /admin/puzzles?elo_min=&elo_max=&name=&theme=&limit=&offset=
        -> { total, puzzles: [{ id, name, rating, themes, plays, nb_wins,
                                featured, bounty? }] }     // INCLUDES line for
                                                          // admin preview only
GET  /admin/puzzles/{id}
        -> full puzzle incl. fen + line (admin may inspect the solution)
POST /admin/puzzles/{id}/name      { name }               -- label a puzzle
POST /admin/puzzles/{id}/feature   { featured: bool }     -- the "select" toggle
POST /admin/puzzles/{id}/enable    { enabled: bool }      -- pull a bad puzzle

POST /admin/puzzles/fund                                  -- THE funding action
        body: { scope: "puzzle"|"band"|"daily",
                puzzle_id?, band_lo?, band_hi?,
                reward_lamports, budget_lamports, max_per_wallet }
        -> { bounty_id, fund_sig }     // signed by vps_authority, like fund_prize
GET  /admin/puzzles/bounties
        -> active bounties + spent/budget so the operator sees burn-down
POST /admin/puzzles/bounties/{id}/close
        -> stop paying; (optionally) sweep remaining budget back to VPS wallet
```

- **Index by ELO** → the `elo_min/elo_max` filter is the same indexed rating
  query as §5, served as a paged list (mirror
  [`routes/history.rs`](../backend/src/signing/routes/history.rs) shape).
- **Index by name** → `name LIKE :q` (the `idx_puzzles_name` index). Also allow
  exact `id` lookup, since imported puzzles start nameless.
- **Select** → `feature`/`name`/`enable` toggles + `POST /admin/puzzles/fund`
  with `scope="puzzle"`.
- **Fund with the VPS key** → `/admin/puzzles/fund` runs the §9.1 pattern,
  writes a `puzzle_bounties` row with the returned `fund_sig`. The payout path
  (§6/§8) then debits `spent_lamports` per solve and flips the bounty to
  `exhausted` when `spent >= budget`.

### 9.3 On-chain piece (optional, mirrors tournaments)

For trustless escrow, add a **puzzle prize vault PDA** in
`programs/xfchess-game/src/account_ix/` and a `fund_puzzle_prize_ix` mirroring
the tournament `fund_sol_prize_ix`. The bounty's `vault_pubkey` then points at
real on-chain escrow and the solve payout draws from it. **v1 can skip this** and
keep the bounty budget as a plain VPS-wallet transfer accounted in
`puzzle_bounties` — same staging you used for moves (off-chain first, harden
later).

### 9.4 Frontend: a `Puzzles` page in tournament-admin

A new page added exactly like `Treasury` / `TournamentList`:

1. **`src/components/Puzzles.tsx`** — three zones, copying the visual language of
   [`Treasury.tsx`](../tauri/tournament-admin/src/components/Treasury.tsx):
   - **Browser** — filter bar (ELO min/max, name/id search, theme), paged table
     of `{ name, id, rating, themes, plays, win%, featured, bounty }`. This is
     the "index by ELO and name" surface.
   - **Inspector** — select a row → preview FEN (reuse `PgnViewer`/board), see
     the solution `line`, set a `name`, toggle `featured`/`enabled`.
   - **Funding** — a form (puzzle / ELO band / daily, reward per solve, total
     budget, max per wallet) → `BUILD & FUND` → calls `/admin/puzzles/fund`.
     Shows the returned `fund_sig` and a live burn-down of active bounties.
     This mirrors Treasury's funding/refund panel and the
     `fund_tournament_prize` flow.

2. **`src/services/api.ts`** — add methods next to the tournament ones:
   ```ts
   async listPuzzles(q: { eloMin?: number; eloMax?: number; name?: string;
                          theme?: string; limit?: number; offset?: number }) { ... }
   async getPuzzle(id: string) { ... }
   async namePuzzle(id: string, name: string) { ... }
   async featurePuzzle(id: string, featured: boolean) { ... }
   async fundPuzzle(body: { scope: string; puzzleId?: string; bandLo?: number;
                            bandHi?: number; rewardLamports: number;
                            budgetLamports: number; maxPerWallet: number }) { ... }
   async getPuzzleBounties() { ... }
   ```

3. **`src/App.tsx`** — add `"puzzles"` to the `Page` union and a
   `case "puzzles": return <Puzzles />;`.

4. **`src/components/common/Layout.tsx`** — add a "Puzzles" nav entry beside
   "Treasury".

Net: the operator opens the admin app, goes to **Puzzles**, filters by ELO band
and name, selects a puzzle, and clicks fund — the backend signs the funding tx
with the VPS authority key and records the bounty. No new auth, no new signing
model, no CLI.

---

## 10. Game client integration (`src/`)

Mirror the existing `AppState::Game` flow (see [CLAUDE.md](../CLAUDE.md) §Game
client). The client is a **renderer + input collector**, never the judge.

1. New `src/puzzle/` module and an `AppState::Puzzle` variant in
   `core/` alongside `Splash → MainMenu → Game → Pause`.
2. On entering: `GET /puzzle/next`, receive `{ nonce, fen, color }`, set up the
   board via the existing FEN/board rendering in `game/` + `rendering/`.
3. Collect the player's moves. Optionally pre-validate locally with
   `chess-logic-on-chain` for instant red/green feedback — **but the server's
   verdict is authoritative.**
4. On the last move, `POST /puzzle/solve { nonce, moves }`. Show the returned
   `win`, `rating_diff`, and (if any) payout confirmation.
5. Wire the two stubs:
   - [`new_menu.rs:865`](../src/states/main_menu/new_menu.rs#L865) "Solve
     Puzzles" → enter `AppState::Puzzle` in `solve` mode.
   - [`new_menu.rs:879`](../src/states/main_menu/new_menu.rs#L879) "Puzzle Rush
     (Earn)" → stake flow, then `AppState::Puzzle` in `rush` mode.

---

## 11. Minimal first ship: the Daily Puzzle

Don't build all of the above before shipping something. The smallest valuable
slice, copying [`DailyPuzzle.scala`](../reference/lila/modules/puzzle/src/main/DailyPuzzle.scala):

- One shared puzzle per day (pick one row, pin it for 24h).
- `GET /puzzle/daily` (serve) + `POST /puzzle/daily/solve` (verify) — no rating,
  no sessions, just the nonce + verify path.
- Small fixed house bounty, one claim per wallet per day (Model A, tightly
  capped).
- Shareable result ("I solved today's XFChess puzzle") = free organic reach.

This exercises the whole serve→verify→pay pipeline with almost none of the
selection/rating machinery, and is a daily reason to open the app.

---

## 12. Phased build plan

| Phase | Deliverable | Touches |
|-------|-------------|---------|
| 0 | `018_puzzles.sql` migration + `import_puzzles` binary; load Lichess CSV | `backend/migrations/`, `backend/src/bin/` |
| 1 | `routes/puzzle.rs`: serve + **server-side verify** (no payout, no rating) | `backend/src/signing/routes/` |
| 2 | Client `AppState::Puzzle` + wire "Solve Puzzles" stub | `src/puzzle/`, `src/states/main_menu/` |
| 3 | Puzzle rating (Glicko) + `puzzle_rounds`/`puzzle_ratings` | `backend/src/db/`, `routes/puzzle.rs` |
| 4 | **Admin (tournament-admin extension §9):** ELO/name index endpoints + `Puzzles.tsx` page (browse, select, no funding yet) | `routes/admin.rs`, `tauri/tournament-admin/` |
| 5 | Anti-cheat gate: think-time floor, dubious-rating, daily caps | reuse `tasks/anticheat_worker.rs`, `signing/` |
| 6 | **Admin funding** `/admin/puzzles/fund` via `vps_authority` + `puzzle_bounties`; funding panel in `Puzzles.tsx` | `routes/admin.rs`, `tauri/tournament-admin/` |
| 7 | Daily Puzzle + capped house bounty (Model A), paid from a bounty | `routes/puzzle.rs`, `signing/solana/` |
| 8 | Wagered Puzzle Rush (Model B): stake → ladder → payout | `routes/puzzle.rs`, reuse tournament payout |
| 9 | (optional) On-chain puzzle prize vault + solution verification | `programs/xfchess-game/` |

Phases 0–4 are shippable as a **free, unpaid** puzzle trainer with an admin
browser (zero financial risk). Money only enters at Phase 6+, after the
anti-cheat gate (Phase 5) and the VPS-key funding plumbing are in place.

---

## 13. Open decisions

1. **Free-tier reward:** nothing, daily-only, or a small capped per-solve
   bounty? (Drives faucet cost + abuse surface.)
2. **Rush payout curve + rake:** how does payout scale with streak length, and
   what's the house cut?
3. **Funding escrow (§9.3):** keep admin-funded bounties as plain VPS-wallet
   transfers accounted in `puzzle_bounties`, or escrow them in an on-chain puzzle
   prize vault PDA? (VPS-wallet v1, on-chain later — same as moves.)
4. **On-chain verification (Phase 9):** trust the VPS verdict, or also prove the
   solution inside the Solana program for trustlessness?
5. **Pool filtering:** import all ~5M puzzles, or pre-filter by rating band /
   deviation / theme to keep the DB small and the quality high?

---

## 14. Key references

- Lila puzzle module:
  [`reference/lila/modules/puzzle/src/main/`](../reference/lila/modules/puzzle/src/main/)
  — `Puzzle.scala` (data model), `PuzzleSelector.scala` (selection),
  `PuzzleFinisher.scala` (rating + dubious check), `PuzzleStreak.scala` (rush
  ladder), `DailyPuzzle.scala` (daily), `PuzzleComplete.scala` (the
  client-trusted flow we deliberately do **not** copy for payouts).
- Existing backend patterns to copy:
  [`routes/history.rs`](../backend/src/signing/routes/history.rs) (route shape),
  [`db/repository.rs`](../backend/src/db/repository.rs) (repo + FromRow),
  [`tasks/settlement_worker.rs`](../backend/src/tasks/settlement_worker.rs) +
  `tournament_scheduler.rs` (payout machinery),
  [`migrations/014_think_time.sql`](../backend/migrations/014_think_time.sql)
  (server-audited timing).
- VPS-key funding to copy:
  [`routes/admin.rs`](../backend/src/signing/routes/admin.rs) `fund_tournament_prize`
  (signs with `state.vps_authority` and `add_audit`s),
  [`signing/mod.rs`](../backend/src/signing/mod.rs) (`vps_authority: Arc<Keypair>`),
  [`signing/config.rs`](../backend/src/signing/config.rs) (`VPS_AUTHORITY_KEY`).
- Admin app to extend:
  [`tauri/tournament-admin/src/App.tsx`](../tauri/tournament-admin/src/App.tsx) (page routing),
  [`Treasury.tsx`](../tauri/tournament-admin/src/components/Treasury.tsx) (funding-panel pattern),
  [`services/api.ts`](../tauri/tournament-admin/src/services/api.ts) (`ApiClient` + admin endpoints).
- Lichess open puzzle DB: `https://database.lichess.org/#puzzles`.
