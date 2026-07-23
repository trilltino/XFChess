# XFChess

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Discord](https://img.shields.io/badge/Discord-Join-purple.svg)](https://discord.gg/erZJCPCm)
[![GitHub Stars](https://img.shields.io/github/stars/trilltino/XFChess?style=social)](https://github.com/trilltino/XFChess/stargazers)

**[Install](docs/INSTALL.md)** · **[MagicBlock Integration](MAGICBLOCK.md)** · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [Environment Guide](docs/ENVIRONMENTS.md) · [Runbooks](docs/runbooks/)

XFChess is a forever-free, open source 3D chess platform built with Rust, Bevy, Solana, MagicBlock, and Iroh/Braid networking. It supports local play, online multiplayer, tournaments, Solana-backed game state, wager escrow, session keys, spectators, and observability tooling.

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
- [Runbooks](docs/runbooks/)

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

## Repo Map

| Path | Purpose |
| --- | --- |
| `src/` | Native Bevy game client |
| `backend/` | Axum backend and signing service |
| `programs/xfchess-game/` | Anchor program |
| `crates/engine/` | Nimzovich chess engine |
| `crates/shared/` | Shared protocol, pairing, backend types, anti-cheat |
| `crates/solana/` | Solana helpers, on-chain chess logic, ER benchmarks |
| `crates/zarathustra_net/` | Braid/Iroh networking crates |
| `xfchessdotcom/` | React/Solana web frontend |
| `tauri/` | Desktop companion services and visualization tooling |
| `docs/` | Architecture notes, ADRs, plans, runbooks |
| `deploy/` | VPS, nginx, staging, backend, monitoring config |

## Quick Start

### Prerequisites

- Rust stable
- Node.js 18+
- Solana CLI
- Anchor CLI compatible with Anchor `0.31.1`
- Docker and Docker Compose, optional but recommended for services

### Native Client

```bash
git clone https://github.com/trilltino/XFChess.git
cd XFChess
cargo run
```

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

## Development

Common checks:

```bash
cargo test
cargo test -p xfchess-game
cargo test -p nimzovich_engine
```

Solana program tests need a built program artifact:

```powershell
scripts\build_program.bat
cargo test -p xfchess-game --test er_move_tests
cargo test -p xfchess-game --test er_delegation_tests
cargo test -p xfchess-game --test game_settlement_tests
```

## Features

- 3D Bevy chess board with animated pieces and UI
- Local play, online games, spectators, puzzles, and tournaments
- Solana Anchor program for game lifecycle, wager escrow, profiles, ELO, disputes, and treasury
- MagicBlock Ephemeral Rollups for low-latency move recording
- Session keys to avoid wallet popups on every move
- Backend signing service for auth, matchmaking, settlement, tournaments, and operations
- Iroh/Braid networking for realtime sync
- Nimzovich chess engine and UCI binary
- Anti-cheat support crate and backend report storage
- Prometheus/Grafana observability and production runbooks

## Deployment

Deployment material lives in:

- `deploy/README.md`
- `deploy/backend/.env.example`
- `deploy/staging/.env.staging.example`
- `docs/ENVIRONMENTS.md`
- `docs/runbooks/`

## Community

- Discord: [Join the community](https://discord.gg/erZJCPCm)
- Issues: [GitHub Issues](https://github.com/trilltino/XFChess/issues)
- Releases: [GitHub Releases](https://github.com/trilltino/XFChess/releases)

## License

XFChess is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE).

If you run a modified XFChess network service, you must provide the corresponding source code to users of that service.
