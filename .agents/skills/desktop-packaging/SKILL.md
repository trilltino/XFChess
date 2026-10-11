---
name: desktop-packaging
description: Maintain XFChess Windows PowerShell/NSIS and macOS packaging, native game/bridge lifecycle, asset lookup and signing. Use for launchers, installers, bundled engines and desktop release artifacts.
metadata:
  provenance: docs-derived
---

# Desktop packaging

Read `scripts/package.ps1`, `.github/workflows/release.yml`, `tauri/installer/`,
`tauri/macos/`, `build.rs` and `tauri/src/wallet/`. The Bevy game and Tauri wallet
bridge are distinct processes; the wallet may run in an external Chromium app window.

- Keep executable-relative resource lookup working from unrelated current directories.
  Windows Stockfish belongs at `assets/bin/stockfish.exe`, remains gitignored, and
  ships through the assets copy. NSIS must fail when that required payload is missing.
- Preserve legacy sibling-engine lookup for existing platform packages. Ensure
  each target gets its own executable format rather than another platform's engine.
- Launch the interactive game visibly; launch background bridge/helpers hidden.
  Pass the game PID and retain verified process ownership for lifecycle cleanup.
- Wallet windows must belong to their game's lifecycle. Preserve request IDs,
  pending cancellation and shutdown cleanup; never kill a user's shared browser
  process to close one wallet window.
- In PowerShell use literal paths and correct encoding. Before deleting or moving
  a staging tree, verify the absolute target and reject unexpected reparse points.
  Keep filesystem operations in one shell and avoid generated command strings.
- Verify installer install/upgrade/uninstall paths and resource presence. macOS
  code signing must include bundled executable engines before outer bundle signing.
- Separate unsigned local validation from signed release artifacts. Keep signing
  credentials outside staged assets and logs.

Check launcher syntax, actual resource lookup, engine UCI startup and installer
compilation. Test game-close cleanup with owned fixtures when lifecycle code changes.
Do not represent one platform's fixture test as a complete release matrix.

Sources: [PowerShell](https://learn.microsoft.com/powershell/),
[NSIS manual](https://nsis.sourceforge.io/Docs/),
[Tauri distribution](https://v2.tauri.app/distribute/),
[Apple signing](https://developer.apple.com/documentation/security/code-signing-services).
