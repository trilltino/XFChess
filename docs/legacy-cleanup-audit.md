# Legacy Cleanup Audit

Date: 2026-07-02

Scope: XFChess workspace, focusing on stale modules, broken helper targets, legacy compatibility paths, and items that can be removed without changing the product surface.

## Commands Run

```powershell
cargo metadata --no-deps --format-version 1
cargo check --workspace --all-targets
cargo check --features solana --bin on_chain_benchmark
cargo check --features solana --bin tournament_data_gen
rg -n "legacy|unused|deprecated|TODO|BraidP2PConfig|BraidPvp|BraidMultiplayer|solana::multiplayer" src backend crates programs docs specs
```

Result summary:

- `cargo check --workspace --all-targets` passes without the root `solana` feature.
- `cargo check --features solana --bin on_chain_benchmark` fails.
- `cargo check --features solana --bin tournament_data_gen` fails.
- Full `cargo check --workspace --all-targets --features solana` timed out, then targeted checks confirmed the known stale bins.

## Safe Remove Candidates

These have no meaningful implementation or are only self-referenced by stale declarations.

### 1. Empty legacy Solana multiplayer module

Files:

- `src/solana/multiplayer/mod.rs`
- `src/solana/multiplayer/ui.rs`

Evidence:

- `src/solana/multiplayer/mod.rs` says "Legacy multiplayer code - all contents unused".
- `src/solana/multiplayer/ui.rs` says "Legacy Solana UI code - all contents unused".
- Repo search found no callers of `crate::solana::multiplayer`.

Removal:

- Delete `src/solana/multiplayer/`.
- Remove `pub mod multiplayer;` from `src/solana/mod.rs`.

### 2. Legacy Braid subscription resource shell — DONE

`src/multiplayer/network/braid.rs` and its resource injection have already been removed
(verified: file no longer exists as of this pass). No action remaining.

### 3. Stale root Solana helper bins

Files:

- `src/bin/on_chain_benchmark.rs`
- `src/bin/tournament_data_gen.rs`

Evidence:

- `on_chain_benchmark` fails with unresolved import `xfchess::nimzovich_engine`.
- `tournament_data_gen` fails against current instruction signatures.
- Equivalent maintained benchmarking now appears to live under `crates/solana/er-cu-benchmark`.

Removal:

- Delete the two files.
- Remove their `[[bin]]` entries from root `Cargo.toml`.

Risk: medium only if someone uses these exact local helpers. Product/runtime risk is low.

### 4. Orphaned `game_ix` instruction files (not part of the build)

Files:

- `programs/xfchess-game/src/game_ix/record.rs`
- `programs/xfchess-game/src/game_ix/record_move.rs`

Evidence:

- `programs/xfchess-game/src/game_ix/mod.rs` does not declare `pub mod record;` or
  `pub mod record_move;` — Rust does not compile a file unless it is reachable from a
  `mod` declaration, so these two are not part of the program build at all.
- The real move-recording path is `programs/xfchess-game/src/moves_ix/record.rs`
  (wired up in `lib.rs` as the `record_move` instruction via `moves_ix::RecordMove`).
- `game_ix/record.rs`'s `record_result` handler body ends with the comment
  `// Additional logic for recording game result here` — it reads as an abandoned
  draft, not a stale-but-complete instruction.

Removal:

- Delete both files; no `mod`/`use` changes needed elsewhere since nothing references them.

Risk: none — they are not compiled today, so deleting them cannot change program behavior.

### 5. Unused `TreasuryVault` account type

Files:

- `programs/xfchess-game/src/state/treasury_vault.rs`

Evidence:

- Declares a per-country typed treasury account (`seeds = [TREASURY_VAULT_SEED, country]`)
  but a repo-wide search finds no instruction that constructs, initializes, or
  deserializes a `TreasuryVault`.
- Every real treasury reference (`account_ix/treasury.rs`'s `WithdrawTreasury`,
  `governance_ix::resolve`, `governance_ix::claim_stale_dispute`,
  `lifecycle::settlement`) uses a single global, untyped `SystemAccount` at
  `seeds = [TREASURY_VAULT_SEED]` — no country component at all.

Removal:

- Delete `state/treasury_vault.rs`, remove `pub mod treasury_vault;` and
  `pub use treasury_vault::*;` from `state/mod.rs`.

Risk: none — no deployed account has ever had this shape, so there is no on-chain data
this type needs to stay compatible with (unlike removing fields from a struct that
*is* live, e.g. `PlayerProfile`).

### 6. Orphaned `src/multiplayer/solana/rpc.rs`

Files:

- `src/multiplayer/solana/rpc.rs`

Evidence:

- `src/multiplayer/solana/mod.rs` does not declare `pub mod rpc;` — not
  reachable from any `mod` statement, not compiled at all (same class as item 4).
- A differently-scoped `SolanaRpc` struct with the same field names is defined
  separately in `integration/systems.rs`, which is the one actually referenced
  (by code that is itself dead — see the doc-audit findings in
  `docs/AUDIT_TRACKING.md` Phase 8 for the fuller picture, including
  `integration/rpc.rs` and `integration/systems.rs`'s `setup_solana_system`
  cluster, which compile but are never called/registered).

Removal:

- Delete `src/multiplayer/solana/rpc.rs`; no `mod`/`use` changes needed
  elsewhere since nothing references it.

Risk: none — not compiled today, so deleting it cannot change program behavior.

## Conditional Remove Candidates

These may be removable after confirming current UX/ops choices.

### 7. Old `GameState::MultiplayerMenu` UI path

Files:

- `src/ui/menus/multiplayer_menu.rs`
- `src/core/states.rs` variant `MultiplayerMenu`

Evidence:

- The plugin is registered, but search did not find a current transition into `GameState::MultiplayerMenu`.
- The current main menu uses `src/states/main_menu/*` and Solana/P2P lobby helpers instead.
- The file still contains TODOs for unimplemented gossip matchmaking.

Removal path:

- Confirm no button or CLI path enters `GameState::MultiplayerMenu`.
- Remove `MultiplayerMenuPlugin` registration from `src/ui/mod.rs`.
- Remove `GameState::MultiplayerMenu` and allowed transitions.
- Delete `src/ui/menus/multiplayer_menu.rs`.

Risk: medium; it is UI-visible if an old route still reaches it.

### 8. Duplicate backend binary alias

Manifest:

- `backend/Cargo.toml`

Evidence:

- Both `signing-server` and `signing-server-http` point to `backend/src/signing_server.rs`.
- Cargo warns that the same file is present in multiple build targets.

Removal path:

- Pick one canonical binary name.
- Update scripts using the removed alias, notably `scripts/start-tournament-admin.bat` currently uses `signing-server-http`.

Risk: medium because ops/local scripts may depend on the alias.

## Do Not Simply Remove Yet

These look legacy but remain active in dependency graphs or compatibility logic.

### Braid/Iroh protocol crates

Packages:

- `braid-core`
- `braid-http`
- `braid-iroh`
- `iroh-gossip`
- `iroh-h3`
- `iroh-h3-client`
- `iroh-h3-axum`
- `braid_chess`

Evidence:

- `cargo tree -i` shows these flow into `xfchess` and/or `backend`.
- `braid_chess` is still used for move payload/version hashing, social chat subscription, rollup bridging, replay, and VPS move-log handling.
- `backend` still starts a `braid-iroh` node.

Recommendation:

- Do not delete as part of simple cleanup.
- Later, split "Braid document compatibility" from "OnlineGameSession live transport" more explicitly.

### On-chain ABI compatibility fields

Files:

- `programs/xfchess-game/src/delegation_ix/delegate.rs`
- `programs/xfchess-game/src/tournament_ix/registration/register.rs`
- `programs/xfchess-game/src/tournament_ix/prizes/claim_prize.rs`
- `programs/xfchess-game/src/moves_ix/record.rs`

Evidence:

- Several comments explicitly say fields are kept for ABI/client compatibility or legacy clients.

Recommendation:

- Do not remove unless doing a coordinated program/client ABI migration.

### `BraidPatch::from_message` — zero call sites, seed-collision bug baked in

Files:

- `crates/zarathustra_net/braid_chess/src/patch.rs:46-58`

Evidence (re-verified `docs/PRE_MAINNET_E2E_PLAN.md` §1.4, 2026-08-02):

- `from_message` seeds non-move events (resign, draw offer, etc.) with a
  constant `version_hash(version_seed, 0)` — two different non-move events
  sharing a `version_seed` collide.
- Zero call sites anywhere in the repo (`grep -rn "from_message"` across
  `src`, `backend`, `crates`, `programs` — the only other hit is an unrelated
  `plumtree::GossipEvent::from_message` inside the vendored `iroh-gossip`
  fork). Live resign/draw/offer events go through `NetworkMessage`/
  `ChessMessage` (P2P) or `backend/src/signing/routes/game_log.rs`'s SQLite
  log directly, both bypassing this function entirely.

Recommendation — **do not add test coverage to this function, and do not
wire up a caller without first fixing the seed** (fold in a monotonic
counter or `agent_id` before any real non-move `version_hash` reaches
storage). If it's still unused in a future cleanup pass, delete it then;
this entry exists so the collision bug doesn't silently get resurrected by
a caller that assumes the seeding is already safe.

## Recommended Cleanup Order

1. Remove `src/solana/multiplayer`.
2. ~~Remove `src/multiplayer/network/braid.rs`~~ — already done.
3. Remove or quarantine `on_chain_benchmark` and `tournament_data_gen`.
4. Delete the orphaned `game_ix/record.rs` and `game_ix/record_move.rs` — zero risk, they are not compiled.
5. Delete the unused `state::TreasuryVault` type — zero risk, never instantiated on-chain.
6. Delete the orphaned `src/multiplayer/solana/rpc.rs` — zero risk, not compiled.
7. Decide whether `GameState::MultiplayerMenu` is still reachable; remove if not.
8. Collapse backend `signing-server`/`signing-server-http` to one canonical bin after updating scripts.

