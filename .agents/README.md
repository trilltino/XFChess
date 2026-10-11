# XFChess skills

Project skills live in `skills/<name>/SKILL.md` and are available from the next
agent turn. Load the skills relevant to the task; references are read on demand.
This installation adds 26 skills and preserves the existing Colosseum skill.

Inventory checked against the repository manifests on 2026-10-10. Manifests and
lockfiles remain authoritative: importing a skill does not migrate dependencies,
authorize deployment/signing, or replace the user's instructions.

| Area | Skills | Repository boundary |
| --- | --- | --- |
| Rust idioms and async HTTP | `rust-best-practices`, `rust-web` | Cargo workspace; Tokio, Axum, Tower, reqwest, tracing, JWT/Argon2 |
| Database | `sqlx-sqlite` | Backend/Tauri SQLx 0.9, SQLite migrations and durable records |
| Native game | `bevy`, `egui` | Bevy 0.20, egui 0.36, bevy_egui 0.43.1; ECS, render/UI and input |
| Solana and Anchor | `solana-dev` | Program Anchor 1.1.2/Solana Rust 3.x; existing Kit/web3.js and JS Anchor boundaries |
| Delegated execution | `magicblock` | Ephemeral Rollups SDK 0.16.2; delegation, commit, sessions, cranks and fees |
| RPC and cost measurement | `triton-rpc` | Triton One/Yellowstone; cu_benchmark and backend RPC telemetry |
| Peer transport | `iroh` | Iroh 1.3, gossip, QUIC and local HTTP/3 adapters |
| Game synchronization | `braid` | Braid HTTP/Iroh resources, resume, durable acknowledgments and pending writes |
| Desktop shell | `tauri-architecture`, `tauri-rust`, `tauri-security`, `tauri-testing`, `tauri-release` | Tauri 2, native commands/capabilities; separate Bevy game and wallet bridge |
| Wallet authentication | `privy` | React Privy Solana/injected wallet adapters; player approval and exact signatures |
| Frontend | `vercel-react-best-practices`, `web-ui`, `vite`, `vitest` | Four React/TypeScript/Vite apps; Router, Motion, ECharts, icons and browser tests |
| Mobile | `android-native` | Kotlin/Gradle, GameActivity, JNI/NDK and Mobile Wallet Adapter |
| Containers | `docker-build-strategies`, `docker-compose-patterns` | Dockerfiles and Compose; BuildKit and container lifecycle |
| Operations | `ops` | Hetzner, Nginx, systemd, Prometheus/Grafana/Alertmanager and GitHub Actions |
| Packaging | `desktop-packaging` | PowerShell, NSIS, macOS signing; assets/bin engine and process ownership |
| Chess data and analysis | `chess-engine` | Stockfish UCI, nimzovich, Shakmaty, Lichess puzzles and anti-cheat workers |
| Startup research | `colosseum-copilot` | Existing skill preserved unchanged |

## Sources and compatibility

Imported skills and their supporting references come from:

- [Solana Foundation](https://github.com/solana-foundation/solana-dev-skill)
- [MagicBlock](https://github.com/magicblock-labs/magicblock-dev-skill)
- [Chris Gliddon's community Bevy collection](https://github.com/chrisgliddon/bevy-skills)
- [Community Tauri 2 collection](https://github.com/fxckcode/tauri-skills)
- [Apollo Rust practices](https://github.com/apollographql/skills)
- [Docker](https://github.com/docker/skills)
- [Anthony Fu's Vite/Vitest skills](https://github.com/antfu/skills)
- [Vercel React practices](https://github.com/vercel-labs/agent-skills)
- [Privy's published skill](https://docs.privy.io/skill.md)

GitHub imports use exact commits recorded in [skills-lock.json](../skills-lock.json).
Privy's published document is content-pinned by hash. Repository adaptations are
marked in the lockfile; their installed content hashes include those changes.
[NOTICE.md](NOTICE.md) retains upstream licensing and attribution.

The Bevy core guide originated at 0.19; its local entrypoint directs work to the
repo's 0.20 sources. The Vitest guide originated at 5.x while these apps use 4.x.
Vite examples may target a newer 8.x minor. React server/Next.js examples and
Privy server-wallet examples are conditional and do not describe this app's default.

Skills marked `metadata.provenance: docs-derived` are concise local guides written
from the linked primary documentation and this repository's integration boundaries.
They are not presented as publisher-authored skills. Related libraries are grouped
by their integration boundary rather than given redundant skills per dependency.

## Maintenance

Review source/version differences before updating an import; preserve local
compatibility notes and required references. Validate frontmatter, names, local
links and helper scripts, then recompute the installed folder hashes in the lockfile.
Keep application changes separate from skill installation.
