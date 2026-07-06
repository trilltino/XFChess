# Workspace crates

Shared library crates consumed by the game client (`src/`), the backend (`backend/`),
the Solana program (`programs/xfchess-game/`), and the web frontend. Organized into
four groups:

| Group | Path | Contents |
|-------|------|----------|
| Engine | [`engine/`](engine/) | `nimzovich_engine` (chess AI: move gen + alpha-beta search) and `nimzovich-uci` (UCI adapter for engine-vs-engine testing) |
| Shared types & logic | [`shared/`](shared/) | `shared` (client↔backend protocol types), `backend-types` (serde-only backend↔web types), `swiss-pairing` (FIDE Dutch pairing), `xfchess-anticheat` (post-game Stockfish analysis) |
| Solana | [`solana/`](solana/) | `chess-logic-on-chain` (no_std move validation compiled into the program), `solana-chess-client` (client-side transaction builders), `er-cu-benchmark` (Ephemeral Rollup compute-unit benchmarks) |
| Networking | [`zarathustra_net/`](zarathustra_net/) | The Braid-HTTP (209) protocol stack and Iroh QUIC/H3 transports powering live game subscriptions and P2P relay |

## Hard constraints

- **`chess-logic-on-chain` must stay `no_std`.** It compiles into the Solana program
  (`move-validation` feature). Any `std` import breaks the program build.
- **`nimzovich_engine` has two personalities.** With `features = ["std", "search"]` it
  runs full alpha-beta search (client, backend). Without `std` it provides move
  generation only, which is what runs on-chain via `chess-logic-on-chain`.
- **`swiss-pairing`'s `network` feature** adds Axum handlers — enable it only in the
  backend, never in the game client.
- **Braid ≠ WebSocket.** HTTP-209 Braid subscriptions carry board/tournament state
  sync; WebSockets carry auth/signaling. They are separate channels with separate roles.

## Adding a new crate

1. Create `crates/<group>/<name>/Cargo.toml` + `src/lib.rs`.
2. Add it to `workspace.dependencies` in the root `Cargo.toml`.
3. Reference it via `{ path = "crates/<group>/<name>" }` (or `.workspace = true`) from consumers.
4. Prefer keeping the crate a workspace member; exclusion disables `dep.workspace = true`
   inheritance and trades drift-protection for build time.

Per-crate details live in each crate's own README. AI-assistant guidance for this tree
is in [CLAUDE.md](CLAUDE.md).
