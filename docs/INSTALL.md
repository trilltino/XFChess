# Installing XFChess

End-to-end install instructions for players downloading a prebuilt release from
[GitHub Releases](https://github.com/trilltino/XFChess/releases) — no Rust
toolchain or build step required. If you're setting up a dev environment instead,
see the [Quick Start](../README.md#quick-start) in the root README.

Every release publishes three assets, one per platform, all built by the same
CI pipeline (`.github/workflows/release.yml` — see [PUBLISHING.md](PUBLISHING.md)
for how they're cut):

| Platform | Asset | Format |
|----------|-------|--------|
| Windows | `XFChess-Setup-<version>.exe` | NSIS installer |
| macOS | `XFChess-<version>.dmg` | Disk image |
| Linux | `XFChess-linux-x86_64-<version>.tar.gz` | Tarball |

Single-player vs the built-in engine works fully offline. Online multiplayer,
tournaments, and wager play need an internet connection (the app talks to
`https://xfchess.com`) and a Solana wallet (e.g. Phantom or Solflare) for
on-chain features.

**Signing status:** as of this writing, builds are unsigned (see
[PUBLISHING.md](PUBLISHING.md#signing-status-check-before-assuming-a-build-is-signed)).
That means Windows SmartScreen and macOS Gatekeeper will both flag the app on
first run — this is expected, not a sign of a compromised download. The
bypass steps for each are below.

## Windows

1. Go to the [Releases page](https://github.com/trilltino/XFChess/releases)
   and download `XFChess-Setup-<version>.exe`.
2. Double-click it. Windows SmartScreen will show **"Windows protected your
   PC"** because the binary isn't code-signed yet. Click **More info**, then
   **Run anyway**.
3. The installer requests admin rights (it installs to
   `C:\Program Files\XFChess`) — accept the UAC prompt.
4. Click through the install wizard (Welcome → choose install directory →
   Install). On the finish page, leave **Launch XFChess** checked and click
   **Finish**.
5. First launch starts two processes: `xfchess-tauri.exe` (the wallet bridge,
   runs in the background) and `xfchess.exe` (the game window). Both are also
   reachable later from the **XFChess** Start Menu folder or the desktop
   shortcut, which launch them together via `launch.bat`.

**Uninstalling:** Settings → Apps → XFChess → Uninstall, or run
`C:\Program Files\XFChess\uninstall.exe` directly.

## macOS

1. Download `XFChess-<version>.dmg` from the
   [Releases page](https://github.com/trilltino/XFChess/releases).
2. Double-click the `.dmg` to mount it, then drag **XFChess.app** into the
   **Applications** folder shown in the same window. Eject the mounted image
   once the copy finishes.
3. The app is unsigned and unnotarized, so Gatekeeper blocks a normal
   double-click launch (**"XFChess" can't be opened because it is from an
   unidentified developer** / **is damaged and can't be opened**). To bypass:
   - Right-click (or Control-click) **XFChess.app** in Applications → **Open**
     → confirm **Open** in the dialog. This only needs to be done once; later
     launches work normally via Launchpad/Spotlight.
   - If that dialog doesn't offer an Open button, instead go to
     **System Settings → Privacy & Security**, scroll to the Security
     section, and click **Open Anyway** next to the XFChess block message,
     then confirm.
4. On launch, the app starts the wallet-bridge helper (`xfchess-tauri`)
   alongside the game window automatically.

**System requirement:** macOS 10.15+. Current builds are produced on GitHub's
`macos-latest` runner, which is Apple Silicon (arm64) — there is no
universal/Intel binary yet, so the release won't run on an Intel Mac.

**Uninstalling:** drag `XFChess.app` from Applications to the Trash.

## Linux

1. Download `XFChess-linux-x86_64-<version>.tar.gz` from the
   [Releases page](https://github.com/trilltino/XFChess/releases).
2. Extract it and enter the folder:
   ```bash
   tar -xzf XFChess-linux-x86_64-<version>.tar.gz
   cd linux
   ```
3. **Prerequisite:** the wallet-bridge binary is a Tauri/WebKitGTK app and is
   dynamically linked against system libraries that aren't bundled in the
   tarball. Install them first (Debian/Ubuntu):
   ```bash
   sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-2 libgtk-3-0
   ```
   (Package names vary on Fedora/Arch — look for `webkit2gtk`,
   `libappindicator`/`libayatana-appindicator`, and `gtk3` equivalents.)
4. Run both binaries together:
   ```bash
   ./launch.sh
   ```
   This starts `xfchess-tauri` (wallet bridge) in the background, then
   `xfchess` (the game window) in the foreground. If the extracted files
   aren't marked executable, run `chmod +x xfchess xfchess-tauri launch.sh`
   first.
5. There's no system-wide install step or `.desktop` entry yet — launch
   `./launch.sh` from the extracted folder each time, or symlink/alias it
   yourself.

**Uninstalling:** delete the extracted folder.

## Troubleshooting

- **The app won't connect / multiplayer doesn't load anything:** confirm
  `https://xfchess.com/health` is reachable from your machine — the app has
  no offline fallback for online features.
- **Windows/macOS security dialogs keep reappearing on every launch:** you
  bypassed the wrong binary, or a later update reset the exception. Repeat
  the bypass step above on the current install.
- **Chess engine feels weak/strong:** difficulty is adjustable in-game
  (Level 1–8); the default engine is the built-in `XFChessEngine`
  (native Rust, no setup needed). Stockfish is only used if explicitly
  selected as the AI engine and a `stockfish` binary is bundled — the
  current release pipeline doesn't guarantee one ships in the Windows/macOS
  packages.
