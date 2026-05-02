# XFChess

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Discord](https://img.shields.io/badge/Discord-Join-purple.svg)](https://discord.gg/erZJCPCm)
[![GitHub Stars](https://img.shields.io/github/stars/trilltino/XFChess?style=social)](https://github.com/trilltino/XFChess/stargazers)

**XFChess** is a 3D chess game built with Rust, Bevy, and Solana blockchain integration. Play locally or online with move verification and tournament support.

![Screenshot](https://via.placeholder.com/800x450?text=Gameplay+Screenshot)
![Screenshot](https://via.placeholder.com/800x450?text=Tournament+Interface)
![Screenshot](https://via.placeholder.com/800x450?text=Multiplayer+Match)

## Table of Contents

- [Architecture](#architecture)
- [Installation](#installation)
- [Development](#development)
- [Features](#features)
- [Contributing](#contributing)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security](SECURITY.md)
- [License](#license)
- [Credits](#credits)
- [About](#about)

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         XFChess Architecture                  │
└─────────────────────────────────────────────────────────────────┘

┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Game Client │────▶│  Backend API │────▶│  Solana RPC  │
│  (Bevy/Rust)  │     │  (Axum/Rust) │     │   Network    │
└──────────────┘     └──────────────┘     └──────────────┘
       │                    │                    │
       │                    ▼                    │
       │            ┌──────────────┐             │
       │            │  PostgreSQL  │             │
       │            │   (Sessions) │             │
       │            └──────────────┘             │
       │                                         │
       ▼                                         ▼
┌──────────────┐                       ┌──────────────┐
│  P2P Network │                       │   Solana     │
│  (Iroh/QUIC) │                       │   Program    │
└──────────────┘                       │  (xfchess)   │
                                        └──────────────┘
                                               │
                                               ▼
                                    ┌──────────────────┐
                                    │ Ephemeral Rollups│
                                    │   (MagicBlock)    │
                                    │   EU Devnet      │
                                    └──────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Observability Stack                          │
├─────────────────────────────────────────────────────────────────┤
│  Prometheus ◀── Metrics ──▶ Backend API                           │
│     │                                                            │
│     └──▶ Grafana ──▶ Dashboards                                 │
│                                                                  │
│  Logging ──▶ Structured Logs ──▶ ELK Stack (optional)           │
└─────────────────────────────────────────────────────────────────┘
```

### Components

- **Game Client**: 3D chess board built with Bevy, handles local gameplay and rendering
- **Backend API**: REST API for matchmaking, tournaments, and game state management
- **Solana Program**: Smart contract for move verification and tournament prize escrow
- **Ephemeral Rollups**: MagicBlock integration for sub-second move latency
- **P2P Network**: Iroh/QUIC for direct peer-to-peer communication
- **Observability**: Prometheus metrics, Grafana dashboards, structured logging

## Installation

### Prerequisites
- Rust toolchain (stable)
- Node.js 18+ (for web interface)
- Solana CLI (optional, for blockchain features)
- Docker (optional, for containerized deployment)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/trilltino/XFChess.git
cd XFChess

# Build the game
cargo build --release

# Run the game
cargo run --release
```

### Build Scripts

Use the provided build scripts for comprehensive building:

```bash
# Build all components (game, backend, wallet UI, web frontend, Solana program)
scripts\build.bat

# Build Solana program with size optimization
scripts\build_program.bat
```

### Docker Deployment

```bash
# Build and run with Docker Compose
docker-compose up -d
```

See [OBSERVABILITY.md](docs/OBSERVABILITY.md) for more details.

## Development

### Local Development

```bash
# Run local development with monitoring stack
scripts\run_offline.bat
```

This starts:
- Game client
- Backend server with telemetry
- Local monitoring stack (Prometheus, Grafana)
- Wallet UI

### Observability

The project includes comprehensive observability:
- **Prometheus metrics** at `/metrics`
- **Grafana dashboards** at `http://localhost:3000`
- **Debug transaction API** at `/api/debug/transaction/:signature`

See [OBSERVABILITY.md](docs/OBSERVABILITY.md) for full documentation.

### Solana Development

The Solana program uses **ephemeral-rollups-sdk v0.8.5** for sub-second latency:

```bash
# Build program
scripts\build_program.bat

# Deploy to devnet
anchor deploy

# Deploy to mainnet (requires ~6.5 SOL)
solana program deploy target/deploy/xfchess_game.so
```

See [SMART_CONTRACTS_GUIDE.md](docs/SMART_CONTRACTS_GUIDE.md) for contract details.

## Features

- **3D Chess Board**: Beautiful isometric rendering with piece animations
- **Blockchain Integration**: Solana-based game history and move verification
- **Ephemeral Rollups**: Sub-second move latency via MagicBlock EU devnet
- **Multiplayer Support**: Play against friends locally or online
- **Tournament System**: Create and manage chess tournaments with prize escrow
- **Cross-Platform**: Windows, macOS, and Linux support
- **Observability**: Built-in Prometheus metrics, Grafana dashboards, structured logging
- **Docker Support**: Containerized deployment with monitoring stack
- **P2P Networking**: Direct peer-to-peer communication via Iroh/QUIC
- **Crash Reporting**: Client-side crash collection and upload

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute to XFChess.

## Code of Conduct

Please see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for our community guidelines and enforcement policies.

## Security

Please see [SECURITY.md](SECURITY.md) for our security policy, including how to report vulnerabilities and contract security guidelines.

## License

This project is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE) for details.

**Important:** If you run a modified version of XFChess as a network service, you must provide the source code to your users.

## Credits

- **[trilltino](https://github.com/trilltino)** - Project lead
- **Bevy** - Game engine
- **Solana** - Blockchain platform
- **MagicBlock** - Ephemeral Rollups infrastructure
- **Iroh** - P2P networking

### Contributors

Thanks to all contributors who have helped make XFChess better!

## Community

- **Discord**: [Join our Discord](https://discord.gg/erZJCPCm)
- **GitHub**: [Report issues](https://github.com/trilltino/XFChess/issues)
- **Releases**: [Latest releases](https://github.com/trilltino/XFChess/releases)

## About

XFChess is a modern 3D chess game combining traditional gameplay with blockchain technology. Built with Rust and Bevy, it offers a smooth gaming experience with move verification on Solana and sub-second latency via Ephemeral Rollups.

**Key Technologies:**
- **Rust** - Core game engine
- **Bevy** - 3D graphics and game framework
- **Solana** - Blockchain integration
- **Anchor** - Solana smart contract framework
- **MagicBlock** - Ephemeral Rollups infrastructure
- **Iroh** - P2P networking
- **Axum** - Backend web framework
- **Prometheus/Grafana** - Observability stack
- **Docker** - Containerization
- **React** - Web interface

---

**[XFChess](https://github.com/trilltino/XFChess)** - The forever free, open source 3D chess game

