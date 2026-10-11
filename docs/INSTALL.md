# Installing XFChess

Download the asset for your platform from [GitHub Releases](https://github.com/trilltino/XFChess/releases).
For source builds, see [contributing](../CONTRIBUTING.md).

| Platform | Asset |
| --- | --- |
| Windows | `XFChess-Setup-<version>.exe` |
| macOS | `XFChess-<version>.dmg` |
| Linux | `XFChess-linux-x86_64-<version>.tar.gz` |
| Chrome OS/Crostini | `XFChess-chromeos-x86_64-<version>.tar.gz` |
| Android | Check the release/workflow for an APK; the current CI artifact is unsigned |

Offline single-player needs no wallet. Online features need network access;
on-chain features also need wallet authorization. The game and desktop wallet
bridge run together through the packaged launcher.

## Windows

Run the installer, then use the XFChess desktop or Start Menu shortcut.
If Windows blocks an unsigned build, check the download source and signature
before using More info / Run anyway. Signing depends on the release configuration;
the dialog alone does not establish whether a download is trustworthy.
Uninstall through Settings / Apps or the installed `uninstall.exe`.

## macOS

Mount the DMG, drag XFChess.app into Applications, eject the image and launch it.
Check the release's architecture and signing/notarization information. If you
choose to open a verified unsigned build, use the system's Open Anyway workflow.
Uninstall by removing the app from Applications.

## Linux and Chrome OS

Chrome OS requires the Linux development environment (Crostini) and a matching CPU
architecture. Both tarballs contain a `linux/` directory.

```bash
tar -xzf XFChess-linux-x86_64-<version>.tar.gz
cd linux
sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-2 libgtk-3-0
chmod +x xfchess xfchess-tauri launch.sh
./launch.sh
```

Runtime package names vary by distribution. Launch from the extracted directory
so bundled assets can be found. Uninstall by deleting that directory.

## Android

The native app targets Android 12+ on arm64-v8a. Current CI builds an unsigned
release APK; it needs signing before installation/distribution. Use a debug
build for device testing. See [the Android runbook](runbooks/android-release.md).

## Troubleshooting and logs

- Test `https://xfchess.com/health` if online features cannot connect.
- Choose the engine/difficulty in the Play vs Computer setup; packaged desktop
  builds include Stockfish.
- Settings / Help & Support / Export logs writes a diagnostic text file locally.
  Review it before attaching it to an issue.
- Profile logs live under the chosen profile directory, usually
  `Documents/xfchess/profiles/<name>/logs`. Before profile creation, check
  `%LOCALAPPDATA%\xfchess\logs` on Windows,
  `~/Library/Application Support/xfchess/logs` on macOS or
  `~/.local/share/xfchess/logs` on Linux.

Include platform, release version, reproduction steps and relevant redacted logs.
