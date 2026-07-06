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

### 2. Legacy Braid subscription resource shell

Files:

- `src/multiplayer/network/braid.rs`

Evidence:

- `BraidGameState`, `BraidConnectionStatus`, and `NetworkGameStateUpdated` are not used outside their defining file.
- `BraidSubscriptionConfig` is only initialized and injected into `MainMenuUIContext`; no code reads it.
- Live game transport now uses `OnlineGameSession`.

Removal:

- Remove `pub mod braid;` and `pub use braid::*;` from `src/multiplayer/network/mod.rs`.
- Remove `.init_resource::<network::braid::BraidSubscriptionConfig>()` from `src/multiplayer/mod.rs`.
- Remove `braid_subscription` from `MainMenuUIContext`.
- Delete `src/multiplayer/network/braid.rs`.

Risk: low, but compile-check after removal because `MainMenuUIContext` is a large Bevy system param.

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

## Conditional Remove Candidates

These may be removable after confirming current UX/ops choices.

### 4. Old `GameState::MultiplayerMenu` UI path

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

### 5. Duplicate backend binary alias

Manifest:

- `backend/Cargo.toml`

Evidence:

- Both `signing-server` and `signing-server-http` point to `backend/src/signing_server.rs`.
- Cargo warns that the same file is present in multiple build targets.

Removal path:

- Pick one canonical binary name.
- Update scripts using the removed alias, notably `scripts/start-tournament-admin.bat` currently uses `signing-server-http`.

Risk: medium because deploy/local scripts may depend on the alias.

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

## Recommended Cleanup Order

1. Remove `src/solana/multiplayer`.
2. Remove `src/multiplayer/network/braid.rs` and the unused resource injection.
3. Remove or quarantine `on_chain_benchmark` and `tournament_data_gen`.
4. Decide whether `GameState::MultiplayerMenu` is still reachable; remove if not.
5. Collapse backend `signing-server`/`signing-server-http` to one canonical bin after updating scripts.

