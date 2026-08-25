# Publishing a Release

How to cut a Windows/macOS/Linux/Chrome OS release, what actually gets verified before
it ships, and the landmines already dug out of `release.yml`/`ci.yml` so
they don't have to be rediscovered.

## Cutting a release

```powershell
# Pushes the current branch + an auto-bumped (or explicit) version tag to both
# origin and private in one step
.\scripts\push_and_release.ps1
.\scripts\push_and_release.ps1 -Version v0.5.0
```

Or manually, origin only:

```bash
git tag v0.4.0
git push origin v0.4.0
```

This triggers `.github/workflows/release.yml`, which gates on
`verify-backend` (production health check) before building. It builds and
uploads installers for all three platforms in parallel:
`windows` (NSIS `.exe`), `linux` (`.tar.gz`), `macos` (`.dmg`).

**To test the pipeline without cutting a real release**, use the manual
trigger instead of a tag:

```bash
gh workflow run release.yml --ref <branch>
```

The version string comes from `GITHUB_REF_NAME` with a leading `v` stripped
and `/` replaced with `-` (branch names contain slashes; tags don't, so
this only matters for manual test runs). Artifacts from a manual run are
attached to the workflow run (`actions/upload-artifact`), not to a GitHub
Release — the "Attach to release" step only runs `if:
startsWith(github.ref, 'refs/tags/')`.

## Signing status (check before assuming a build is signed)

```bash
gh secret list
```

As of this writing: **no signing secrets are configured**. Both platforms
build and ship successfully, but:

- **Windows**: no `AZURE_TS_ACCOUNT` → the Azure Trusted Signing steps are
  skipped (`if: env.AZURE_TS_ACCOUNT != ''`). The `.exe`/installer ship
  unsigned.
- **macOS**: no `APPLE_CERTIFICATE` → `package_macos.sh` skips codesigning
  entirely and prints `No APPLE_SIGNING_IDENTITY set — skipping
  sign/notarize (unsigned .app, dev only)`. The `.dmg` ships unsigned and
  unnotarized (Gatekeeper will block it on end-user Macs without a manual
  right-click-Open bypass).

Required secrets, if/when signing is provisioned: `AZURE_TENANT_ID`,
`AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TS_ENDPOINT`,
`AZURE_TS_ACCOUNT`, `AZURE_TS_PROFILE` (Windows); `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD`, `APPLE_TEAM_ID` (macOS).

- **Android**: no keystore exists yet, deliberately — the `android` job builds
  and uploads a **debug-signed** APK only (`assembleDebug`, not
  `assembleRelease`), which is fine for device testing but cannot go to the
  dApp Store. Generating the release keystore is a one-way decision (the same
  key is required for every future update — losing it forfeits the ability to
  update the listing at all) and this repo has a documented history of a
  secret reaching public git (see the "Secret Exposure" incident), so it is
  not something to fabricate inside a CI file or generate speculatively. When
  ready: generate with `keytool` (see `mobile/app/build.gradle.kts`'s
  comments), store it out of the repo as `ANDROID_KEYSTORE_BASE64` +
  `ANDROID_KEYSTORE_PASSWORD` + `ANDROID_KEY_ALIAS` + `ANDROID_KEY_PASSWORD`
  secrets, add a signing config + `assembleRelease` step to the `android` job,
  and switch the `dapp-store-cli` publish step on.

## Watching a run

```bash
gh run list --workflow=release.yml --limit 5
gh api repos/trilltino/XFChess/actions/runs/<run-id>/jobs \
  --jq '.jobs[] | "\(.name): \(.status)/\(.conclusion)"'
# Full logs for one job (works even mid-run, unlike `gh run view --log`):
gh api repos/trilltino/XFChess/actions/jobs/<job-id>/logs
```

## Landmines already found and fixed (2026-07-21)

Both `ci.yml` and `release.yml` had apparently never once run to completion
before this pass — `--all-features` checks and full platform release builds
were blocked at the very first step (a missing apt package), so nothing
past that point had ever actually been exercised in CI. Fixing that one gap
peeled back 17 further issues, one wall at a time. None of these should
recur, but if a *new* instance of the same class shows up, recognize the
pattern:

**CI environment gaps**
- `tauri/`'s webkit2gtk chain needs Linux dev packages (`libwebkit2gtk-4.1-dev`
  and friends) that a plain Bevy project never would — `ci.yml`'s `check`/
  `clippy`/`test` jobs now install the same list `release.yml`'s Linux build
  already proved works. If a *new* crate gets added to the `solana` or any
  other feature and Check breaks with a `pkg-config`/`-sys` build-script
  error, check whether the new crate pulls in a system library that isn't
  in that apt list yet.
- `Test Suite` needs the on-chain program built first
  (`cargo build-sbf --manifest-path programs/xfchess-game/Cargo.toml`)
  before any test file that spins up `solana-program-test` will pass —
  those files say so in their own header comments (grep `Prereq:` under
  `programs/xfchess-game/tests/`).
- Git LFS: `actions/checkout` needs `with: { lfs: true }` anywhere the built
  binary actually *runs* (release.yml's smoke test, or a real user's
  install) — without it, `assets/**/*.{png,mp3,ttf,glb,obj}` check out as
  ~130-byte pointer stubs instead of real files, and the app crashes trying
  to parse them. `ci.yml`'s jobs don't need it (no test loads real asset
  files); anything that launches the actual binary does.

**Dependency graph**
- Watch for duplicate versions of `solana-program` in `Cargo.lock`
  (`grep -c '^name = "solana-program"' Cargo.lock` should be `1`). A crate
  with a loose, unbounded version requirement (`>=1.16`, no upper bound)
  can resolve independently to whatever's newest on crates.io instead of
  reusing the workspace's pinned version, and a newer major can have moved
  APIs the older crate's own source still expects. Fix with
  `cargo update -p <crate>@<bad-version> --precise <good-version>` — valid
  as long as the good version still satisfies every consumer's own range;
  don't `[patch.crates-io]` a version bump on the *same* registry, that's
  not what patch is for (needs a different source: git or path).
- Don't add a dependency "just in case" — `magic-resolver` sat in
  `Cargo.toml` for who knows how long as `optional = true` with zero call
  sites anywhere in `src/`/`crates/`, only surfacing as a `--all-features`
  build break. If you're not calling it yet, don't wire it into the
  manifest yet either.

**Bevy API drift**
- Feature-gated code that's `default = []`'d out (like `templeos`) doesn't
  get compiled by a normal `cargo check` — it can drift arbitrarily far out
  of sync with a pinned dependency's API and nothing will notice until
  someone finally builds with that feature on. `TextLayout::new_with_justify`
  → `TextLayout::justify`, `TextFont.font_size: f32` → `FontSize` enum, both
  in this exact bevy_text pin (`=0.19.0-rc.3`).

**Test isolation**
- Anything that calls `tracing_subscriber::fmt()....init()` installs a
  process-global subscriber and panics on a second call. `cargo test` runs
  tests in one process, concurrently — if more than one test in a file
  needs to exercise an init function like this, gate it behind
  `std::sync::Once` (see `tauri/src/utils/logging.rs`), don't call it
  directly from each test.
- Don't hand-write "random" 32-byte test keys for anything that's really an
  Ed25519 public key (`EndpointId::from_bytes(&[i as u8; 32])`) — only
  ~50% of arbitrary byte patterns decompress to a valid curve point.
  Derive a real key from a `SecretKey` seed instead
  (`SecretKey::from_bytes(seed).public()`), which accepts any input.

**Doctests** (none of these had ever run — `Test Suite` always died earlier)
- Inside a `///` doc-comment example, `crate::` refers to the *synthetic
  doctest binary's* root, not the crate the docs live in — reference the
  real crate name instead (`braid_http::...`, not `crate::...`).
- A fenced code block with no language tag defaults to being compiled as
  Rust. An ASCII architecture diagram (box-drawing characters) needs
  ` ```text `, not a bare ` ``` `. This can hide in a `README.md` that gets
  spliced into a crate's docs via `#![doc = include_str!(...)]`.

**Windows-specific**
- `stockfish.exe` is `.gitignore`'d — it never exists in a checkout on any
  platform, only after `release.yml`'s "Download Stockfish" step fetches it
  fresh. That download used to be best-effort (`continue-on-error: true`,
  NSIS `File /nonfatal`) so a flaky fetch would silently ship a release
  without the engine. As of 2026-08-03 it's mandatory: the download step has
  no `continue-on-error`, a "Verify Stockfish binary" step spawns it and
  checks for a `uciok` UCI handshake on all three platforms, and the NSIS
  `File` directive (and `package_macos.sh`'s copy) are hard failures if the
  binary is missing. If a future edit reintroduces best-effort semantics
  here, it's a regression, not a feature. Tauri's `bundle.resources` still
  has no "optional" mechanism, so don't list an optional/best-effort file
  there either.
- PowerShell: `$env:ProgramFiles(x86)` is invalid — a bareword `$env:`
  reference can't be followed by `(x86)`, it parses as a call expression.
  Needs brace syntax: `${env:ProgramFiles(x86)}`.
- Branch names can contain `/`; a `/` inside an output filename
  (`XFChess-linux-x86_64-${version}.tar.gz`) gets parsed as a path
  separator, and the containing directory won't exist. Only bites manual
  `workflow_dispatch` test runs off a branch — real tags (`v0.4.0`) never
  have slashes — but the version-derivation step sanitizes it anyway now.

**macOS-specific**
- `tauri/icons/icon.icns` was never checked in (only `icon.ico` +
  `32x32.png`/`128x128.png` exist), so every DMG shipped with the generic
  default Dock icon and `package_macos.sh` only printed a `WARN`. As of
  2026-08-03 the script generates `icon.icns` on the fly from
  `128x128.png` via `sips`/`iconutil` (both macOS-builtin, no extra CI
  deps) when no pre-made `.icns` is checked in. It's upscaled from 128px so
  it won't be crisp at 512/1024 — drop a real `icon.icns` (or a larger
  source PNG) into `tauri/icons/` whenever a designed icon exists, and the
  script will prefer it automatically.

## Verifying this guide is still accurate

If `release.yml`/`ci.yml` fail again, check first whether it's actually a
*new* issue before assuming it's one of the above resurfacing — grep this
file for the symptom, but trust the actual error over this document if they
disagree. Update this list when a genuinely new class of failure gets
fixed; delete an entry if the underlying code path it describes no longer
exists.
