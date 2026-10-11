# Development

The [contribution guide](../../CONTRIBUTING.md) covers setup and pull requests.
Run commands from the repository root.

The `justfile` uses Windows PowerShell and several `.exe`/`.bat` launch paths.
On Windows, `just dev` launches the local stack; `just dev2` launches two clients.
On Linux/macOS, use Cargo and npm commands for each component; the Windows launch
recipes are not portable.

| Component | Source and guide | Focused check |
| --- | --- | --- |
| Bevy game | [src/](../../src/lib.rs), [UI](../../src/ui/README.md) | `cargo check --bin xfchess --features solana` |
| Axum backend | [backend/](../../backend/README.md) | `cargo test -p backend --lib` |
| Multiplayer | [Networking](../architecture/networking.md), [models](../../specs/README.md) | Affected crate tests and `just check-specs` |
| Solana program | [Program](../architecture/xfchess-game-crate.md) | Build SBF, then `cargo test -p xfchess-game` |
| Desktop wallet | [Tauri shell](../../tauri/src/README.md) | `cargo test -p xfchess-tauri --bin xfchess-tauri` |
| Operator panel | [Admin panel](../../tauri/tournament-admin/README.md) | In its directory: `npm test` and `npm run build` |
| Website | [xfchessdotcom/](../../xfchessdotcom/README.md) | Build and relevant Playwright tests |
| Cost/capacity runner | [cu_benchmark](../../crates/cu_benchmark/README.md) | `cargo test -p cu_benchmark --all-targets` |
| Android | [Release runbook](../runbooks/android-release.md), [mobile/](../../mobile/) | Native build and device checks |

Install npm dependencies with `npm ci` in the frontend being changed. Node 20
must be at least 20.19 for the current Vite setup. Native Linux builds need the
system development libraries listed in the Linux jobs of
[CI](../../.github/workflows/ci.yml) and [release](../../.github/workflows/release.yml).
Use workspace dependency pins when reading framework examples.

Use `.env.example` for local configuration. Placeholder local keys are not
production credentials. Guest/offline play does not require a Solana deployment.
Lichess integrations live in the backend auth and puzzle modules; use the upstream
[API reference](https://lichess.org/api) and [puzzle data](https://database.lichess.org/).

See [verification](verification.md) for release boundaries and
[backlog](backlog.md) for work requiring further validation.
