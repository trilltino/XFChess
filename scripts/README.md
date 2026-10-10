# Commands

Use the root `justfile` as the command menu on Windows. Install with
`cargo install just`, then run `just` from the repository to list tasks.
Scripts here implement workflows that need more than a few commands.
The RPC load test requires PowerShell 7 (`pwsh`) for parallel requests.

| Task | Command |
| --- | --- |
| Local stack / desktop admin | `just dev` / `just admin` |
| Debug Rust builds / full local release build | `just build` / `just build-all` |
| Android unsigned release APK / Linux game folder | `just build-android` / `just build-linux` |
| Solana binary / Docker Anchor build with IDL | `just build-program` / `just build-program-docker` |
| Solana deploy via native Anchor / Docker | `just deploy-devnet` / `just deploy-program-docker` |
| Verify devnet program matches local build | `just verify-program` (add `-Build` to rebuild) |
| Local Windows package with backend | `just package local` (add `-Debug` for a fast build) |
| Windows package using a remote backend | `just package dev` (add `-BackendUrl https://your-host`) |
| Push a release tag / release then deploy VPS | `just release` / `just release -Deploy` |
| Stop local services / close wallet popups only | `just kill` / `just kill-wallets` |
| Eight tournament participants / engine SPRT match | `just dev8` / `just engine-match` |
| Signing diagnostics / RPC load test | `just wallet-logs` / `just test-triton` |
| Optional local monitoring | `just monitoring` |

`build-all` includes the wallet, admin and web UIs plus the local Rust release
binaries. Build the on-chain program separately with `build-program`.
Android builds require the SDK, NDK, JDK and cargo-ndk described in
[the Android runbook](../docs/runbooks/android-release.md).

Packages land in `release/local` or `release/dev`, with a `launch.bat` entry
point. Local packages include a copy of `backend/.env` and supply missing
development secrets in the staged copy; keep that package private. Remote
packages default to `https://xfchess.com`. Stockfish is copied if already
present; installable public releases are built by CI.

`release` defaults to pushing the branch and tag. `-Deploy` waits for the
release workflow, checks its Chrome OS asset, then calls `ops/scripts/deploy.ps1`.
Use `-DryRun` to preview; `-Version vX.Y.Z` selects a version. The scripts
remain directly callable with PowerShell for parameters containing spaces.

The former `build*.bat`, `run_offline.bat`, and `start-tournament-admin.bat`
entry points are replaced by the recipes above. The two Anchor Docker
scripts are merged into `anchor.ps1`; both package variants into `package.ps1`;
both release variants into `release.ps1`; wallet cleanup into
`kill_stale_xfchess_dev.ps1 -WalletOnly`.
