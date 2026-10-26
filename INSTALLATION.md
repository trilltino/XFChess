# Installing XFChess

Latest release: **v0.0.2-closedbeta**
<https://github.com/trilltino/XFChess/releases/latest>

| Platform | Download | Status |
|---|---|---|
| Windows 10/11 (64-bit) | `XFChess-Setup-0.0.2-closedbeta.exe` | Available |
| macOS 10.15+ (Intel and Apple Silicon) | `XFChess-0.0.2-closedbeta.dmg` | Available |
| Linux (x86_64) | `XFChess-linux-x86_64-0.0.2-closedbeta.tar.gz` | Available |
| Chrome OS (x86_64, Linux enabled) | `XFChess-chromeos-x86_64-0.0.2-closedbeta.tar.gz` | Available |
| Android | none yet | In development, see [Android](#android) |

The game ships with its Stockfish engine bundled. You do not need to install a chess engine.

The builds are **unsigned** at this stage, so Windows and macOS will warn you the first time you open them. The steps below say what to click.

## What you need for online and wagered games

Offline play and playing the AI work without any account. Online play needs an internet connection. Wagered games also need a Solana wallet:

- A desktop browser, **Chrome or Edge**, with the **Phantom** or **Solflare** extension installed. The game opens a small browser window for wallet approvals.
- Some devnet SOL while the beta runs on devnet. Get it from <https://faucet.solana.com>.

---

## Windows

1. Download `XFChess-Setup-0.0.2-closedbeta.exe` from the release page.
2. Run it. If Windows SmartScreen shows "Windows protected your PC", click **More info**, then **Run anyway**.
3. Accept the administrator prompt. The installer puts the game in `C:\Program Files\XFChess`.
4. Leave **Launch XFChess** ticked on the last page, or start it later from the Start menu or the desktop shortcut.

The Start menu also has **XFChess (2nd Instance)**. Use it to run two copies on one PC, for example to test an online game against yourself.

**Uninstall:** Settings > Apps > Installed apps > XFChess > Uninstall.

**Troubleshooting**
- *The window does not appear:* the installer stops any running copy first. If you still see nothing, end `xfchess.exe` and `xfchess-tauri.exe` in Task Manager and start the game again.
- *Wallet approvals do not open:* install Chrome or Edge, then the Phantom or Solflare extension, and sign in to it once.

---

## macOS

1. Download `XFChess-0.0.2-closedbeta.dmg`.
2. Open it and drag **XFChess** onto the **Applications** shortcut.
3. The first launch is blocked because the app is not notarized by Apple. Either:
   - Right-click (or Control-click) **XFChess** in Applications, choose **Open**, then **Open** again; or
   - Open **System Settings > Privacy & Security**, scroll down, and click **Open Anyway** next to the XFChess message.

   If macOS says the app is damaged, run this once in Terminal and try again:
   ```sh
   xattr -dr com.apple.quarantine /Applications/XFChess.app
   ```
4. Later launches work normally.

**Apple Silicon (M1 and newer):** the build is Intel (x86_64) and runs through Rosetta 2. If macOS offers to install Rosetta, accept. To install it ahead of time:
```sh
softwareupdate --install-rosetta --agree-to-license
```

**Uninstall:** drag XFChess from Applications to the Trash.

---

## Linux

Tested for x86_64 with a desktop session (X11 or Wayland) and working graphics drivers (OpenGL or Vulkan).

1. Install the runtime libraries. On Debian or Ubuntu:
   ```sh
   sudo apt-get update
   sudo apt-get install -y libasound2 libudev1 libxkbcommon-x11-0 \
     libgl1 libvulkan1 mesa-vulkan-drivers \
     libwebkit2gtk-4.1-0 libayatana-appindicator3-1 libxdo3
   ```
   On Fedora: `sudo dnf install alsa-lib systemd-libs libxkbcommon-x11 mesa-libGL mesa-vulkan-drivers webkit2gtk4.1 libayatana-appindicator-gtk3 xdotool`. Package names differ by distro; the libraries listed are what the game and its wallet helper link against.
2. Download and unpack the tarball:
   ```sh
   tar -xzf XFChess-linux-x86_64-0.0.2-closedbeta.tar.gz
   cd linux
   ./launch.sh
   ```
   `launch.sh` starts the wallet helper (`xfchess-tauri`) and then the game (`xfchess`). Keep all the files in that folder together. The game finds its `assets/`, `wallet-ui/` and `stockfish` next to the executables.
3. If a file is not executable after unpacking: `chmod +x launch.sh xfchess xfchess-tauri stockfish`.

**Uninstall:** delete the unpacked folder.

---

## Chrome OS

The Chrome OS download is the same Linux build, run inside Chrome OS's Linux environment (Crostini). It needs a Chromebook with **x86_64** and Linux support. ARM Chromebooks are not supported by this build.

1. Turn on Linux: **Settings > Advanced > Developers > Linux development environment > Turn on**.
2. Download `XFChess-chromeos-x86_64-0.0.2-closedbeta.tar.gz`. Move it into **Linux files** with the Files app.
3. In the Linux terminal, follow the [Linux](#linux) steps. The Debian packages in step 1 apply, because Crostini is Debian.
4. If the window does not appear or rendering is slow, your Chromebook's GPU support for Linux apps may be limited. Check **Settings > Advanced > Developers** for GPU or Vulkan options, and make sure Chrome OS is up to date.

For wallet approvals, install Phantom or Solflare in the Chrome OS browser, not in the Linux container.

---

## Android

There is no Android download yet. The release workflow skips the Android job on purpose until signing and store delivery are in place. The full plan, including how Solana Mobile Seeker phones and ordinary Android phones will be handled, is kept outside the repo in `Documents\ANDROID_PARITY_PLAN.md`.

If you want to build the current work-in-progress APK yourself, you need:

| Tool | Version |
|---|---|
| Rust (stable) with the Android target | `rustup target add aarch64-linux-android` |
| JDK | 17 |
| Android SDK | any recent platform |
| Android NDK | `27.2.12479018` |
| CMake (via the SDK manager) | `3.22.1` |
| `cargo-ndk` | `cargo install cargo-ndk` |
| `just` (optional) | `cargo install just` |

Install the NDK and CMake with the SDK manager:
```sh
sdkmanager --install "ndk;27.2.12479018" "cmake;3.22.1"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
```

Then, from the repo root on Windows:
```sh
just build-android
```
This fetches the Android Stockfish binary, cross-compiles the game library for `arm64-v8a`, and assembles an **unsigned** release APK in `mobile/app/build/outputs/apk/release/`. An unsigned APK cannot be installed as is. Sign it with your own key first (`apksigner`), or use a debug build from Android Studio, then install with:
```sh
adb install -r path/to/app.apk
```
Enable **Developer options > USB debugging** on the phone for `adb`. This build is for development only. Wallet connection on Android is not finished, so wallet and wagered play are not expected to work on it yet.

---

## Building from source (all desktop platforms)

You only need this if you want to change the code.

```sh
git clone https://github.com/trilltino/XFChess.git
cd XFChess
cargo install just
(cd tauri/wallet-ui && npm ci && npm run build)
cargo build --release --bin xfchess --features solana
cargo build --release -p xfchess-tauri
```
Needs Rust stable, Node.js 20, and a C toolchain. On Windows, building the Anchor program also needs a prebuilt Win64 OpenSSL and `OPENSSL_NO_VENDOR=1`. Run `just` to list the other commands.

---

## Verifying a download

Each release lists the file sizes on its page. To compare what you downloaded:
```sh
gh release view v0.0.2-closedbeta --repo trilltino/XFChess --json assets --jq '.assets[] | "\(.name) \(.size)"'
```
Only download from the official repository above.
