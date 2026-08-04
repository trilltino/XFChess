# XFChess

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![CI](https://github.com/trilltino/XFChess/actions/workflows/ci.yml/badge.svg)](https://github.com/trilltino/XFChess/actions/workflows/ci.yml)
[![Release](https://github.com/trilltino/XFChess/actions/workflows/release.yml/badge.svg)](https://github.com/trilltino/XFChess/actions/workflows/release.yml)
[![Discord](https://img.shields.io/badge/Discord-Join-purple.svg)](https://discord.gg/erZJCPCm)
[![GitHub Stars](https://img.shields.io/github/stars/trilltino/XFChess?style=social)](https://github.com/trilltino/XFChess/stargazers)

**[Install](docs/INSTALL.md)** · **[MagicBlock Integration](MAGICBLOCK.md)** · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [Environment Guide](docs/ENVIRONMENTS.md) · [Runbooks](docs/runbooks/) · [Full Docs Index](docs/README.md)

XFChess is a forever-free, open source 3D chess platform: local play against a
built-in engine, online multiplayer and tournaments, and optional Solana-backed
wagered play with on-chain escrow, ELO, and dispute resolution — all in one
client, no separate "crypto mode."

The native client is written in Rust on [Bevy](https://bevyengine.org/) (ECS,
3D rendering, [egui](https://github.com/emilk/egui) UI). Chess logic — move
generation, legality, check/checkmate — lives in the `nimzovich_engine` crate,
which is `no_std`-compatible so the exact same logic runs both as a full
alpha-beta search engine on the client and, via `chess-logic-on-chain`, inside
the Solana program itself. Multiplayer sync is peer-to-peer over
[Iroh](https://iroh.computer/) QUIC gossip, backed by a Rust port of the
[Braid-HTTP 209](https://braid.org/) streaming-subscribe protocol as a durable
server-side fallback and catch-up path, so moves, chat, and clock state still
replicate correctly even when a direct P2P link never establishes. The backend
is an async [Axum](https://github.com/tokio-rs/axum) server on Tokio, backed
by SQLite via [SQLx](https://github.com/launchbadge/sqlx); it builds Solana
transactions but never signs them — the player's wallet or a delegated
session key does that, client-side, so the backend never touches a private
key. On-chain game state, wager escrow, ELO, and tournaments run through an
[Anchor](https://www.anchor-lang.com/) program, with
[MagicBlock](https://www.magicblock.gg/) Ephemeral Rollups delegating the live
game account off mainnet for sub-second move recording before committing the
result back. The web frontend (`xfchessdotcom/`) is
[React](https://react.dev/) + [Vite](https://vite.dev/) +
[Chakra UI](https://chakra-ui.com/); the desktop build is wrapped with
[Tauri](https://tauri.app/). Production observability runs on
[Prometheus](https://prometheus.io/) + [Grafana](https://grafana.com/).

Just want to play? Grab a prebuilt build from
[Releases](https://github.com/trilltino/XFChess/releases) — see
[docs/INSTALL.md](docs/INSTALL.md) for per-platform steps (including the
SmartScreen/Gatekeeper prompts unsigned builds currently trigger). Single-player
against the engine works fully offline; online multiplayer, tournaments, and
wagered play need an internet connection and, for on-chain features, a Solana
wallet (Phantom or Solflare).

![Gameplay Screenshot](docs/images/screenshot_1.png)
![Tournament Interface](docs/images/screenshot_2.png)
![Multiplayer Match](docs/images/screenshot_3.png)

## Docs

- [Install (Windows/macOS/Linux)](docs/INSTALL.md)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [Environment guide](docs/ENVIRONMENTS.md)
- [Git workflow](docs/GIT_WORKFLOW.md)
- [Publishing a release](docs/PUBLISHING.md)
- [Runbooks](docs/runbooks/)
- [Full docs index](docs/README.md) — ADRs, architecture deep dives, threat model, SLOs, capacity/scaling/DR plans

## Architecture

```text
Bevy client
  |-- local 3D chess, UI, input, animation, AI
  |-- Solana/session-key UX
  |-- Iroh/Braid realtime sync

Backend API
  |-- wallet auth, matchmaking, tournaments
  |-- signing routes, relay routes, settlement workers
  |-- metrics, logs, health checks

Solana program
  |-- game lifecycle, escrow, profiles, ELO
  |-- tournaments, disputes, treasury
  |-- MagicBlock delegation and settlement boundaries

MagicBlock Ephemeral Rollups
  |-- delegated Game PDA
  |-- low-latency move recording
  |-- commit + undelegate before base-layer settlement
```

Every move is validated twice by the same logic: once locally for instant UI
feedback, and once on-chain (or on the delegated Ephemeral Rollup, for
wagered games) as the authoritative source of truth. Casual and local games
never touch the chain at all — Solana is opt-in, not a hard dependency of the
game.

## Repo Map

Everything — game client, backend, Solana program, web frontend, and
deploy/ops tooling — lives in this one repository; there's no separate
private half.

| Path | Purpose |
| --- | --- |
| `src/` | Native Bevy game client |
| `backend/` | Axum backend and signing service — see [backend/README.md](backend/README.md) |
| `programs/xfchess-game/` | Anchor program |
| `crates/engine/` | Nimzovich chess engine |
| `crates/shared/` | Shared protocol, pairing, backend types, anti-cheat |
| `crates/solana/` | Solana helpers, on-chain chess logic, ER benchmarks |
| `crates/zarathustra_net/` | Braid/Iroh networking crates |
| `xfchessdotcom/` | React/Solana web frontend |
| `tauri/` | Desktop companion services and visualization tooling |
| `docs/` | Architecture notes, ADRs, plans, runbooks |
| `ops/` | VPS, nginx, staging, backend, monitoring config |

## Quick Start

### Prerequisites

- Rust stable
- Node.js 18+
- Solana CLI
- Anchor CLI matching `anchor-lang` 1.1.2 (see `programs/xfchess-game/Cargo.toml`)
- Docker and Docker Compose, optional but recommended for services

### Native Client

```bash
git clone https://github.com/trilltino/XFChess.git
cd XFChess
cargo run
```

Add `--features solana` to build with Solana wallet/wager support enabled.

### Build

```powershell
scripts\build.bat
scripts\build_program.bat
```

### Web Frontend

```bash
cd xfchessdotcom
npm ci
npm run dev
```

### Docker Services

```bash
docker-compose up -d
```

## Backend API

The backend (`backend/`) is what the native client and web frontend both talk
to for matchmaking, session-key auth, tournaments, and building (never
signing) Solana transactions — the player's wallet or a delegated session key
signs client-side, so a private key never reaches the server. It also serves
a durable move/chat log over the Braid-HTTP 209 streaming-subscribe protocol,
which is how a game keeps syncing even if the direct peer-to-peer link never
comes up. See [backend/README.md](backend/README.md) for the full route map,
binaries, and module layout.

- Health check: `GET /health`
- Metrics (Prometheus format): `GET /metrics`

## Development

Common checks:

```bash
cargo test
cargo test -p xfchess-game
cargo test -p nimzovich_engine
cargo fmt
cargo clippy
```

Solana program tests need a built program artifact:

```powershell
scripts\build_program.bat
cargo test -p xfchess-game --test er_move_tests
cargo test -p xfchess-game --test er_delegation_tests
cargo test -p xfchess-game --test game_settlement_tests
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for commit conventions, PR expectations,
and the AI-assisted-contribution disclosure policy.

## Features

- 3D Bevy chess board with animated pieces and UI
- Local play, online games, spectators, puzzles, and tournaments
- Solana Anchor program for game lifecycle, wager escrow, profiles, ELO, disputes, and treasury
- MagicBlock Ephemeral Rollups for low-latency move recording
- Session keys to avoid wallet popups on every move
- Backend signing service for auth, matchmaking, settlement, tournaments, and operations
- Iroh/Braid networking for realtime sync, with a durable backend fallback path
- Nimzovich chess engine and UCI binary
- FIDE Dutch Swiss tournament pairing
- Anti-cheat support crate and backend report storage
- Prometheus/Grafana observability and production runbooks

## Deployment

Deployment material lives in:

- `ops/README.md`
- `ops/backend/.env.example`
- `ops/staging/.env.staging.example`
- `docs/ENVIRONMENTS.md`
- `docs/PUBLISHING.md`
- `docs/runbooks/`

## Credits

[![Contributors](https://contrib.rocks/image?repo=trilltino/XFChess)](https://github.com/trilltino/XFChess/graphs/contributors)

## Community

- Discord: [Join the community](https://discord.gg/erZJCPCm)
- Issues: [GitHub Issues](https://github.com/trilltino/XFChess/issues)
- Releases: [GitHub Releases](https://github.com/trilltino/XFChess/releases)

## License

XFChess is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE).

If you run a modified XFChess network service, you must provide the corresponding source code to users of that service.
