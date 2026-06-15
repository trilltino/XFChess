# Workspace Crates — Deep Audit & Remediation Plan

> **Date:** 2026-06-15
> **Scope:** The entire repository — every crate under `crates/` (engine, shared, solana,
> zarathustra_net) **and** the application components: game client (`src/`), backend
> (`backend/`), Solana program (`programs/`), desktop wrapper (`tauri/`), web frontend
> (`web-solana/`).
> **Goal:** Find security issues, bugs, dead/unused code, mis-named types, non-idiomatic
> Rust, and unsafe `unwrap`/`panic` usage; specify the most efficient, idiomatic fix for each.
> **Structure:** **Part I** (§1–§8) audits the `crates/`. **Part II** (§A1–§A6) cross-
> references the workspace against the six-dimension Systems-Architect checklist (ownership ·
> concurrency/async · `unsafe` · API/errors · security/supply-chain · verification); **§A7**
> integrates the UCI/engine-boundary roadmap. **Part III** audits the application components
> (`src`/`backend`/`programs`/`tauri`/`web-solana`) — including a **deep on-chain smart-contract
> audit (§P)** — and contains the **🔴 P0 exploit chain**.
> §9 is the unified phased plan (incl. parallel **Phase E**); §10 the non-goals.

> ‼️ **Read Part III first.** The highest-severity findings in the whole audit are not in the
> crates — they're a Tauri + web XSS→native-code exploit chain (Part III, "P0 exploit chain").
> **Status of this document:** Plan only — no source changed yet. Companion to the
> already-completed [SOLANA_CRATES_AUDIT.md](SOLANA_CRATES_AUDIT.md) (the `solana/*` group).

---

## 0. How to read this plan

Each finding carries a **severity** and a **category**:

| Sev | Meaning |
|-----|---------|
| 🔴 P0 | Security risk, correctness bug, or panic on attacker/edge input — fix first |
| 🟠 P1 | Real bug-risk or large maintenance/build liability — fix this pass |
| 🟡 P2 | Idiomatic / efficiency / naming cleanup — fix opportunistically |
| ⚪ P3 | Nice-to-have, low value, document-and-defer |

Categories: **SEC** (security) · **BUG** · **DEAD** (dead/unused) · **NAME** (naming) ·
**IDIOM** (non-idiomatic) · **PERF** (efficiency) · **PANIC** (unwrap/expect/panic) · **DEP** (dependency hygiene).

---

## 0.5 Top of the whole audit — the 🔴 P0 (Part III)

Before the crate-level items below: the single most dangerous issue in the repository is a
**Tauri + web XSS → native-code-execution chain** — `script-src 'unsafe-inline'` +
`withGlobalTauri:true` + unsanitized `dangerouslySetInnerHTML` + a `shell:allow-execute`
PowerShell/`ssh root@` capability + an unvalidated `open_url`. Plus **secret API keys
(`VITE_HELIUS_API_KEY`, payment keys) bundled into the public web JS**. See Part III, "🔴 P0".
These outrank everything in Parts I–II.

## 1. Executive summary — the three things that matter (crates)

1. **🔴/🟠 The networking layer is ~10× larger than XFChess uses.**
   `braid-core` is **43,664 LOC**, but the entire XFChess consumer surface is **two types**:
   `braid_core::Update` and `braid_core::Version` (used only by `braid-iroh`). Of that 43k:
   - **35,036 LOC** is a vendored `diamond_types` CRDT + `rle` (`src/vendor/`) that **no
     XFChess consumer reaches** — `braid-iroh` only touches `Update`, the backend imports
     nothing from it.
   - **4,904 LOC** is a filesystem/NFS sync module (`src/fs/`) gated behind an `fs` feature
     that **no consumer enables**.
   - Only **~3,682 LOC** (`src/core/`) is the actual Braid protocol, and consumers use a
     sliver of it.
   This is the single highest-leverage item: pruning/quarantining ~40k LOC of unreachable
   code shrinks build time, attack surface, and audit burden enormously. See §5.

2. **🟠 `backend` declares `braid-core` but never uses it** ([backend/Cargo.toml:70](../backend/Cargo.toml#L70))
   — no `use braid_core` / `braid_core::` anywhere in `backend/src`. Candidate unused
   dependency (verify against re-exports before removing). See §4.

3. **🔴 Two concrete panics on bad/edge input** that aren't in test code:
   - Float `partial_cmp().unwrap()` on tournament tiebreaks
     ([swiss-pairing/standings.rs:35-37](../crates/shared/swiss-pairing/src/standings.rs#L35-L37))
     — any NaN Buchholz/Sonneborn score panics the standings sort. **BUG/PANIC.**
   - All-zeros placeholder peer key + `format!().parse().unwrap()` in the P2P node
     constructor ([braid-iroh/lib.rs:43-44](../crates/zarathustra_net/braid-iroh/src/lib.rs#L43-L44)).

Everything else is comparatively small polish, with one exception worth calling out:
**`nimzovich_engine` carries a whole dead evaluation module** (`evaluation/pst.rs`, the
tapered piece-square-table system — the engine actually evaluates via `pesto`) plus dead
search-tuning fields and ~30 clippy warnings. Real cleanup, not just style. See §6.1.

The remaining first-party crates (`shared`, `swiss-pairing`, `xfchess-anticheat`,
`braid_chess`, `braid-iroh`, `xfchess-braid-server`) are in good shape and need light work.

**Architecture-first highlights (Part II), beyond the per-crate list:**
- 🟠 **One genuine soundness risk:** unjustified `unsafe impl<F> Send for SendFuture<F>` +
  6 manual `unsafe` pin projections in `braid-http` with **zero SAFETY comments** (A3.2).
- 🟠 **Zero-copy claim not honoured:** the engine's on-chain board uses raw `transmute`
  (one missing its SAFETY note) where its own comment says `bytemuck` — and `bytemuck`
  isn't even a dependency (A1.3/A3.1). Mechanical, safe fix.
- 🟠 **Trust-boundary input is unbounded:** `game_id` in the Braid resource parser has no
  length/format limit and becomes a relay map key — OOM vector (A5.1).
- 🟠 **Libraries leak `anyhow`/`Box<dyn Error>`** in public APIs (`braid-iroh`, `braid-core`,
  `iroh-h3-client`) — callers can't match errors (A4.1).
- 🟠 **Supply-chain gates missing:** no `cargo-deny`/`audit`/`semver-checks`/property tests,
  and no `[workspace.lints]` (A5.2/A6.1/A6.2/A6.3). CI's clippy/perft/differential gate is
  otherwise strong.
- 🟠 **Engine boundary is the biggest *architecture* defect (§A7):** the only UCI client is
  lossy and inlined into a Bevy system, there's no `Engine` trait, and `nimzovich-uci`'s
  `stop` is a no-op (uninterruptible search). [UCI_INTEGRATION.md](UCI_INTEGRATION.md) is the
  remediation — extract `uci-client`, add the trait, complete the server — and it clears
  A1.4 (I/O coupling), feeds A4.1/A4.3 (errors/typestate) and A2.4 (cancellation), and kills
  the anti-cheat Stockfish duplication.
- ✅ **Done right (mirror these):** the anticheat bounded work-queue + `spawn_blocking`, and
  clean crypto (no MD5/SHA-1) — see Part II preamble.

---

## 2. Methodology / reproduce

```bash
# LOC per crate
for d in crates/*/*; do [ -d "$d/src" ] && echo "$(find $d/src -name '*.rs'|xargs wc -l|tail -1|awk '{print $1}')  $d"; done | sort -rn

# Panic-surface census
grep -rEc "\.unwrap\(\)|\.expect\(|panic!|todo!|unimplemented!|unreachable!" crates/*/*/src

# Reachability of an internal module (example: braid-core fs/vendor)
grep -rn "braid_core::fs\|braid_core::vendor" --include=*.rs crates backend src   # → no hits = unreachable

# Compiler/lint signal (run as Phase 0 of execution — see §7)
cargo clippy --workspace --all-targets -- -W clippy::pedantic -W clippy::nursery
cargo +nightly udeps --workspace        # unused dependency detection
cargo machete                            # lighter unused-dep heuristic
```

This audit combined the LOC/panic census above, reachability tracing for the big modules,
and targeted reads of the hot paths. **A full `cargo clippy --workspace` + `cargo udeps`
run is Phase 0 of execution** and will mechanically surface the long tail of P2 IDIOM/DEAD
items this document does not enumerate by hand.

---

## 3. Crate taxonomy — audit each tier differently

The most important framing decision: **do not refactor vendored forks for style.** Renaming
types or "making it idiomatic" inside an upstream fork destroys the ability to rebase on
upstream and adds zero value. Forks get a *different* audit: integration correctness,
dead-code pruning, and dependency pinning only.

### Tier A — First-party, load-bearing (full deep audit: SEC/BUG/NAME/IDIOM/PERF/PANIC)
| Crate | LOC | Notes |
|-------|-----|-------|
| `nimzovich_engine` | 8,469 | Chess AI; perf-critical; dual `std`/`no_std` |
| `chess-logic-on-chain` | 63 | Thin no_std wrapper — already audited (Solana doc §1) |
| `shared` | 353 | Bevy+serde shared types |
| `backend-types` | 30 | Serde-only DTOs |
| `swiss-pairing` | 1,421 | FIDE Dutch Swiss pairing |
| `xfchess-anticheat` | 2,135 | Anti-cheat feature extraction + Stockfish |
| `braid_chess` | 895 | Game-facing Braid wrapper (was `braid_uri`) |
| `braid-iroh` | 1,316 | Iroh QUIC transport glue |
| `xfchess-braid-server` | 593 | Axum HTTP-209 subscribe integration |
| `nimzovich-uci` | 312 | UCI adapter binary |

### Tier B — First-party but Solana group (already audited — see [SOLANA_CRATES_AUDIT.md](SOLANA_CRATES_AUDIT.md))
`solana-chess-client` (536), `er-cu-benchmark` (3,006). Carry that doc's open follow-ups
into §7 rather than re-auditing here.

### Tier C — Vendored forks (audit: DEAD-pruning + DEP-pinning + integration only; **no style refactor**)
| Crate | LOC | Provenance |
|-------|-----|-----------|
| `braid-core` | 43,664 | First-party shell wrapping **35k LOC vendored** `diamond_types`/`rle` (`src/vendor/`) |
| `braid-http` | 4,612 | Braid-HTTP client (largely derived) |
| `iroh-gossip` | 8,026 | **Explicit fork** of `n0-computer/iroh-gossip` (README says so) |
| `iroh-h3` / `iroh-h3-client` / `iroh-h3-axum` | 601 / 3,203 / 218 | HTTP/3-over-iroh, fork-derived |

---

## 4. Workspace-level findings

| # | Sev | Cat | Finding | Action |
|---|-----|-----|---------|--------|
| W1 | 🟠 P1 | DEP | `backend` depends on `braid-core` ([backend/Cargo.toml:70](../backend/Cargo.toml#L70)) with **no `braid_core::` usage** in `backend/src`. | Run `cargo udeps`/`cargo machete`; if confirmed unused, remove. Saves the whole 43k-LOC compile from the backend build graph. |
| W2 | 🟠 P1 | DEAD | `braid-core` pulls **35k LOC vendored CRDT + 4.9k LOC `fs/`** that no consumer reaches. | See §5 — feature-gate hard or split into a separate excluded crate. |
| W3 | 🟡 P2 | DEP | Several crates pin deps directly instead of via `workspace.dependencies` (called out for `er-cu-benchmark`/`solana-chess-client` in the Solana doc; re-verify for `braid-*`). | Move shared deps to the workspace table to stop version drift; same rationale as [crates/CLAUDE.md](../crates/CLAUDE.md) §"Adding a new shared crate". |
| W4 | 🟡 P2 | DEP | `iroh-gossip` declares `crate-type = ["cdylib", "rlib"]`. A `cdylib` is only needed if something loads it as a C dynamic lib — XFChess links it as a normal Rust dep. | Confirm nothing needs the `cdylib`; drop it to halve that crate's link/codegen cost. |
| W5 | 🟢 P3 | DEP | Some Tier-C forks carry `publish = false` already; ensure all four do (they must never hit crates.io given fork/AGPL constraints). | Audit `publish` flags. |
| W6 | 🟡 P2 | DEP | clippy warns: "profiles for the non-root package will be ignored" (≥2 crates declare `[profile.*]` that Cargo silently ignores outside the workspace root). | Move those `[profile]` blocks to the root [Cargo.toml](../Cargo.toml) or delete them — they currently do nothing. |
| W7 | ⚪ P3 | DEP | clippy warns `backend/src/signing_server.rs` is present in multiple build targets. | Backend (out of `crates/` scope) but cheap: declare the bin/lib targets explicitly so the file isn't double-compiled. |

---

## 5. The braid-core problem (highest leverage) — detailed plan

**Reachability proven:** the only symbols any XFChess crate imports from `braid-core` are
`Update` (used in [braid-iroh/node.rs](../crates/zarathustra_net/braid-iroh/src/node.rs),
[subscription.rs](../crates/zarathustra_net/braid-iroh/src/subscription.rs)) and `Version`
(used in [braid-iroh/protocol.rs](../crates/zarathustra_net/braid-iroh/src/protocol.rs)).
`braid_chess` talks to `braid_http` directly, not `braid-core`. Backend uses neither (W1).

**Composition of the 43,664 LOC:**

| Sub-tree | LOC | Reachable by XFChess? | Compiled today? |
|----------|----:|-----------------------|-----------------|
| `src/vendor/` (`diamond_types`, `rle`) | 35,036 | ❌ no (CRDT merge path unused) | ✅ yes (`pub mod vendor`, always on) |
| `src/fs/` (NFS/mount/watcher sync) | 4,904 | ❌ no | ❌ no (`fs` feature never enabled) |
| `src/core/` (Braid protocol) | 3,682 | ⚠️ partial (`Update`/`Version` + a little) | ✅ yes |
| blob (`braid_blob`) | — | ❌ no (`blob` feature never enabled) | ❌ no |

**Decision tree (pick one — recommend B):**

- **Option A — minimal, safe now:** Put `pub mod vendor` behind a `crdt`/`merge` feature
  (default-off) exactly like `fs`/`blob` already are. Verify `braid-iroh` + backend still
  build (they should — they only need the `Update`/`Version` types, which live in `core`,
  not `vendor`). This removes 35k LOC from the default compile immediately. **Risk:** low if
  `core` doesn't reference `vendor` unconditionally — *must verify the `core::merge`
  re-export* ([lib.rs:37-38](../crates/zarathustra_net/braid-core/src/lib.rs#L37-L38)) is
  also gated.
- **Option B — recommended:** Extract the genuinely-used surface (`Update`, `Version`, and
  whatever `braid-iroh` needs) into the slim crate, and move `vendor/` (diamond_types) into
  its own **excluded** workspace member `crates/zarathustra_net/diamond-types-vendor/` that
  is only built if/when the CRDT path is actually wired up. Quarantines upstream code so it
  can't rot the audit surface, and makes "is the CRDT used?" answerable by grep.
- **Option C — most aggressive:** If the CRDT merge path is genuinely never going to be used
  (chess moves are totally ordered per game via on-chain nonce — a sequence CRDT is arguably
  the wrong tool), delete `vendor/` + `fs/` + blob entirely. **Do not do this without
  confirming** no roadmap item (e.g. live collaborative analysis boards / chat) depends on
  CRDT merge. Record the decision either way.

**Sequencing:** Do W1 (drop backend dep if unused) → Option A (feature-gate, prove builds)
→ then decide B vs C in a follow-up once the merge path's future is confirmed with the owner.

---

## 6. Per-crate findings (Tier A)

### 6.1 `nimzovich_engine` — 🟠 dead module + ~30 clippy warnings
The strongest first-party crate by design, but **not** clippy-clean (default lints, verified
this pass). `no_std` discipline does hold (the `use std::` hits in `pgn.rs`, `book.rs`,
`search/iterative.rs`, `types.rs` are in `std`/`search`-feature paths). Panic census is good
— **15 hits, nearly all in `#[cfg(test)]`/doc-comments**.

| # | Sev | Cat | Location | Action |
|---|-----|-----|----------|--------|
| E1 | 🟠 P1 | DEAD | [evaluation/pst.rs](../crates/engine/nimzovich_engine/src/evaluation/pst.rs) — `PAWN_PST`…`KING_PST_ENDGAME` (13 consts) + `get_pst_value_tapered` all "never used" | Entire tapered piece-square-table evaluator is dead; the engine evaluates via `pesto`/`position`. Delete `pst.rs` and `mod pst;` ([evaluation/mod.rs:18](../crates/engine/nimzovich_engine/src/evaluation/mod.rs#L18)) — ~123 LOC, after confirming no `--features` path re-enables it. |
| E2 | 🟠 P1 | DEAD | [constants.rs:231-342](../crates/engine/nimzovich_engine/src/constants.rs#L231) — `MAX_PHASE`, `PHASE_VALUES`, `CORE_BIT_BUFFER_SIZE`, `BIT_BUFFER_SIZE`, `bit_buffer_size()` never used; [search/params.rs:14](../crates/engine/nimzovich_engine/src/search/params.rs#L14) — `lmr_quiet_mul`, `lmr_quiet_base`, `see_depth`, `see_quiet_margin`, `see_nonquiet_margin` never read | Dead tuning constants/fields. Remove, or wire them into the search if they were meant to be live (the `see_*`/`lmr_*` names suggest abandoned search tuning — confirm intent before deleting). |
| E3 | 🟡 P2 | BUG | [pgn.rs:675](../crates/engine/nimzovich_engine/src/pgn.rs#L675) `in_tag` assigned but never read; [pgn.rs:664](../crates/engine/nimzovich_engine/src/pgn.rs#L664) `RawToken` field `0` never read; [tables.rs:116](../crates/engine/nimzovich_engine/src/move_gen/tables.rs#L116) unused `col` | The PGN lexer's `in_tag` flag is vestigial (set true→false within one match arm; the `!in_tag` guard is always true). Not a correctness bug today, but remove the dead state machine to avoid confusion. Drop unused `col`. |
| E4 | 🟡 P2 | PANIC | [pgn.rs:478](../crates/engine/nimzovich_engine/src/pgn.rs#L478) `chars().last().unwrap()`, [pgn.rs:699](../crates/engine/nimzovich_engine/src/pgn.rs#L699) `chars.next().unwrap()` | PGN parsing runs on imported (untrusted) games. Return the crate's parse error instead of panicking on malformed input. |
| E5 | 🟡 P2 | IDIOM | ~20 `casting to the same type is unnecessary (i8→i8)` across [move_gen/tables.rs](../crates/engine/nimzovich_engine/src/move_gen/tables.rs) + [search/alphabeta.rs](../crates/engine/nimzovich_engine/src/search/alphabeta.rs); manual `Range::contains` in [board.rs:26-33](../crates/engine/nimzovich_engine/src/board.rs#L26) / [api/moves.rs:19](../crates/engine/nimzovich_engine/src/api/moves.rs#L19); `i8::abs() as usize` casts ([material.rs:16](../crates/engine/nimzovich_engine/src/evaluation/material.rs#L16), [hash.rs:71](../crates/engine/nimzovich_engine/src/hash.rs#L71)); empty line after doc comment | Mostly `cargo clippy --fix` auto-fixable. The `i8::abs() as usize` casts are worth a manual look (abs of `i8::MIN` overflows — verify ranges). |
| E6 | 🟡 P2 | PERF | [alphabeta.rs:109](../crates/engine/nimzovich_engine/src/search/alphabeta.rs#L109) `iter().any()` → `contains()`; [alphabeta.rs:57](../crates/engine/nimzovich_engine/src/search/alphabeta.rs#L57) function takes 9 args (>7) | Apply the `contains` lint (hot search path). Consider bundling the 9 search args into a `SearchContext` struct for clarity + fewer stack moves. |
| E7 | 🟡 P2 | PERF | `move_gen/`, `see.rs` | Profile hot loops; confirm move lists use `SmallVec`/arrayvec (chess move count ≤ ~218) rather than heap `Vec`. Quantify before changing. |

### 6.2 `swiss-pairing` — 🔴 one real bug, otherwise clean
| # | Sev | Cat | Location | Action |
|---|-----|-----|----------|--------|
| S1 | 🔴 P0 | BUG/PANIC | [standings.rs:35-37](../crates/shared/swiss-pairing/src/standings.rs#L35-L37) `partial_cmp(&...).unwrap()` ×3 on `score`/`buchholz`/`sonneborn` (`f*`) | NaN tiebreak → panic mid-sort (sort can also leave inconsistent order). Switch float compares to `f64::total_cmp` (or `partial_cmp(...).unwrap_or(Ordering::Equal)`). This is a real-money tournament path. |
| S2 | 🟡 P2 | PANIC | [pairing.rs:68](../crates/shared/swiss-pairing/src/pairing.rs#L68) `.unwrap()` | Confirm invariant holds for all inputs (incl. odd player counts / all-byes); convert to typed error if reachable. |
| S3 | ⚪ P3 | NAME | `types.rs`, `color.rs` | Verify FIDE-domain names (`buchholz`, `sonneborn`) match spec spelling; ensure `Color` doesn't shadow chess piece color elsewhere — disambiguate if it does. |

### 6.3 `xfchess-anticheat` — 🟢 clean (1 test-only unwrap)
| # | Sev | Cat | Location | Action |
|---|-----|-----|----------|--------|
| A1 | 🟠 P1 | SEC | [engine/stockfish.rs](../crates/shared/xfchess-anticheat/src/engine/stockfish.rs) | Anti-cheat shells out to Stockfish. Verify: engine path isn't attacker-controllable, UCI input from games is sanitized before being fed to the process, and process spawn failures degrade gracefully (no panic, no false "cheat" verdict). |
| A2 | 🟡 P2 | BUG | [features/timing.rs](../crates/shared/xfchess-anticheat/src/features/timing.rs), [features/screen.rs](../crates/shared/xfchess-anticheat/src/features/screen.rs) | Review for integer/float division-by-zero (empty game → zero moves) in feature extraction; these feed cheat scores. |
| A3 | ⚪ P3 | NAME | `cross_game/`, `ingest.rs` | Ensure feature-vector field names are self-documenting (they drive a model/threshold). |

### 6.4 `shared` + `backend-types` — 🟢 clean
| # | Sev | Cat | Location | Action |
|---|-----|-----|----------|--------|
| H1 | 🟡 P2 | IDIOM | [shared/src/protocol.rs](../crates/shared/shared/src/protocol.rs) | `bincode::serialize().expect()` / `panic!` are all in `#[cfg(test)]` — fine. Confirm the *runtime* protocol decode path returns `Result` (network input must never `unwrap`). |
| H2 | ⚪ P3 | NAME | both crates | Check `LobbyMessage` variants and DTO field names are consistent between `shared` (Bevy side) and `backend-types` (wire side) so the JSON contract is obvious. |

### 6.5 `braid_chess` (was `braid_uri`) — 🟢 clean
Recently renamed (`braid_uri → braid_chess`, `uri.rs → resource.rs`). All `unwrap`s are in
tests/doc-comments. Action: confirm the rename is complete — no lingering `braid_uri` /
`Uri` naming in identifiers or docs ([NAME]); spot-check [message.rs](../crates/zarathustra_net/braid_chess/src/message.rs)
serde round-trips have non-panicking runtime paths.

### 6.6 `braid-iroh` — 🟠 transport hygiene
| # | Sev | Cat | Location | Action |
|---|-----|-----|----------|--------|
| I1 | 🟠 P1 | PANIC | [lib.rs:43-44](../crates/zarathustra_net/braid-iroh/src/lib.rs#L43-L44) `format!("127.0.0.1:{port}").parse().unwrap()` + `EndpointId::from_bytes(&[0u8;32]).expect("placeholder")` | The all-zeros placeholder peer is a footgun (could be dialed). Make `default_peer` an `Option`, or construct the `SocketAddr` without string parsing (`SocketAddr::from(([127,0,0,1], port))` — no `unwrap`, no alloc). |
| I2 | 🟡 P2 | PANIC | [discovery.rs:62,91,96](../crates/zarathustra_net/braid-iroh/src/discovery.rs#L62) `RwLock::{read,write}().unwrap()` | Lock-poisoning panics propagate through the P2P relay. Either adopt `parking_lot` (no poisoning, already in the dep graph via braid-core) or centralize on a helper that handles poison. Low individual risk, but it's the relay. |
| I3 | 🟡 P2 | IDIOM | [protocol.rs:84](../crates/zarathustra_net/braid-iroh/src/protocol.rs#L84) `StatusCode::from_u16(209).unwrap()` | Hoist to a `const HTTP_209: StatusCode` (or `http::StatusCode::from_u16` once) — 209 is the Braid status, used repeatedly. |

### 6.7 `xfchess-braid-server` — 🟢 clean
Single non-test `.unwrap()` at [resource/subscribe.rs:120](../crates/zarathustra_net/xfchess-braid-server/src/resource/subscribe.rs#L120) — read it in context; if it's response-builder boilerplate on a constant it's fine, otherwise convert to a `500` mapping. This crate handles untrusted subscribe requests, so verify the resource-path parser rejects malformed/oversized paths.

### 6.8 `nimzovich-uci` — 🟢 thin adapter, but incomplete as an engine
Small binary, zero panic hits. Its `println!`/`eprintln!` are **correct** — UCI speaks over
stdout, so this is not an observability finding. Two action sets:
- **Audit hygiene:** confirm UCI stdin parsing can't panic on malformed GUI input; clippy pass.
- **Completeness (architecture):** `stop` is a no-op, no `go infinite`/ponder, `score cp` only
  (never `mate`), single-move PV, `movestogo` ignored. These are the **§A7** engine-boundary
  items (A7.4/A7.5), not just polish — they gate SPRT testing and lichess-bot use. See
  [UCI_INTEGRATION.md](UCI_INTEGRATION.md).

---

## 7. Tier C (vendored forks) — narrow scope

**No style refactors. No renames.** For each of `braid-core/vendor`, `braid-http`,
`iroh-gossip`, `iroh-h3*`:

1. **Pin upstream provenance** — record the upstream commit/version each fork is based on in
   its `README.md` (only `iroh-gossip` does this today). Without it, no one can ever rebase
   security fixes from upstream. **🟠 P1 / SEC** (forks silently miss upstream CVE patches).
2. **Dead-code prune** — apply §5 to `braid-core`; for the `iroh-*` forks, delete modules/
   features XFChess doesn't use (e.g. server/relay code paths not exercised by the relay).
3. **Dependency pinning** — move to `workspace.dependencies` where the version must match the
   non-forked `iroh` stack to avoid two `iroh` versions in the graph.
4. **`cargo audit`** — run `cargo audit` against the whole graph; forks are the likeliest
   source of known-vuln transitive deps.

---

## 8. Cross-cutting policies to adopt

- **Unwrap policy (PANIC):** Establish the rule — `unwrap`/`expect` allowed only in
  `#[cfg(test)]`, build scripts, `Lazy`/`OnceLock` static init of compile-time-constant data
  (e.g. regexes, attack tables), and `main`. Everywhere else returns `Result`. Most of the
  census already complies; the exceptions are S1, I1, E4, I2. Consider adding
  `#![warn(clippy::unwrap_used, clippy::expect_used)]` to Tier-A `lib.rs` files (with
  `#[cfg_attr(test, allow(...))]`).
- **Float ordering:** ban `partial_cmp().unwrap()` workspace-wide; use `total_cmp`. (S1.)
- **Idiomatic pass (IDIOM):** one `cargo clippy --workspace -- -W clippy::pedantic` sweep,
  triaged per-crate; auto-fixable with `cargo clippy --fix` on Tier A only.
- **Naming (NAME):** the recent `braid_uri → braid_chess` / `networking → zarathustra_net`
  renames are good. Remaining: verify no half-renamed identifiers, and that domain types
  (`Color`, `Update`, `Version`, `Resource`) don't collide across crates.
- **Dead/unused (DEAD/DEP):** `cargo machete` + `cargo +nightly udeps` as a CI gate.

---

# Part II — Architecture-first audit (Systems Architect checklist)

> Cross-references the entire workspace against the six-dimension architect checklist
> (ownership, concurrency, `unsafe`, API/errors, security/supply-chain, verification).
> Findings here are tagged `Axx` and feed the same phases (§9). Where Part I already
> covers an item, it's linked rather than repeated.

### What the codebase already gets *right* (don't "fix" these)
- **Bounded work queue done correctly** — [xfchess-anticheat/job_queue.rs](../crates/shared/xfchess-anticheat/src/engine/job_queue.rs):
  bounded `mpsc::channel(cfg.queue_capacity)`, `try_send → AcError::QueueFull` (backpressure,
  not unbounded growth), a fixed worker pool, and a `depth()` gauge. This is the §2 standard.
- **CPU offload done correctly** — [xfchess-anticheat/lib.rs:42](../crates/shared/xfchess-anticheat/src/lib.rs#L42)
  runs Stockfish eval under `tokio::task::spawn_blocking` (keeps the executor unblocked, §2).
- **Crypto hygiene** — **zero** MD5/SHA-1/DES/RC4 anywhere; the graph uses `sha2`, `blake3`,
  `rand_chacha`, and Solana's `ed25519`. §5 crypto standard already met.
- **CI lint gate exists** — [.github/workflows/ci.yml](../.github/workflows/ci.yml) runs
  `cargo check --all-features`, `cargo fmt --check`, **`cargo clippy --workspace
  --all-features -- -D warnings`**, the full test suite, deep perft, a differential-perft
  vs shakmaty, and an engine match-sanity job. That's a strong §6 baseline.

> ⚠️ **Reconcile first:** CI runs clippy with `--all-features -D warnings`, yet the ~30
> warnings in `nimzovich_engine` (E1–E6) appeared under the **default** feature set. Either
> CI is currently red, or that dead code is only dead in the default build and a feature
> re-enables it (e.g. `pst.rs` behind a not-yet-wired eval feature). Resolve this before
> deleting E1/E2 — it changes whether they're "dead" or "feature-gated".

## A1. Architecture & Data Ownership

| # | Sev | Cat | Finding | Standard / action |
|---|-----|-----|---------|-------------------|
| A1.1 | 🟡 P2 | PERF/IDIOM | `.clone()` density in first-party hot/relay code: `swiss-pairing` 27, `braid-iroh` 26, `xfchess-anticheat` 21 (non-test). (braid-core's 291 is mostly vendored/dead — ignore.) | Triage each: many are `String`/`Vec` clones that could be `&str`/`&[_]` or `Arc` shares. Not blanket-remove — clone-to-satisfy-borrowck is the AI smell to hunt. Start with `swiss-pairing` (per-round player vectors). |
| A1.2 | 🟢 P3 | ARCH | Shared mutable state is modest in first-party code (`braid-iroh` 3, `xfchess-braid-server` 2 `Arc<RwLock>`); the relay's `RwLock<HashMap>` peer/resource tables are a reasonable use. | Keep, but see A2.2 (poisoning). No actor-model rewrite warranted — state is genuinely shared session state, justified. |
| A1.3 | 🟠 P1 | PERF/SEC | **Zero-copy claim not honoured in the engine's on-chain path.** [on_chain.rs:42-84](../crates/engine/nimzovich_engine/src/on_chain.rs#L42) comments "repr(C) so bytemuck can cast it with zero cost" but then uses raw `core::mem::transmute` and the engine has **no `bytemuck` dependency**. | Add `bytemuck`, derive `Pod`/`Zeroable` on `CompactBoard`, and use `bytemuck::from_bytes`/`bytes_of`. Eliminates both `unsafe` blocks (A3.1), enforces "no padding" at compile time, and is genuinely zero-copy. This is the §1 zero-copy + §3 unsafe standard in one fix. |
| A1.4 | 🟠 P1 | ARCH | **I/O coupling violation — the engine boundary.** Core libs are pure (`chess-logic-on-chain`/`nimzovich_engine` no_std), but the **only UCI client is hand-inlined into game business logic** ([src/game/ai/systems.rs:382-471](../src/game/ai/systems.rs#L382)) — a Stockfish subprocess + line parser baked into a Bevy system, and a *lossy* one (drops `mate`, full `pv`, `nps`, `multipv`). There is no `Engine` trait; Nimzovich and Stockfish are two parallel `spawn_*` paths. | This is exactly the checklist's "abstract external deps behind a trait for deterministic testing." Remediation is already specified in [UCI_INTEGRATION.md](UCI_INTEGRATION.md): extract a `crates/engine/uci-client` crate + an `Engine` trait. Tracked in full as **§A7**. |
| A1.5 | ⚪ P3 | ARCH | `braid-iroh`/`xfchess-braid-server` keep transport reasonably separated; `xfchess-anticheat` abstracts engine work behind a job queue. | Spot-check only; no major violation. The engine boundary (A1.4) is the one real coupling defect. |

## A2. Concurrency & Async Guarantees

| # | Sev | Cat | Finding | Standard / action |
|---|-----|-----|---------|-------------------|
| A2.1 | 🟠 P1 | PERF | 35 `tokio::spawn` sites but only 5 backpressure constructs (`spawn_blocking`/`buffer_unordered`/`Semaphore`). Most spawns are in **dead** `braid-core/fs` (§5) or vendored forks; first-party unbounded-spawn risks are `braid-iroh/node.rs:180,258` (per-event save tasks). | For `braid-iroh` snapshot-save spawns ([node.rs:258](../crates/zarathustra_net/braid-iroh/src/node.rs#L258)): a burst of resource updates spawns unbounded save tasks racing on the same dir. Serialize via a single writer task + channel, or debounce. Pruning `fs/` (§5) removes most of the count. |
| A2.2 | 🟡 P2 | BUG | **Sync locks across `.await`** — `xfchess-braid-server` uses `parking_lot::RwLock` ([hub.rs:13](../crates/zarathustra_net/xfchess-braid-server/src/hub.rs#L13), [store.rs:13](../crates/zarathustra_net/xfchess-braid-server/src/resource/store.rs#L13)) in an async server. parking_lot guards are **not** `Send` and must never be held across `await`. | Audit each guard's lifetime: ensure the lock is dropped before any `.await` (clone/copy the needed data out, then drop). If a guard must span an await, switch that one to `tokio::sync::RwLock`. The anticheat queue already models the correct pattern (lock only around `recv`). |
| A2.3 | 🟡 P2 | PERF | The anticheat worker pool shares one receiver via `Arc<tokio::sync::Mutex<mpsc::Receiver>>` ([job_queue.rs](../crates/shared/xfchess-anticheat/src/engine/job_queue.rs)) — correct (tokio Mutex is await-safe) but workers contend on one lock per `recv`, serializing dequeue. | Low priority. If worker count grows, switch to an MPMC channel (`async-channel`) so workers pull without a shared mutex. Fine as-is for small pools. |
| A2.4 | 🟠 P1 | BUG | **Cancellation safety unverified** at `select!` sites in first-party/derived async: [braid-http/subscription.rs:127,144](../crates/zarathustra_net/braid-http/src/client/subscription.rs#L127). A subscription stream dropped mid-`select!` must not lose a buffered move/patch. | Verify each branch is cancellation-safe (no partial read left in a non-restartable future). The Braid subscription is the live board-sync path — a lost patch desyncs the board. Add a test that drops the future mid-stream. |
| A2.5 | 🟢 P3 | ARCH | `select!` in vendored `iroh-gossip`/`braid-core/fs` — Tier C / dead. | Out of scope (don't refactor forks; `fs` is dead). |

## A3. Memory Safety & the `unsafe` Boundary

| # | Sev | Cat | Finding | Standard / action |
|---|-----|-----|---------|-------------------|
| A3.1 | 🟠 P1 | SEC/IDIOM | `nimzovich_engine`: 2 `unsafe transmute` ([on_chain.rs:78,83](../crates/engine/nimzovich_engine/src/on_chain.rs#L78)); `from_bytes` has a `// SAFETY:` but `to_bytes` does **not**. `transmute` on a struct→`[u8;68]` is fragile (padding/repr drift). | Replace with `bytemuck` (A1.3) — removes the `unsafe` entirely and makes padding a compile error. If kept, add the missing SAFETY comment and a `const _: () = assert!(size_of::<CompactBoard>() == 68)` guard. |
| A3.2 | 🟠 P1 | SEC | `braid-http`: **7 `unsafe`, 0 SAFETY comments** — manual `Pin` projection (`Pin::new_unchecked`, `get_unchecked_mut`, `map_unchecked_mut`) plus an unjustified **`unsafe impl<F> Send for SendFuture<F>`** ([traits.rs:62](../crates/zarathustra_net/braid-http/src/client/traits.rs#L62), [subscription.rs:25-205](../crates/zarathustra_net/braid-http/src/client/subscription.rs#L25)). | The `unsafe impl Send` asserts thread-safety with no proof — a real soundness risk if `F` holds non-Send state. **Replace manual pin projection with `pin-project-lite`** (eliminates all the `unsafe` pin code) and either prove or delete the `Send` impl. Highest-value `unsafe` cleanup in the workspace. |
| A3.3 | 🟢 P3 | SEC | `braid-core` 19 `unsafe`/1 SAFETY — almost all in vendored `diamond_types` (Tier C); `MaybeUninit` hits are commented-out. No FFI `extern "C"` blocks anywhere; no `set_var`; the only `repr(C)` is the engine's `CompactBoard`. | Tier C: don't refactor. If §5 quarantines `vendor/`, this `unsafe` surface leaves the default build entirely. |
| A3.4 | 🟡 P2 | IDIOM | No crate sets `#![forbid(unsafe_code)]` even where it would hold (`swiss-pairing`, `shared`, `backend-types`, `braid_chess`, `chess-logic-on-chain`). | Add `#![forbid(unsafe_code)]` to the crates that contain none — makes "this crate is safe" a compiler-enforced invariant and catches AI-introduced `unsafe` in review. |

## A4. API Design & Error Handling

| # | Sev | Cat | Finding | Standard / action |
|---|-----|-----|---------|-------------------|
| A4.1 | 🟠 P1 | IDIOM | **Library crates leak `anyhow`/`Box<dyn Error>` in public APIs** (should be `thiserror` enums): `braid-iroh` `spawn_node() -> anyhow::Result<…>` ([lib.rs:30](../crates/zarathustra_net/braid-iroh/src/lib.rs#L30), 16 anyhow uses, 0 thiserror); `braid-core` (16 anyhow + 3 `Box<dyn Error>`); `iroh-h3-client` (6 `Box<dyn Error>`); `solana-chess-client` (3 anyhow). | Define exhaustive `thiserror` enums for the public surface of `braid-iroh`/`braid-core`. Callers (game client, backend) can't match on `anyhow` — they get stringly-typed errors. `braid_chess` already does this right (`BraidChessError`); mirror it. |
| A4.2 | 🟢 P3 | IDIOM | Good examples to mirror: `xfchess-anticheat` (`AcError`/`AcResult`), `braid_chess` (`BraidChessError`), `iroh-h3-client` partly (1 thiserror file). | Use these as the template for A4.1. |
| A4.3 | 🟡 P2 | IDIOM | **Typestate not used** for connection/session/game lifecycle — states tracked by enums/bools at runtime (e.g. Braid subscription open/closed, ER delegate/undelegate). | Where cheap, encode lifecycle in the type system (e.g. `Game<Delegated>` vs `Game<Mainnet>`, `Subscription<Open>`). High-value for the ER delegation lifecycle (CLAUDE.md flags undelegate-before-finalize as a footgun). Scope as a focused design task, not a blanket rewrite. |
| A4.4 | 🟡 P2 | PANIC | Public APIs returning `Result` but panicking internally undermine the contract — S1, I1, E4 (Part I). | Fold into Phase 1; an API that returns `Result` must not `unwrap` on input it received. |

## A5. Security & Supply Chain

| # | Sev | Cat | Finding | Standard / action |
|---|-----|-----|---------|-------------------|
| A5.1 | 🟠 P1 | SEC | **No length/format bound on `game_id` at the trust boundary** — [braid_chess/resource.rs:116](../crates/zarathustra_net/braid_chess/src/resource.rs#L116) `from_http_path` accepts any `game_id`, `.to_string()`s it, and it flows into the relay's `RwLock<HashMap>` keys (hub/store). An attacker can submit megabyte keys / unbounded distinct keys → memory exhaustion. | Validate `game_id` against the real format (short alphanumeric code, e.g. `^[A-Z0-9]{4,12}$`) and reject oversize/oversized paths **before** allocating. Same for `xfchess-braid-server` subscribe paths. §5 input-sanitization standard. |
| A5.2 | 🟠 P1 | SEC/DEP | **No `cargo-deny` / `deny.toml`** in the repo — no duplicate-version, license, or advisory gate. Forks (iroh-gossip, iroh-h3) risk pulling a second `iroh`/`quinn` version. | Add `deny.toml` + a CI `cargo deny check` job (advisories + bans + licenses; AGPL project needs license discipline anyway). |
| A5.3 | 🟡 P2 | DEP | `--no-default-features` discipline unverified — heavy deps (`tokio` "full" in braid-core's `native` feature, `regex`, `reqwest`) may pull more than needed. | Audit each `Cargo.toml`: turn off default features, enable only what's used (e.g. `tokio` with explicit feature list, not `full`). Pairs with §5 dead-code pruning. |
| A5.4 | 🟢 P3 | SEC | Crypto primitives are current (no MD5/SHA-1/DES). | No action — keep `cargo audit` (A6.2) watching for future drift. |

## A6. Verification & CI/CD Tooling

| # | Sev | Cat | Finding | Standard / action |
|---|-----|-----|---------|-------------------|
| A6.1 | 🟠 P1 | TEST | **No property-based testing / fuzzing** of the parsers & state machines (the AI-error-prone surfaces): PGN lexer ([pgn.rs](../crates/engine/nimzovich_engine/src/pgn.rs)), `from_http_path` resource parser, FEN parsing, Swiss pairing. (The old devnet fuzzer was deleted — see Solana audit §4.) | Add `proptest` for: PGN round-trip (parse∘format), resource path parse (never panics, bounded), Swiss pairing invariants (no player paired twice, color balance). Optionally `cargo-fuzz` targets for the parsers. Wire into CI. |
| A6.2 | 🟠 P1 | DEP | CI has no `cargo audit`, `cargo deny`, `cargo machete`/`udeps`, or `cargo semver-checks`. | Add CI jobs: `cargo audit` (advisories), `cargo deny check` (A5.2), `cargo machete` (catches W1/W2 dead deps), and `cargo semver-checks` for the published-shape crates. |
| A6.3 | 🟡 P2 | IDIOM | **No `[workspace.lints]` table.** CI enforces `clippy -D warnings` but per-crate lint posture is ad-hoc (only the iroh forks set `#![deny(missing_docs)]`, inherited from upstream). | Add a root `[workspace.lints]` table (`clippy::all` + chosen `pedantic` lints, `unwrap_used`/`expect_used` = warn, `missing_docs` = warn for libs) and `lints.workspace = true` in each first-party crate. Centralizes the §6 linting-limits standard; pedantic should be opt-in per-crate to avoid noise. |
| A6.4 | 🟡 P2 | TEST | Resolve the clippy/`--all-features` discrepancy (see ⚠️ above) so the CI gate's "0 warnings" claim is true under the default build too. | Run clippy under both `--all-features` and default; make both clean. |

## A7. Engine boundary & UCI integration

> Integrates [UCI_INTEGRATION.md](UCI_INTEGRATION.md) into this audit. The UCI roadmap is not a
> separate feature effort — it **is** the remediation for the workspace's single biggest
> architecture defect (A1.4) and it touches five checklist dimensions at once. It also adds a
> new crate (`crates/engine/uci-client`), so it belongs in the crates audit.

**Why it's load-bearing for this audit:** today UCI lives in two disconnected places — a
minimal **server** binary (`nimzovich-uci`, consumed by *nothing* in the product) and a lossy
**client** inlined into `src/game/ai/systems.rs` (Stockfish only). Neither is shared, the
client throws away `score mate`/`pv`/`nps`/`multipv`, and there is no engine abstraction. That
is simultaneously an I/O-coupling defect, an error-handling gap, a typestate gap, a
cancellation-safety gap, and a verification gap.

| # | Sev | Cat | Finding (UCI) | Standard / action |
|---|-----|-----|---------------|-------------------|
| A7.1 | 🟠 P1 | ARCH | The lossy Stockfish UCI client is inlined into a Bevy system ([systems.rs:382](../src/game/ai/systems.rs#L382)); no `Engine` trait; two parallel `spawn_*` AI paths. | **Step 1** of the roadmap: extract `crates/engine/uci-client` — a `UciEngine` owning the child + handshake, `async fn bestmove(fen, Limits) -> Result<SearchInfo>`, and a *complete* `SearchInfo` (`bestmove`, `ponder`, `Score::{Cp,Mate}`, `depth`, `seldepth`, `nodes`, `nps`, `hashfull`, `pv`, `multipv`). Then **Step 2**: an `Engine` trait with `InProcessNimzovich` + `UciSubprocess` backends. Satisfies A1.4. |
| A7.2 | 🟠 P1 | IDIOM | The new `uci-client` is a **library crate** — it must not repeat A4.1 (no `anyhow` in its public API). | Define a `thiserror` `UciError` enum (handshake timeout, unexpected EOF, parse failure, engine crashed). Mirror `BraidChessError`/`AcError`. Folds A7 into the A4.1 error-typology workstream. |
| A7.3 | 🟡 P2 | IDIOM | The UCI handshake (`uci → uciok → isready → readyok`) is sequenced by ad-hoc reads ([systems.rs:394-410](../src/game/ai/systems.rs#L394)); engine readiness is implicit. Prime **typestate** candidate (A4.3). | Encode the handshake in the type system: `UciEngine<Unconfigured>` → `setoption`/`isready` → `UciEngine<Ready>` where only `Ready` exposes `bestmove`. Invalid call order fails to compile. |
| A7.4 | 🟠 P1 | BUG | **Cancellation safety + interruptibility** — `nimzovich-uci`'s `stop` is a **no-op**; search is synchronous and uninterruptible ([nimzovich-uci/main.rs:288](../crates/engine/nimzovich-uci/src/main.rs#L288)). On the client side, a Stockfish `go` that is dropped mid-read (game abandoned / player resigns) must kill the child cleanly, not leak it. | Ties to A2.4. **Step 3**: threaded interruptible search behind an atomic stop flag (enables real `stop`/`go infinite`/ponder). In `uci-client`, make `bestmove` cancellation-safe — `Drop` must terminate the child process; add a test that drops the future mid-search. |
| A7.5 | 🟡 P2 | PERF | Time budgeting is wrong for tournaments: `parse_go_budget` ignores `movestogo` and there's no `go nodes` ([nimzovich-uci/main.rs:158](../crates/engine/nimzovich-uci/src/main.rs#L158)); the client emits only `cp`+`depth`. | **Step 3**: honour `movestogo`/`go nodes`; emit `score mate N`, full PV, per-iteration `info` streaming. Required for correct time control and for analysis/anti-cheat (A7.6) to read mate scores. |
| A7.6 | 🟡 P2 | ARCH | `xfchess-anticheat` shells out to Stockfish itself ([engine/stockfish.rs](../crates/shared/xfchess-anticheat/src/engine/stockfish.rs)) — duplicating the client logic the game already inlines (A1.4). The same duplication will recur in puzzles/analysis. | **Step 4**: route anti-cheat (and puzzles per [PUZZLES.md](PUZZLES.md), in-game analysis, bot fallback) through a backend `uci-client` pool. Collapses A1.1's Stockfish-spawn hardening + A1.4's coupling into one shared, tested boundary. The anti-cheat bounded job queue (Part II preamble) is the right host for that pool. |
| A7.7 | 🟢 P3 | TEST | Step 3 makes `nimzovich-uci` a *real* engine, unlocking **SPRT** strength testing (OpenBench/cutechess) and lichess-bot online play — strong verification leverage (A6.1). CI already has an "engine match sanity (no forfeits)" job to build on. | Add a cutechess/SPRT harness once `stop`/PV/`info` land; gate engine changes on no-regression match results. |
| A7.8 | 🔴 P0 | SEC | **no_std boundary must hold.** UCI = process spawning + IO; it must live **only** on the `std`/`search` side of `nimzovich_engine`. Any UCI/process/reqwest dep leaking into the no_std path breaks `chess-logic-on-chain` and the Solana program build. | Hard constraint from [crates/CLAUDE.md](../crates/CLAUDE.md). Keep `uci-client` a separate `std` crate; never add it to `chess-logic-on-chain`'s graph. Verify with `cargo build -p xfchess-game` after the work. Also preserve the **in-process Nimzovich TT-pool** (~2.2 GB) — the `Engine` trait is the swap boundary, *not* a mandate to pipe our own engine through a subprocess. |

**Sequencing (from the roadmap, ranked payoff/effort):** A7.1 (`uci-client` crate) → A7.2/A7.3
(error type + typestate, free while extracting) → A7.4/A7.5 (`nimzovich-uci` completeness) →
A7.6 (backend consumers). A7.8 is a constraint on *all* of them.

---

# Part III — Application components (src / backend / programs / tauri / web-solana)

> Extends the audit beyond `crates/` to the whole repo, same six-dimension checklist.
> Findings tagged by component (`G`=game `src/`, `B`=backend, `P`=Solana program, `T`=tauri,
> `W`=web-solana). Sizes: `src/` 55,158 LOC · `backend/` 27,597 · `programs/` 8,019 ·
> `tauri/` 2,463 · `web-solana/` 8,373 (TS).

## 🔴 P0 — the one exploit chain (fix before anything else)

Five individually-questionable settings in the **Tauri desktop app + web frontend** compose
into a full **XSS → native code execution** chain. Treat as a single P0 incident:

1. **CSP allows `script-src 'unsafe-inline'`** ([tauri/tauri.conf.json:24](../tauri/tauri.conf.json#L24)) — inline script injection is not blocked.
2. **`withGlobalTauri: true`** ([tauri.conf.json:13](../tauri/tauri.conf.json#L13)) — the native IPC bridge (`window.__TAURI__`) is exposed to all webview JS.
3. **`dangerouslySetInnerHTML`** with a hand-rolled regex "highlighter" that does not sanitize ([web-solana/CodeViewer.tsx:60](../web-solana/src/components/CodeViewer.tsx#L60)) — an injection sink.
4. **`shell:allow-execute` / `shell:allow-spawn` of PowerShell** with a baked-in `ssh root@<IP>` validator ([tauri/capabilities/default.json](../tauri/capabilities/default.json)) — the webview can spawn PowerShell.
5. **`open_url(url: String)` → `open::that(&url)` with no scheme validation** ([tauri/src/services/ipc.rs:111](../tauri/src/services/ipc.rs#L111)) — opens any URL/path/protocol the OS will handle.

**Why it's P0:** an attacker who lands any HTML/JS (via #1+#3, or a compromised dependency)
gets `window.__TAURI__` (#2), and from there can invoke `open_url` (#5) or the shell
capability (#4) to run native commands — on a machine that also holds the user's wallet/JWT.
The `ssh root@…` admin command should **never** ship in a distributed desktop app's capability
set.

**Fix all five:** remove `'unsafe-inline'` (hash/nonce inline scripts instead); set
`withGlobalTauri: false` and expose only the specific commands needed; replace the regex
highlighter with a vetted highlighter (`shiki`/`highlight.js`) + `DOMPurify`, or escape input
first; **delete the `shell:*` capabilities entirely** (move any deploy/SSH tooling out of the
shipped app into `scripts/`); and make `open_url` validate `http`/`https`/`mailto` only.

## T. Tauri desktop wrapper (`tauri/`, 2,463 LOC; 13 IPC commands)

| # | Sev | Cat | Finding | Action |
|---|-----|-----|---------|--------|
| T1 | 🔴 P0 | SEC | Shell capabilities + CSP + `open_url` + `withGlobalTauri` exploit chain (above). | The five fixes above. |
| T2 | 🟠 P1 | SEC | `env::set_var` for `RUST_LOG`/config in [config.rs](../tauri/src/services/config.rs)/[logging.rs](../tauri/src/utils/logging.rs) (mostly tests, but pattern present). Edition is **2021** — in **2024 `set_var` is `unsafe`** (process-wide env race). | Audit all `set_var`; prefer reading config into typed structs over mutating global env. Blocks a clean 2024 migration (also G-edition below). |
| T3 | 🟡 P2 | IDIOM | 13 `#[tauri::command]` handlers take raw `String`/`f64` (`set_tournament_admin_title(String)`, `show_notification(title, body)`, `copy_to_clipboard(text)`); no length bounds. | Bound/validate command args at the IPC trust boundary (§5 input sanitization) — these cross from webview to native. |
| T4 | 🟢 P3 | TEST | `tauri/src/utils/crypto.rs` exists — verify it uses vetted primitives (no custom crypto). | Spot-check against §5 crypto standard. |

## W. Web frontend (`web-solana/`, 8,373 LOC TS)

| # | Sev | Cat | Finding | Action |
|---|-----|-----|---------|--------|
| W-1 | 🔴 P0 | SEC | **Secret API keys bundled into client JS:** `VITE_HELIUS_API_KEY` ([useWalletUsdBalance.ts:6](../web-solana/src/hooks/useWalletUsdBalance.ts#L6)) and payment keys `VITE_MOONPAY_API_KEY`/`VITE_TRANSAK_API_KEY`/`VITE_BANXA_API_KEY` ([FundWallet.tsx:24-39](../web-solana/src/pages/FundWallet.tsx#L24)). `VITE_*` vars are inlined into the public bundle. | Move the **Helius** key server-side (proxy RPC through the backend). For MoonPay/Transak/Banxa, keep only *publishable* keys client-side; secret keys go server-side. Ties to [[project_secret_exposure]]. |
| W-2 | 🟠 P1 | SEC | `dangerouslySetInnerHTML` fed by a regex highlighter that doesn't sanitize ([CodeViewer.tsx:60](../web-solana/src/components/CodeViewer.tsx#L60)). | Part of the P0 chain (#3). Sanitize or replace; if `code` is only ever static literals today, still fix — it's one prop change from XSS. |
| W-3 | 🟠 P1 | SEC | **JWT in `localStorage`** (`xfchess_token`, [App.tsx:134](../web-solana/src/App.tsx#L134), [LoginModal.tsx:35](../web-solana/src/components/LoginModal.tsx#L35)) — readable by any injected script, so any XSS (W-2) escalates to account/wallet takeover. | Prefer httpOnly+SameSite cookies for the session token; if localStorage must stay, the XSS surface (W-2, T1) must be airtight. |
| W-4 | 🟡 P2 | IDIOM | **43** `: any` / `as any` / `@ts-ignore` across `web-solana/src` — erodes type safety at API/wallet boundaries. | Replace with real types (esp. around `import.meta.env` and API responses); enable `strict` + `noImplicitAny` if not already, and lint-ban `@ts-ignore`. |
| W-5 | 🟢 P3 | SEC | Lichess PKCE verifier/state in `sessionStorage` ([lichess.ts:67](../web-solana/src/lib/api/lichess.ts#L67)) — acceptable for PKCE. | No action; noted for completeness. |

## B. Backend (`backend/`, 27,597 LOC; `signing/` is 20,443)

| # | Sev | Cat | Finding | Action |
|---|-----|-----|---------|--------|
| B1 | 🟠 P1 | BUG | **Sync locks in an async server:** `invite_store: Arc<std::sync::RwLock<HashMap>>` ([signing/mod.rs:129](../backend/src/signing/mod.rs#L129)), plus `std::sync::RwLock` in [pyth_oracle.rs](../backend/src/signing/pyth_oracle.rs) and `std::sync::Mutex` in [routes/admin.rs](../backend/src/signing/routes/admin.rs). A std guard held across `.await` in an Axum handler blocks/deadlocks the executor (§2 lock discipline). | Audit each guard's span; ensure it's dropped before any `.await` (copy data out), or switch to `tokio::sync` where it must span an await. Highest-risk: `invite_store` (mutated in request handlers). |
| B2 | 🟠 P1 | SEC | **No global request body limit** — `DefaultBodyLimit` appears 0× in `backend/src`. Large/unbounded request bodies → allocation-exhaustion DoS (§5). | Add `DefaultBodyLimit::max(N)` to the router; set per-route limits where bodies are known-small. |
| B3 | 🟡 P2 | PANIC | 111 panic-surface hits (non-test) in backend — highest in the repo. Many are likely in `bin/`/startup, but the request-handling paths in `signing/routes/` must be panic-free (a panicked handler is a 500 + dropped connection). | Triage `signing/routes/**` for `unwrap`/`expect` on request-derived data; convert to `AppError`. |
| B4 | 🟢 P3 | IDIOM | **Done right:** `backend/src/error.rs` defines a `thiserror` `AppError` enum with `IntoResponse` (proper app error handling, §4); auth replay window (`AUTH_SIG_MAX_AGE_SECS=300`), `MAX_MESSAGE_LEN=500` chat bound, and anti-cheat registration caps exist. | Mirror these patterns; backend error/typology is a positive baseline. |
| B5 | 🟡 P2 | ARCH | `signing/` is 20k LOC in one module tree — verify transaction-building stays I/O-pure where possible and that the "backend never holds private keys" invariant (CLAUDE.md) is enforced structurally, not just by convention. | Spot-audit for any keypair/secret material in `signing/`; consider a type that *cannot* hold a private key. |

## G. Game client (`src/`, 55,158 LOC; multiplayer 14,575 / game 13,685)

| # | Sev | Cat | Finding | Action |
|---|-----|-----|---------|--------|
| G1 | 🟠 P1 | SEC | **Unjustified `unsafe impl Send`/`Sync`** for `AvatarCache` ([ui/game/game_ui.rs:131-132](../src/ui/game/game_ui.rs#L131-L132)), no SAFETY comment. Forcing `Send+Sync` so a type can live in a Bevy resource (accessed by parallel systems) is unsound if it holds non-thread-safe handles. | Prove thread-safety with a SAFETY comment, or make it genuinely `Send+Sync` (e.g. wrap the offending field). Same class as A3.2. The repo's only first-party `unsafe`. |
| G2 | 🟡 P2 | PANIC | 60 panic-surface hits (non-test). Concentrate on the multiplayer trust boundary ([src/multiplayer/](../src/multiplayer/), 14.5k LOC) — network/WS/P2P input must never `unwrap`. | Triage `multiplayer/**`; convert input-derived `unwrap` to error handling. Some bounds already exist (`p2p.rs` rejects `input.len() > 60`; rollup `MAX_*` constants). |
| G3 | 🟡 P2 | SEC | `std::env::set_var("RUST_BACKTRACE","full")` at startup ([core/plugin.rs:159](../src/core/plugin.rs#L159)) — fine in 2021, **`unsafe` in 2024**. | 2024-edition migration blocker (G-edition); set it before threads spawn or via the launcher env instead. |
| G4 | 🟡 P2 | ARCH | `multiplayer/` (14.5k) + `game/` (13.7k) are the largest modules; verify the Solana/ER paths stay behind the `solana` feature (CLAUDE.md) and that network transport is trait-abstracted for testability (§1 interface coupling). | Structural spot-check; ensure no Solana types leak into non-`solana` builds. |

## P. Solana program (`programs/xfchess-game/`, 8,019 LOC, ~50 instructions) — deep audit

> **Note:** [SOLANA_CRATES_AUDIT.md](SOLANA_CRATES_AUDIT.md) audited the *helper crates*
> (`crates/solana/*`), **not** this on-chain program. This section is its first deep pass.
> **Verdict: the contract is well-engineered — not typical AI output.** The money-handling
> paths I read are guarded correctly; findings are mostly hardening/consistency, with one P1
> that is a *key-custody* cross-reference rather than a code bug.

**What is correct (verified by reading the handlers, not just grep):**
- **Every privileged instruction gates on a hardcoded authority key** via `#[account(address =
  …::ID @ Err)] Signer`: `resolve_dispute` → `dispute_authority` ([resolve.rs:19](../programs/xfchess-game/src/governance_ix/resolve.rs#L19)),
  `verify_profile` → `kyc_authority`, fee-vault deposit → `vps_authority`, `link_external_elo`
  → `link_authority`. Several add a redundant in-handler `require!` too.
- **PDAs are seed-validated even when typed `UncheckedAccount`** — e.g. [withdraw.rs:13-15](../programs/xfchess-game/src/account_ix/withdraw.rs#L13)
  constrains `escrow_pda` with `seeds=[WAGER_ESCROW_SEED, game_id], bump`.
- **Prize distribution is textbook-safe** ([distribute.rs:75-113](../programs/xfchess-game/src/tournament_ix/prizes/distribute.rs#L75)):
  matches each winner by key from `remaining_accounts`, computes `prize_pool as u128 *
  share_bps / 10_000` with `checked_mul`/`checked_div`, **preserves the rent-exempt minimum**
  (`escrow - prize >= rent_min`), and uses a `prizes_claimed` bitmask to prevent double-payout.
- **Refund loop validates each account** ([cancel.rs:182-197](../programs/xfchess-game/src/tournament_ix/lifecycle/cancel.rs#L182)):
  `player_wallet.key() == all_players[i]` + `is_writable`, `checked_mul` total, balance check
  before paying — *not* the naive remaining-accounts drain vector.
- **`overflow-checks = true` in `[profile.release]`** ([Cargo.toml:94](../Cargo.toml#L94)) — raw
  arithmetic **aborts** the instruction on overflow/underflow rather than silently wrapping
  (the size-opt `[profile.release.package.xfchess-game]` override inherits it).

| # | Sev | Cat | Finding | Action |
|---|-----|-----|---------|--------|
| P1 | 🟠 P1 | SEC | **The whole privileged-instruction model rests on four authority keypairs** (`dispute_/kyc_/vps_/link_authority`) — exactly the keys flagged as **exposed** in [[project_secret_exposure]]. The on-chain code is correct; the risk is custody. A leaked `dispute_authority` can resolve disputes to itself; a leaked `vps_authority` can drain the fee vault. | Rotate all four before mainnet (already tracked); consider multisig/threshold authority for `dispute`/`kyc`. This is the contract's top risk and it lives *off*-chain. |
| P2 | 🟡 P2 | BUG | **Inconsistent rent-floor guard on manual lamport moves.** `distribute.rs` checks `escrow - prize >= rent_min` before `try_borrow_mut_lamports() -=`, but `resolve.rs:85-94` and the escrow branch of `cancel.rs:196` drain via `lamports.borrow_mut() -=` **without** the rent-floor check. Draining a PDA below rent-exemption can strand/garbage-collect it. | Apply the same `>= rent_min` guard (and prefer `try_borrow_mut_lamports()?` over `lamports.borrow_mut()` for the `?` path) uniformly across all manual transfers. |
| P3 | 🟡 P2 | IDIOM | **Raw arithmetic in money paths** — `wager_amount * 2` ([finalize.rs:69](../programs/xfchess-game/src/game_ix/finalize.rs#L69), resign, timeout, [resolve.rs:79](../programs/xfchess-game/src/governance_ix/resolve.rs#L79)), `wager_total - platform_fee`, `distributable / 2`. Safe today (overflow-checks aborts) but yields an opaque panic, not a clean error, and breaks if overflow-checks is ever disabled. | Convert to `checked_mul`/`checked_sub` → `GameErrorCode::Overflow`. Also **verify `anchor build` actually emits with `overflow-checks`** for the sBPF target (confirm the deployed `.so` isn't built from a profile that drops it). |
| P4 | 🟢 P3 | IDIOM | **Misleading `/// CHECK` comments.** Anchor requires CHECK docs to state *how* an account is validated; several just say what it is (e.g. "Wager escrow PDA" while the real guarantee is the `seeds` constraint). 158 `AccountInfo`/`UncheckedAccount` refs total. | Rewrite CHECK comments to cite the validating constraint; downgrade `UncheckedAccount`→`Account`/`SystemAccount` where the type is known. Low risk, high audit-legibility. |
| P5 | 🟡 P2 | SEC | **Tournament sharding (2,772 LOC) is the complexity hotspot** — `swiss_standings[*].color_balance += / -=` on a small int ([record_swiss_result.rs:119](../programs/xfchess-game/src/tournament_ix/matches/record_swiss_result.rs#L119)), index math into shard `Vec`s, cross-shard updates. Index OOB → abort; balance overflow → abort (overflow-checks), but logic errors here mis-pair/mis-pay. | Targeted review of `tournament_ix/matches/**` and `lifecycle/initialize_shards*`; add `proptest` invariants (no double-pairing, color balance bounded, sum of shards == registered). |
| P6 | 🟢 P3 | TEST | Strong existing verification: `tests/security_tests.rs`, `smoke_tests.rs`, on-chain differential tests, and TLA+ specs in [specs/](../specs/). Only 8 panic-surface hits. | Extend with `proptest` for instruction-arg decoding + the P5 tournament invariants (A6.1). The ~50-instruction constraint surface is largely covered; the unread instructions still warrant a line-by-line constraint pass. |

## Repo-wide (app components)

| # | Sev | Cat | Finding | Action |
|---|-----|-----|---------|--------|
| G-edition | 🟡 P2 | SEC | Whole workspace is **edition 2021**. A 2024 migration turns `set_var` (G3/T2) into `unsafe` and tightens `unsafe_op_in_unsafe_fn` — surfacing latent issues. | Plan the 2024 migration as a dedicated task after Phase 1; it's a free audit pass for the §3 "implicit unsafe" checklist row. |
| R-supply | 🟠 P1 | DEP | The supply-chain gates from A5.2/A6.2 (`cargo-deny`, `cargo audit`) apply to **all** Rust components, and `web-solana` needs `npm audit`/lockfile-deny too. | Extend the CI gates to backend/tauri/programs and add `npm audit` for the web app. |

---

## 9. Phased execution plan

**Phase −1 — P0 security (do this first, before any cleanup)**
The Part III exploit chain (T1 + W-1/W-2/W-3): remove CSP `'unsafe-inline'`, set
`withGlobalTauri:false`, delete the `shell:*` capabilities, validate `open_url` schemes,
sanitize/replace `dangerouslySetInnerHTML`, and move secret `VITE_*` keys server-side. Then
backend B2 (request body limit). These are trust-boundary fixes; nothing else matters if the
desktop app can be driven into native code execution.

**Phase 0 — instrument (no code change, ~30 min)**
`cargo clippy --workspace --all-targets` (run under **both** default and `--all-features` —
A6.4), `cargo machete`, `cargo +nightly udeps`, `cargo audit`, `cargo deny check`. Capture
output; it populates the P2/P3 long tail this doc summarizes. (Already run this pass for the
cheap Tier-A crates — `nimzovich_engine`/`shared`/`backend-types`/`swiss-pairing`/
`xfchess-anticheat` — producing E1–E6 + W6/W7 + the architecture census; the Solana and
`iroh-*`/`braid-*` crates still need their clippy/udeps/audit run.)

**Phase 1 — correctness / panic / soundness (P0/P1, small, high-value)**
S1 (float tiebreak), I1 (placeholder peer / no-unwrap SocketAddr), E4 (PGN parse panics),
A1 (Stockfish input/spawn hardening), **A3.1** (engine `transmute`→`bytemuck`),
**A3.2** (braid-http `unsafe` pin/`Send` → `pin-project-lite`), **G1** (`AvatarCache`
`unsafe impl Send/Sync` — prove or fix), **A5.1** (`game_id` length bound at the trust
boundary), **A2.2 + B1** (sync locks across `await` in the relay *and* backend `invite_store`/
`pyth_oracle`/admin). Each is a localized diff with a regression test.

**Phase 2 — dependency & dead-code (biggest build/maintenance win)**
W1 (drop backend `braid-core` if unused) → §5 Option A (gate `vendor`/CRDT behind a feature,
prove `braid-iroh` + backend still build) → E1/E2 engine dead-code removal (after resolving
A6.4) → W3/W4/W5/A5.3 dep hygiene. Defer §5 Option B/C pending owner decision on the CRDT
roadmap.

**Phase 3 — error-typology & Tier-C provenance**
A4.1 (`thiserror` enums for `braid-iroh`/`braid-core` public APIs) → §7 (record fork
upstreams, prune unused fork modules). A4.3 typestate is a separate scoped design task.

**Phase E — engine boundary & UCI (§A7, parallelizable with Phases 2–5)**
Its own workstream because it's feature-shaped, not cleanup, and is fully specified in
[UCI_INTEGRATION.md](UCI_INTEGRATION.md). Order: A7.1 (`uci-client` crate, with A7.2 `UciError`
+ A7.3 typestate baked in from the start) → A7.4/A7.5 (`nimzovich-uci` interruptible search,
`mate`/PV/`info`, `movestogo`) → A7.6 (backend UCI pool for anti-cheat/puzzles/analysis). A7.8
(no_std boundary + in-process TT pool) is a guardrail on every step — finish each with
`cargo build -p xfchess-game`. Resolves A1.4 and removes the A7.6 anti-cheat duplication.

**Phase 4 — idiomatic sweep + tooling (P2/P3)**
Clippy `--fix` on Tier A (E5/E6), `.clone()` triage (A1.1), naming verification, then adopt
the workspace lint posture: `[workspace.lints]` table (A6.3), `#![forbid(unsafe_code)]` on
the safe crates (A3.4), and the unwrap-policy lints (§8).

**Phase 5 — verification hardening**
Add `proptest` for parsers/state machines (A6.1: PGN, resource path, FEN, Swiss pairing,
Solana instruction decoding P1), cancellation-safety test for the Braid subscription (A2.4),
the new CI gates (`cargo deny`/`audit`/`machete`/`semver-checks` — A6.2) **extended to
backend/tauri/programs**, plus `npm audit` + TS `strict`/`@ts-ignore` ban for `web-solana`
(W-4, R-supply).

**Phase 6 — backend / contract / app hardening (P1/P2)**
B3 (panic triage in `signing/routes/**`), B5 (private-key invariant made structural), G2
(multiplayer trust-boundary `unwrap` triage), G4 (Solana feature-gating spot-check), T3 (IPC
arg bounds). **Smart contract:** P2 (uniform rent-floor guard on manual lamport moves),
P3 (raw→`checked_` money math + verify sBPF overflow-checks), P4 (CHECK-comment/`UncheckedAccount`
tidy), P5 (`tournament_ix/matches` review + Swiss invariants). **P1 (authority-key rotation)
is pre-mainnet and tracked in [[project_secret_exposure]] — not a code change.**

**Phase 7 — edition 2024 migration (dedicated, after Phase 1)**
Migrate the workspace from 2021→2024 (G-edition). This forces `set_var` (G3/T2) to become
`unsafe` and tightens `unsafe_op_in_unsafe_fn` — a free pass over the §3 "implicit unsafe"
checklist row.

**Phase 8 — verify the whole**
`cargo test --workspace`, `cargo build` (default + `--features solana`), `cargo build -p
xfchess-game` (no_std program build), `cargo build -p backend` / `-p xfchess-tauri`,
`npm run build` + `npm run lint` in `web-solana`, and the existing perft/differential/security
suites + the Anchor `security_tests`. Per [CLAUDE.md], manual-testing proof is required for
game-affecting changes.

---

## 10. Non-goals / risks

- **Do not** refactor `src/vendor/diamond_types` or the `iroh-*` forks for style — it breaks
  rebasing. Touch them only for §5/§7.
- **Do not** delete the CRDT (§5 Option C) without confirming no roadmap need (collaborative
  analysis/chat). Default to feature-gating (reversible) over deletion.
- **`no_std` risk:** any change to `nimzovich_engine` / `chess-logic-on-chain` must preserve
  `no_std` (CLAUDE.md hard constraint) — build the Solana program after touching them. This
  is doubly true for the UCI work (A7.8): `uci-client` is `std`-only and must never enter the
  no_std graph.
- **Do not pipe Nimzovich through a subprocess.** The `Engine` trait (§A7) is the boundary for
  swapping *external* engines; the in-process Nimzovich TT pool (~2.2 GB, pre-warmed) must stay
  in-process — routing it through UCI would lose the shared TT and add IPC latency.
- **Tournament/relay are real-money + live paths** — Phase 1 fixes need regression tests, not
  just "compiles".
- **Loose keypairs** under `er-cu-benchmark/keys/` (Solana doc §3) — carry forward; relates to
  the secret-exposure follow-ups already tracked.
- **Don't over-architect:** A4.3 typestate and A2.3 MPMC are improvements, not defects — scope
  them deliberately; a chess relay does not need an actor-framework rewrite. The bounded
  anticheat queue shows the right altitude already.
- **`unsafe impl Send` (A3.2) is the one genuine soundness risk** — prioritize proving or
  removing it; everything else `unsafe` is either vendored (Tier C) or mechanically replaceable
  with `bytemuck`/`pin-project-lite`.

---

## Appendix — full crate census (LOC / panic-hits)

| Crate | LOC | unwrap/expect/panic hits | Tier |
|-------|----:|-------------------------:|------|
| braid-core | 43,664 | 444 (mostly vendored) | C |
| nimzovich_engine | 8,469 | 15 (test/doc) | A |
| iroh-gossip | 8,026 | 71 | C |
| braid-http | 4,612 | 22 | C |
| iroh-h3-client | 3,203 | 58 | C |
| er-cu-benchmark | 3,006 | 58 | B (audited) |
| xfchess-anticheat | 2,135 | 1 (test) | A |
| swiss-pairing | 1,421 | 12 (1 live: S1) | A |
| braid-iroh | 1,316 | 13 (I1/I2/I3 live) | A |
| braid_chess | 895 | 20 (test/doc) | A |
| iroh-h3 | 601 | 15 | C |
| xfchess-braid-server | 593 | 1 | A |
| solana-chess-client | 536 | 2 | B (audited) |
| shared | 353 | 23 (test) | A |
| nimzovich-uci | 312 | 0 | A |
| iroh-h3-axum | 218 | 0 | C |
| chess-logic-on-chain | 63 | 0 | A (audited) |
| backend-types | 30 | 0 | A |
| *uci-client* (planned, §A7) | — | — | A (new — extract from `src/game/ai/systems.rs`) |
