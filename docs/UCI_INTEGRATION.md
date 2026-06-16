# UCI Integration — current state & roadmap

> **Date:** 2026-06-15
> **Scope:** `crates/engine/nimzovich_engine`, `crates/engine/nimzovich-uci`, and the game-client AI paths in `src/game/ai/`.
> **Goal:** make UCI (Universal Chess Interface) a first-class boundary in XFChess so any engine — Nimzovich, Stockfish, Leela — can be driven through one interface, and so the engine can be reused for analysis, anti-cheat, puzzles, and online bot play.

---

## TL;DR

| Area | Today | Target |
|------|-------|--------|
| Stockfish in game | UCI subprocess, **hand-inlined lossy client** | Shared `uci-client` crate |
| Nimzovich in game | Direct Rust API (in-process, pooled 2.2 GB TT) | Keep in-process, behind a common `Engine` trait |
| `nimzovich-uci` binary | Minimal UCI **server**, used only for cutechess-cli | Full UCI engine (stop/ponder/mate/PV/MultiPV) |
| Engine selection | Two parallel code paths, no abstraction | One `Engine` trait, two backends |
| Backend engine use | None | UCI pool for anti-cheat / puzzles / analysis |

**Highest-leverage first step:** extract a shared `uci-client` crate. The only UCI client today (in `src/game/ai/systems.rs`) discards most of what the protocol reports.

---

## 1. Current architecture

There are **two AI paths** in the game client, with an asymmetry that drives this whole document.

### 1a. Nimzovich — direct Rust API (in-process)
[src/game/ai/systems.rs:309-378](../src/game/ai/systems.rs#L309) — `spawn_xf_engine_task`:
- Calls `nimzovich_engine::reply(&mut game, color).await` directly ([systems.rs:340](../src/game/ai/systems.rs#L340)).
- **Deliberately in-process** to reuse a pre-warmed, pooled `Game` and avoid re-allocating the ~2.2 GB transposition table per move ([systems.rs:298-303](../src/game/ai/systems.rs#L298)).
- Difficulty = search time / depth budget ([src/game/ai/resource.rs](../src/game/ai/resource.rs)).

> ⚠️ This in-process design is intentional and must be preserved. Routing Nimzovich through a subprocess pipe would lose the shared TT and add IPC latency. UCI is the right boundary for *external* engines, not for our own.

### 1b. Stockfish — UCI subprocess (out-of-process)
[src/game/ai/systems.rs:382-471](../src/game/ai/systems.rs#L382) — `spawn_stockfish_task_persistent`:
- Persistent child process; one-time `uci` → `uciok` → `isready` → `readyok` handshake ([systems.rs:394-410](../src/game/ai/systems.rs#L394)).
- Per move: `position fen …` then `go movetime …` / `go depth …`, read until `bestmove` ([systems.rs:413-445](../src/game/ai/systems.rs#L413)).
- **Lossy parser:** only extracts `depth` and `cp`. It ignores `score mate`, the full `pv`, `nps`, `hashfull`, `seldepth`, and `multipv` ([systems.rs:431-444](../src/game/ai/systems.rs#L431)).

### 1c. `nimzovich-uci` — UCI server, unused by the product
[crates/engine/nimzovich-uci/src/main.rs](../crates/engine/nimzovich-uci/src/main.rs) wraps Nimzovich *as* a UCI engine for cutechess-cli / GUIs. **Nothing in the game or backend consumes it** — the game talks to Nimzovich via the Rust API (1a), not this binary.

### The gap
UCI exists in two disconnected places: a minimal **client** baked into the game (for Stockfish only) and a separate **server** binary (for Nimzovich only). Neither is shared, the client is lossy, and there is no common engine abstraction.

```
                 ┌─────────────── game client ───────────────┐
   Nimzovich ───►│  reply() Rust API  (in-process, TT pool)   │
   Stockfish ───►│  inlined UCI client (lossy: cp+depth only) │
                 └────────────────────────────────────────────┘

   nimzovich-uci (UCI server)  ──►  cutechess-cli   [not used by product]
```

---

## 2. What `nimzovich-uci` supports today

A clean, minimal, **synchronous** adapter ([main.rs](../crates/engine/nimzovich-uci/src/main.rs)):

- Commands: `uci`, `isready`, `setoption` (**Hash only**), `ucinewgame`, `position` (`startpos`/`fen` + `moves`), `go`, `stop` (**no-op**), `quit`.
- Extras: `perft N`, `bench`, `d` (dump FEN/stm).
- Opening book lookup before search ([main.rs:72-86](../crates/engine/nimzovich-uci/src/main.rs#L72)).
- Time management from `movetime` / `wtime`/`btime` / `winc`/`binc` ([main.rs:158-186](../crates/engine/nimzovich-uci/src/main.rs#L158)).
- Promotion parsing; BOM/NUL tolerance for Windows shells.

### Gaps vs. a full UCI engine
- **`stop` is a no-op; no `go infinite`; no ponder** — search is synchronous and time-bounded, so it can't be interrupted ([main.rs:288-291](../crates/engine/nimzovich-uci/src/main.rs#L288)). This is the biggest gap for real GUI / bot use.
- **Always `score cp`** — never emits `score mate N` ([main.rs:101-109](../crates/engine/nimzovich-uci/src/main.rs#L101)).
- **PV is one move** — only the bestmove appears in `pv`; no full principal variation.
- **One `info` line at the end** — no per-iteration streaming of depth/seldepth/score/nodes/nps/hashfull.
- **`setoption` = Hash only** — no `Threads`, `MultiPV`, `Move Overhead`, `Ponder`, `Clear Hash`, `UCI_Chess960`.
- **`parse_go_budget` ignores `movestogo`** and there's no `go nodes` ([main.rs:158](../crates/engine/nimzovich-uci/src/main.rs#L158)) — tournament time controls are mis-budgeted.

---

## 3. Roadmap (ranked by payoff vs. effort)

### Step 1 — Extract a shared `uci-client` crate  ·  high payoff / low effort
Pull the inlined Stockfish client out of [systems.rs](../src/game/ai/systems.rs) into `crates/engine/uci-client`.

- A `UciEngine` that owns the child process + handshake state.
- `async fn bestmove(&mut self, fen: &str, limits: Limits) -> Result<SearchInfo>`.
- A **complete** `SearchInfo`: `bestmove`, `ponder`, `score` (`Cp(i32)` | `Mate(i32)`), `depth`, `seldepth`, `nodes`, `nps`, `hashfull`, `pv: Vec<String>`, `multipv`.
- `Limits { movetime, depth, nodes, wtime/btime/winc/binc/movestogo, infinite }`.

**Why first:** today's only client throws away `mate`, `pv`, `nps`, `multipv`. A shared, complete client is the foundation for analysis (#4) and for the trait (#2). The game keeps behaving the same; it just stops re-implementing line parsing.

### Step 2 — Unify engines behind an `Engine` trait  ·  high payoff / medium effort
```rust
pub trait Engine {
    async fn bestmove(&mut self, fen: &str, limits: Limits) -> Result<SearchInfo>;
}
```
- `InProcessNimzovich` — wraps `reply()` + the TT pool (**unchanged behaviour**).
- `UciSubprocess` — wraps the `uci-client` from #1; backs Stockfish, `nimzovich-uci`, Leela, etc.

Collapses the two parallel `spawn_*` paths into one. Difficulty, analysis, and matchmaking all flow through one interface; new engines are a config choice. **Trade-off to honour:** Nimzovich stays in-process — the trait is the boundary for swapping engines, not a mandate to pipe our own engine.

### Step 3 — Make `nimzovich-uci` a full UCI engine  ·  medium payoff / medium effort
Close the §2 gaps:
- **Threaded, interruptible search** for real `stop` / `go infinite` / ponder (`ponderhit`) — search on a worker thread with an atomic stop flag.
- **`score mate N`** reporting.
- **Full PV** from the TT or a triangular PV table.
- **Per-iteration `info`** streaming (depth/seldepth/score/nodes/nps/hashfull).
- **More `setoption`**: `Threads`, `MultiPV`, `Move Overhead`, `Ponder`, `Clear Hash`, optionally `UCI_Chess960`.
- **`movestogo` + `go nodes`** in budgeting.

**Unlocks:** OpenBench / cutechess **SPRT** strength tuning, plugging into **lichess-bot** to run Nimzovich online, and any chess GUI (Arena, Cute Chess, BanksiaGUI).

### Step 4 — Backend UCI consumers  ·  high product payoff / larger effort
A UCI engine pool on the backend (using #1) powers features with existing placeholders:
- **Anti-cheat** (`crates/shared/xfchess-anticheat`) — centipawn-loss / engine-correlation scoring: evaluate player moves vs. engine bestmove. A natural UCI consumer.
- **Puzzle generation** (`docs/PUZZLES.md`) — scan finished games for eval swings to mine tactical positions; one `go` per candidate.
- **In-game analysis** — eval bar, best-move hints, post-game blunder review, driven through the shared client.
- **Bot opponents / matchmaking fallback** — serve a UCI engine when no human is available.

---

## 4. Recommended order

1. **Step 1 (`uci-client` crate)** — structural unlock; removes the lossy duplicated client. Do this first.
2. **Step 2 (`Engine` trait)** — one interface, two backends; keep Nimzovich in-process.
3. **Step 3 (`nimzovich-uci` completeness)** — turns the unused server into a testable, online-capable engine (SPRT, lichess-bot, GUIs).
4. **Step 4 (backend consumers)** — anti-cheat, puzzles, analysis, bots, all cheap once #1–#2 land.

## 5. Constraints to respect (from [crates/CLAUDE.md](../crates/CLAUDE.md))
- `nimzovich_engine` has two personalities: `["std", "search"]` for full alpha-beta vs. `no_std` move-gen on-chain. **UCI work lives only on the `std`/search side** — never pull UCI/process/IO deps into the no_std path used by `chess-logic-on-chain` and the Solana program.
- Keep the in-process Nimzovich TT-pool optimization intact when introducing the `Engine` trait.
</content>
