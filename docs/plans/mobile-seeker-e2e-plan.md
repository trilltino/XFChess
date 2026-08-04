# Mobile (Android + Solana Seeker) — E2E Plan

**Date:** 2026-07-29
**Status:** Draft — proposed, not started
**Trigger:** User request for a deep plan to run the full game on Android/Seeker, including wallet signing e2e and a touch-input difficulty assessment.

## Context

XFChess today is desktop-only across every layer that matters for "playing the
game": the Bevy client renders a real 3D board and drives input entirely
through mouse + keyboard; wallet signing works by having the Tauri desktop
shell spawn an OS Chrome `--app` popup and talk to the game process over a raw
localhost TCP socket. None of that survives a port to Android as-is — there is
no spawnable "Chrome popup window" on Android, no guarantee of a long-lived
loopback listener across app backgrounding, and no touch backend wired into
input.

The good news: two of the three hardest problems are already substantially
solved elsewhere in this codebase, just not connected to the native client:

1. **Mobile Wallet Adapter (MWA) already works.** `xfchessdotcom/src/App.tsx`
   has `@solana-mobile/wallet-adapter-mobile` fully wired into the wallet
   list (`SolanaMobileWalletAdapter` alongside Phantom/Solflare). This is the
   real, Solana-Mobile-blessed way to sign on Android/Seeker — it does the
   Intent + local-websocket handshake with whatever wallet app (or Seeker's
   Seed Vault-backed wallet) is installed. It is currently only reachable
   from the plain web build, disconnected from the native game client.
2. **The global session-key flow is already transport-agnostic and already
   minimizes signing to "once, ever."** Per [global-session-flow-fix-plan.md](global-session-flow-fix-plan.md)
   and `src/multiplayer/solana/global_session_manager.rs`, a player signs
   one `authorize_global_session` transaction, then every move for the next
   30 days / 200 games is either signed server-side by the delegated session
   key, or by a locally-persisted encrypted session key — no wallet
   round-trip per move. Backend endpoints for this are plain HTTP/JSON
   (`backend/src/signing/routes/global_session.rs`), so they don't care
   whether the caller is the native Bevy client, a Tauri desktop shell, or a
   WASM build running in a mobile browser.

That second point matters enormously for scope: **the mobile wallet-signing
problem is "get MWA to run once (rarely twice)," not "make MWA work from
inside a real-time game loop."** That's a much smaller, much lower-risk
surface than it first sounds.

The bad news, and the actual hard part: the native Bevy client has **zero**
touch input, Android build scaffolding, or non-desktop signing path (see full
audit in the "Current state" section below). Getting *that* client onto
Android is real engineering, even if touch mechanics specifically turn out to
be one of the easier pieces (see "How hard is touch," which answers your
direct question).

---

## 1. Current state (inventory)

| Layer | Desktop today | Android/Seeker gap |
|---|---|---|
| Rendering | Real 3D (`Mesh3d`/`StandardMaterial`, `Camera3d`), Bevy 0.19, winit 0.30 | No Android build target configured anywhere (`Cargo.toml` has no `android-*` feature; no `cargo-ndk`/`cargo-apk` setup) |
| Input | `bevy_picking` observers (`Pointer<Click>`, `Pointer<DragStart/Drag/DragEnd>`) gated on `PointerButton::Primary` only (`src/game/systems/input.rs`); hover-based cursor styling (`src/input/pointer.rs`) | No touch backend registered; hover has no touch equivalent |
| Camera | RTS-style: **WASD pan, Q/E rotate (keyboard), scroll-wheel zoom** (`src/game/systems/camera.rs`) — no mouse-drag orbit | No touch/gesture equivalent (drag-to-pan, pinch-to-zoom, rotate) exists |
| Wallet signing (native client) | Bevy → raw TCP → Tauri → spawns **real OS Chrome `--app` popup** → talks to `window.solana`/`window.phantom` extension directly (`src/multiplayer/solana/tauri_signer.rs`, `tauri/src/main.rs`) | Nothing here survives Android: no spawnable browser popup, no extension wallets, unreliable loopback across backgrounding |
| Wallet signing (web) | `@solana-mobile/wallet-adapter-mobile` fully wired (`xfchessdotcom/src/App.tsx:7,58-75`) | Already works for mobile browsers in principle — just disconnected from the game |
| Session-key flow | Global session key: one signature ever, HTTP/JSON backend (`backend/src/signing/routes/global_session.rs`), AES-256-GCM encrypted local file on desktop | Transport-agnostic already — just needs a non-filesystem storage backend on mobile |
| Tauri wrapper | Tauri 2.11, desktop-only `tauri.conf.json` (fixed 400×660 window, desktop bundle targets) | No `gen/android`/`gen/apple` scaffolding exists; `tauri android init` has never been run |
| Distribution | Windows/Mac/Linux release pipeline (see [Distribution Pipeline](../../CLAUDE.md) memory) | No Google Play or Solana dApp Store presence |

---

## 2. Architecture decision: two viable paths

**Revision note (2026-07-29):** the original version of this plan
recommended WASM-first. That was wrong — or at least, it didn't account for
*why* wasm32 was blocked (browser-sandbox restrictions with no workaround)
versus what actually blocks native Android (toolchain setup + one bounded
bridge component, both solvable). Corrected below; native Android is now the
recommended primary path.

### Option A — Native Bevy on Android (recommended primary path)

Compile `src/` for `aarch64-linux-android` (Bevy's official `examples/mobile`
pattern: `cargo-ndk` + a thin Gradle/Android project hosting a
`GameActivity`). Unlike the wasm32 target, **Android is a real POSIX/Linux-kernel
OS, not a browser sandbox** — the three items that hard-blocked wasm32 in §2's
original analysis don't block Android the same way:

- `openssl` (vendored) — `cargo-ndk` cross-compiles vendored OpenSSL via the
  NDK's clang toolchain routinely; this is a known, if occasionally
  version-sensitive, pattern, not a structural block.
- `iroh`/`braid-iroh`/`iroh-gossip` (raw UDP/QUIC via `quinn`) — Android
  exposes real POSIX sockets; there's no browser-style sandbox restriction
  against raw UDP. Iroh P2P multiplayer should port close to as-is.
- `solana-sdk`/`solana-client` (tokio + native-TLS RPC) — Android is a
  Tier-2 Rust target with standard networking support; this is the same kind
  of cross-compile Linux/Mac/Windows already get, just a fourth target
  triple.

None of this is *verified* against this exact dependency graph yet (see §6 —
that's the first spike), but there's no structural reason to expect it to
fail the way wasm32 provably does.

**The one piece that genuinely doesn't fall out of "just recompile," on
Android or anywhere:** wallet signing. Mobile Wallet Adapter has official
implementations only in Kotlin (`mobile-wallet-adapter-clientlib-ktx` —
confirmed via Solana Mobile's own `docs.solanamobile.com/android-native`
guide, which is written in Kotlin, not Rust) and JS
(`@solana-mobile/wallet-adapter-mobile`, already in `xfchessdotcom`). No
mature Rust MWA client exists for native Android. So the signing handshake
specifically needs a small bridge — either a thin Kotlin shim called via
JNI, or an embedded WebView running the already-built-and-tested JS MWA code
from `xfchessdotcom`/`tauri/wallet-ui`, invoked via Tauri's `invoke`/event
IPC (running Tauri in Android mode via `tauri android init`) instead of the
desktop's raw TCP socket. Crucially, **this bridge only needs to cover the
signing handshake itself** — `solana-chess-client` keeps doing all other
chain RPC natively, since (unlike wasm32) nothing structurally prevents it
from running on Android. The WebView/Kotlin surface is foregrounded only for
the rare global-session-authorization moment; gameplay renders full-native
the rest of the time, with real P2P multiplayer intact.

**Pros:** ~95% of the existing codebase — rendering, game logic, on-chain RPC,
P2P multiplayer — plausibly cross-compiles rather than needing rewrites; best
performance/fidelity, matches the desktop experience closely; the "just
another OS release" framing genuinely holds for most of the app, unlike wasm32.
**Cons:** real toolchain setup (NDK, `cargo-ndk`, Gradle project, following
Bevy's `examples/mobile` template); Android app-lifecycle handling for a
native game process (backgrounding, Doze mode affecting Iroh QUIC sockets,
`GameActivity` surface recreation on rotation/resume) is new work with no
desktop equivalent; the MWA bridge, while small in scope, is still a new
JNI-or-WebView component to build and harden; nothing here is verified against
this exact dependency graph yet — first spike is confirming iroh/solana-sdk/
openssl-vendored actually build and run for `aarch64-linux-android`.

### Option B — Bevy → WASM, embedded in the existing web app (secondary / companion path)

`webgl2` is already an enabled Bevy feature in the root `Cargo.toml`, and
rendering/input/game-logic/the AI engine are all realistically portable — but
**the game client does not compile for `wasm32-unknown-unknown` today**, and
this is not just a matter of missing polyfills. Three concrete blockers exist
in the root `Cargo.toml`, none behind a feature flag:

- `openssl = { version = "0.10", features = ["vendored"] }` (line 248) —
  vendored OpenSSL is a C library; it has no `wasm32-unknown-unknown` target
  at all. This alone fails the build before anything else is touched.
- `iroh`, `braid-iroh`, `iroh-gossip` (lines 239-241, 250) — **unconditional**
  dependencies (unlike Solana, which is correctly gated behind
  `--features solana`). Their transport is raw UDP/QUIC via `quinn`, and
  browsers do not expose raw UDP sockets to JS/WASM under any circumstances —
  this isn't a missing-crate-feature problem, it's a browser sandbox
  restriction. **Multiplayer as currently architected cannot run in a browser
  tab, full stop.**
- `solana-sdk`/`solana-client` (correctly `optional = true`, gated behind
  `--features solana`) use a tokio + native-TLS RPC client stack that also
  doesn't target wasm32-browser as-is — but because they're properly
  feature-gated, a wasm build can at least omit `--features solana` and still
  link, unlike the two items above.

Getting *anything* to compile for wasm32 requires first splitting the first
two out via `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`. Doing
only that gets you a wasm build with **no on-chain integration and no
multiplayer** — i.e. local/vs-AI chess only. That is not "the same game," so
recovering full parity is two separate, real ports, not configuration work:

1. **Wallet/chain integration.** `solana-chess-client`'s native RPC calls
   can't run in wasm at all — not just the rare auth-signing moment
   originally scoped in §3, but *every* chain read/write a game performs
   (submitting moves, reading game/tournament state). The fix is a
   `wasm-bindgen` bridge where Rust asks JS to perform each chain operation
   via the JS context's `@solana/web3.js`/wallet-adapter (already present in
   `xfchessdotcom`) and hands the result back — a real, if mechanical,
   parallel implementation of what `solana-chess-client` does natively, not
   a one-off signing shim.
2. **Multiplayer transport.** Iroh P2P has no browser equivalent. Real-time
   move delivery would need to reroute through a WebSocket relay via the
   backend (which already exists as a relay hub — see `crates/CLAUDE.md`'s
   note that the backend holds in-memory P2P relay state) instead of direct
   peer-to-peer. This is a **permanent architectural divergence** from
   desktop, not a todo-list item to close later: WASM multiplayer would be
   server-relayed, desktop stays P2P direct.

Once compiling, mount the result on a `<canvas>` inside `xfchessdotcom` and
ship to mobile either as a plain installable **PWA** (Add to Home Screen —
the pattern Solana Mobile designed `wallet-adapter-mobile` for, works on
Seeker's stock Android Chrome with zero app-store review) or the same content
wrapped by **Tauri Android** for a "real app" listing.

**Pros:** rendering/input/game-logic/AI-engine layer is genuinely cheap to
port; reuses existing, already-tested MWA wiring for the *wallet-adapter
plumbing itself*; ships to Seeker/Android *and* iOS Safari *and* desktop
browsers from one build; no Android Studio/NDK/Gradle toolchain needed;
zero app-store review for the PWA form.
**Cons:** the wallet/chain integration and multiplayer transport are real,
permanent rewrites, not recompiles — Iroh P2P has no browser equivalent at
all (not "hard," *unavailable*), and every chain RPC call needs a JS-bridge
reimplementation, not just the signing moment. This is strictly more new
engineering than Option A's single bounded MWA bridge, for a *worse* ceiling
(no true P2P, browser-sandboxed performance).

### Recommendation

**Ship Option A (native Android) as the primary path.** It's the only route
that gets real P2P multiplayer and native on-chain RPC without a rewrite, and
the one piece that isn't "free" either way — the MWA signing bridge — is
smaller in Option A (just the handshake) than in Option B (all chain traffic).
First validation step: confirm `iroh`, `solana-sdk`/`solana-client`, and
`openssl` (vendored) actually build and run for `aarch64-linux-android` in
this exact dependency graph — that's the load-bearing assumption everything
else rests on, analogous to (but more likely to succeed than) the wasm32
compile spike below.

Keep Option B in scope as a **companion path, not a competing v1**: it's
low-cost given `xfchessdotcom` already has MWA wired, useful for a
lightweight web-playable demo/spectator mode or iOS reach without App Store
review friction — but treat it as strictly additive, not the way to reach
full game parity on Android.

The rest of this plan (§3 on) documents both paths' details, since the
wallet-signing sequence and touch-input work are needed either way; §2 above
is the part that changed.

---

## 3. Wallet signing E2E (Option B)

```
Player opens xfchess.com (PWA/Tauri-Android) on Seeker
        │
        ▼
Local check: encrypted global session key present in IndexedDB
   and not expired (30d / 200 games)?
        │
    ┌───┴───┐
   yes      no
    │        │
    │        ▼
    │   "Connect Wallet" → WalletSelectionModal
    │        │
    │        ▼
    │   SolanaMobileWalletAdapter.connect()
    │        │  (Android Intent: solana-wallet://, local WS handshake)
    │        ▼
    │   Wallet app foregrounds (Phantom / Solflare / Seed Vault
    │   wallet on Seeker) → user approves via biometric/PIN
    │        │
    │        ▼
    │   Client builds + signs `authorize_global_session` tx
    │   (build_first_time_auth_tx, client-side, one MWA round-trip)
    │        │
    │        ▼
    │   POST /session/global/register (backend verifies the
    │   GlobalSessionDelegation PDA on-chain before trusting it)
    │        │
    │        ▼
    │   Session key persisted: AES-256-GCM ciphertext in IndexedDB
    │   (WebCrypto instead of the desktop's <data-dir> file)
    │        │
    └───────►│
             ▼
        Gameplay: every move → wasm-bindgen call → JS builds/POSTs to
        /game/{id}/move → backend signs with the delegated session key
        (or client signs locally with the decrypted session key, per
        existing dual-flow logic) → broadcast to ER/mainnet
             │
             ▼
        No further wallet popups until the 30-day/200-game renewal,
        which repeats the MWA round-trip above exactly once.
```

**What changes vs. desktop, for the one-time auth moment shown above:** only
the transport (MWA via `wallet-adapter-mobile` instead of a Chrome extension
popup) and the session-key storage backend (IndexedDB/WebCrypto instead of an
OS file). The backend, the on-chain program, and the session-key trust model
(verify the PDA on-chain, never blindly accept a client-claimed session key)
are unchanged — this is the same code path documented in
[global-session-flow-fix-plan.md](global-session-flow-fix-plan.md).

**Important scope correction:** this diagram only covers the *authorization*
moment. It does **not** cover the ongoing per-move/state-read chain traffic
the game performs during play — that traffic runs through
`solana-chess-client` natively today, which (per §2) cannot compile to wasm32
at all. On the WASM build, *every* chain interaction — not just this one-time
auth step — needs to go through the same JS-bridge pattern (Rust asks JS to
perform the RPC call via `@solana/web3.js`, JS hands the result back via
wasm-bindgen). Treat that as its own, larger work item, not an extension of
this diagram.

**Option A delta:** identical sequence, except the WebView hosting this flow
is foregrounded by Tauri-mobile IPC from the native Bevy activity instead of
being the whole app; the result (signature / session-key bytes) crosses back
over that IPC channel instead of a `postMessage`/wasm-bindgen call.

---

## 4. Touch input E2E — how hard is it really?

Short answer: **the core piece-selection/move mechanic is low effort; camera
control and UI chrome are medium effort; the wallet/lifecycle work above is
the actually expensive part of this whole project, not touch.**

### Why it's easier than it looks: `bevy_picking` is already input-agnostic

`src/game/systems/input.rs` doesn't read raw mouse coordinates — it's built
entirely on Bevy's `bevy_picking` observer API: `On<Pointer<Click>>`,
`On<Pointer<DragStart/Drag/DragEnd>>`, gated only by
`is_primary(button) == matches!(button, PointerButton::Primary)`
(`input.rs:133`). `bevy_picking` is explicitly designed so that any input
source — mouse, touch, pen — funnels into the same `Pointer<Event>` stream,
with a first touch point conventionally arriving as `PointerButton::Primary`.
That means, **once Bevy's touch picking backend is registered**, the existing
`on_piece_click` / `on_piece_drag_*` / `on_square_click` observers should fire
for taps and drags with little or no change to game logic. This needs to be
verified against Bevy 0.19's exact plugin group (confirm whether the touch
backend ships in `DefaultPlugins`' picking group or needs to be added
explicitly), but it's an additive plugin, not a rewrite.

**What does need real work:**

1. **Hover-dependent UI** (`src/input/pointer.rs`: `on_piece_hover`/
   `on_square_hover`, `CursorIcon`/`SystemCursorIcon` styling) has no touch
   equivalent — no hover state exists on a touchscreen. Needs a tap-to-preview
   pattern instead (first tap on a piece shows legal-move highlights, exactly
   what hover does today, just triggered by `Pointer<Click>` instead of
   `Pointer<Over>`).
2. **Interaction pattern choice:** tap-to-select-then-tap-destination is very
   likely the right primary pattern for a small isometric board on a phone
   screen (drag precision with a fingertip occluding the target square is
   worse than with a mouse cursor); the existing drag observers can stay as
   an optional alternate input for players who prefer it, since both would
   ultimately resolve to the same square via the existing world→board-square
   math in `on_piece_drag_end` (`input.rs:490-496`).
3. **Camera control has no drag/pinch today to repurpose** — it's pure
   RTS-style WASD pan / Q-E rotate / scroll-wheel zoom
   (`src/game/systems/camera.rs`). This is new (if well-isolated) input code:
   single-finger drag on empty board area → pan (replacing WASD), pinch → zoom
   (replacing scroll), two-finger twist or on-screen rotate buttons → replace
   Q/E. Because it's a from-scratch touch-gesture system rather than an
   adaptation, budget this as the single largest touch-specific line item —
   still "days," not "weeks."
4. **UI chrome:** hit-target sizing (44×44dp Android minimum), safe-area
   insets for notches/gesture-nav bars, on-screen buttons for anything
   currently keyboard-only (resign, offer draw, flip board — check
   `KeyCode::KeyN/V/R` bindings at `camera.rs:1033,1081,1110` and elsewhere
   for a full inventory), soft-keyboard handling for any text entry.
5. **Fixed window assumption:** `src/core/window_config.rs` defaults to a
   fixed 1366×768 fullscreen desktop window with no notion of
   orientation/DPI/safe-area — needs a mobile-aware window/viewport config
   (this affects Option A only; Option B inherits the browser's viewport
   handling for free).

**Net effort estimate for touch, Option B path:** low-to-medium, on the order
of days for the tap/select/move core (once the touch picking backend is
confirmed wired), plus a few more days for camera gestures and UI-chrome
polish. This is genuinely one of the cheaper parts of the whole mobile
effort — don't let it drive the schedule; the wallet-signing architecture
change (§3) and Android app-lifecycle handling (§6) are the actual critical
path.

---

## 5. Seeker-specific integration

The `seeker-sdk` package (github.com/saicharanpogul/seeker-sdk) the user
linked is **not** a wallet-signing library — worth being explicit about this,
since the name suggests otherwise. It's a pure TypeScript, read-only query
library: Seeker Genesis Token (SGT) ownership verification, `.skr` domain
resolution, SKR balance/staking queries, and raw
`TransactionInstruction` builders for SKR staking that *some other* wallet
adapter has to sign. Actual wallet signing on Seeker goes through MWA (§3),
completely independent of this package.

Given that, and that it has zero Rust bindings, there are two ways to use it,
matched to trust level:

- **Cosmetic / client-side (JS layer, Option B's web app):** import
  `seeker-sdk` (and its `/react` hook subpackage) directly into
  `xfchessdotcom`. Use `getSeekerProfile()` to show a "Verified Seeker" badge
  and resolve `.skr` domains as a vanity display name instead of a raw
  pubkey. Trivial to add (few hours), purely presentational, fine to trust
  client-reported data for.
- **Security-relevant / server-side (Rust port):** if a Seeker-exclusive
  tournament or anti-sybil gate is ever wanted, do **not** trust a
  client-asserted "I'm a Seeker" flag — verify server-side. The SDK's own
  docs note "zero Anchor runtime dependency... all account layouts decoded
  from the on-chain IDL as raw Buffer operations," which means the relevant
  subset (`isSeeker` via the SGT Token-2022 mint check) is a small, portable
  piece of logic — realistically ~1 day to port to Rust against
  `crates/solana/solana-chess-client`'s existing account-fetching
  infrastructure, next to the existing compliance checks in
  `backend/src/signing/`. Recommended over adding a JS runtime dependency
  anywhere near money-relevant checks.

**Distribution:** Seeker ships stock Android + the Solana dApp Store. A PWA
(Option B, no wrapper) is directly compatible with "Add to Home Screen" on
Seeker's browser today — no store review needed for that path. For a dApp
Store listing (better discoverability, matches how most Solana Mobile apps
distribute), package via `dapp-store-cli` — this wants a real APK/AAB, so
either the Tauri-Android wrap of Option B, or Option A's native build, are
the candidates; the raw PWA-only route can't be dApp-Store-listed as-is.

---

## 6. Risks & open items

- **Android cross-compile is unverified against this exact dependency
  graph — this is the first spike, full stop.** §2's case for Option A rests
  on `iroh`/`quinn`, `solana-sdk`/`solana-client`, and vendored `openssl` all
  building for `aarch64-linux-android` via `cargo-ndk`. There's no structural
  reason (like wasm32's browser-sandbox restrictions) to expect failure, but
  it hasn't been tried with this workspace's exact versions. Do this before
  any other Option A work — it's cheap to find out early (a `cargo-ndk build`
  attempt) and everything else is blocked on it.
- **The client doesn't compile for wasm32 today, for concrete reasons, not
  vague ones.** `openssl` (vendored) and `iroh`/`braid-iroh`/`iroh-gossip`
  are unconditional dependencies with no wasm32-browser path — see §2 for
  specifics. Relevant only if/when the Option B companion path is pursued.
- **Multiplayer is architecturally different on WASM, permanently.** Iroh's
  raw-UDP/QUIC transport has no browser equivalent — not "hard to port,"
  literally unavailable in a browser sandbox. A WASM build's multiplayer
  must be server-relayed over WebSocket instead of direct P2P. Decide early
  whether that's an acceptable permanent divergence from desktop, or whether
  it changes the recommendation toward Option A for multiplayer specifically
  (e.g. WASM for single-player/vs-AI + wallet, native for ranked multiplayer)
  — this is a product decision, not just an engineering one.
- **On-chain integration is a full parallel implementation, not a shim.**
  Corrected in §2/§3: every chain read/write the game performs during play,
  not just the one-time session auth, needs to route through a JS bridge
  since `solana-chess-client` cannot run in wasm. Size this as the largest
  engineering item in the plan.
- **WebView/mobile WebGL2 perf on real Seeker hardware** — unvalidated;
  should be the first *rendering* spike (after the compile-blocker fix
  above), since it's the assumption everything else rests on. Seeker's
  chipset (Dimensity 7300, 8GB RAM, 6.36" 2670×1200 120Hz AMOLED, Android 15)
  is mid-range, not flagship — validate against similar-tier hardware, not a
  flagship test phone.
- **Android background lifecycle** — Iroh P2P (QUIC) connections (native/
  Option A) and any open backend WebSocket (either option) will drop under
  Android's Doze/backgrounding; a chess game isn't always foregrounded
  during the opponent's turn, so this needs reconnect/resync-on-resume logic
  (compare local vs. on-chain move counter) and likely a push-notification
  ("it's your move") channel via FCM — not designed yet, not covered above,
  should be scoped separately once the core loop is validated.
- **Bevy 0.19 touch backend confirmation** — §4's optimism assumes the touch
  picking backend either ships in `DefaultPlugins`' picking group or is a
  one-line `add_plugins` away; verify against the actual Bevy 0.19 source
  before estimating further.
- **Session-key storage on WASM** — IndexedDB + WebCrypto AES-256-GCM is the
  natural swap for the desktop's encrypted file, but wasn't audited here;
  confirm `global_session_manager.rs`'s encryption logic is portable
  (WebCrypto vs. whatever Rust crate it currently uses) or needs a
  wasm-specific implementation.
- **Distribution/legal** — per the [4-country legal verdict](../../CLAUDE.md)
  memory, the 3-part model is currently cleared for UK/DE/BR/CA as pure-skill
  chess; mobile distribution doesn't change that analysis, but Google Play's
  and the Solana dApp Store's own real-money-gaming policies haven't been
  checked against this project's fee-vault/prize model and should be before
  submitting either listing.

---

## 7. Suggested phasing

### Option A track (primary)

1. **Cross-compile spike (small, do first):** stand up `cargo-ndk` +
   Android NDK toolchain, attempt a build of `src/` (with `--features
   solana`) for `aarch64-linux-android`. This either confirms §2's case or
   surfaces the real blockers early — cheap to find out now.
2. **Boot to a triangle (days):** follow Bevy's `examples/mobile` template
   to get the existing renderer producing a frame inside a `GameActivity` on
   a real device/emulator — no touch, no wallet yet, just confirming wgpu's
   Vulkan/GLES backend works end to end on Android.
3. **Touch core (days):** register touch picking backend, verify existing
   `Pointer<Click>`/`Pointer<Drag*>` observers fire from taps, add
   tap-to-preview replacing hover, ship tap-select-then-tap-move as primary
   interaction.
4. **MWA bridge (days, bounded):** thin Kotlin shim (JNI) or embedded
   WebView (Tauri Android mode) hosting the existing MWA-wired JS from
   `xfchessdotcom`/`tauri/wallet-ui`; bridge the signing handshake result
   back to native Rust. Validate the one-time-authorize → play-without-popups
   loop end to end on a real Seeker device — everything after authorization
   should be native `solana-chess-client` + native Iroh, unchanged from
   desktop.
5. **Camera + UI chrome (days):** touch pan/pinch/rotate replacing WASD/Q-E/
   scroll, hit-target sizing, safe-area insets, on-screen action buttons.
6. **Lifecycle hardening:** Activity backgrounding/resume, Doze-mode impact
   on Iroh sockets, `GameActivity` surface recreation on rotation — scope in
   detail once 1–5 are validated.
7. **Packaging/distribution:** APK/AAB signing, Google Play and/or Solana
   dApp Store (`dapp-store-cli`) submission.

### Option B track (companion, run independently/later)

1. Split `openssl`/`iroh`/`braid-iroh`/`iroh-gossip` behind
   `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` to get a
   local/vs-AI-only wasm32 build linking at all.
2. Rendering/perf spike in a mobile browser (WebGL2).
3. Touch core (shared work with Option A track, if sequenced second).
4. On-chain JS bridge for *every* chain operation (not just auth) — the
   larger of the two paths' wallet-integration costs, per §2.
5. Multiplayer: WebSocket-relay implementation, or explicitly scope this
   track to single-player/vs-AI + wallet only, deferring multiplayer to
   Option A.
6. PWA packaging (manifest, service worker, Add-to-Home-Screen).

---

## 8. Effort estimate (Option A track)

Person-weeks of focused work, not calendar time — this codebase has never
targeted Android before, so treat these as ranges with real variance, not
commitments. The two biggest unknowns (§1 = whether the dependency graph
cross-compiles at all, §4 = the MWA bridge) dominate the uncertainty; nothing
downstream can be estimated tightly until §1 lands.

| Phase | Estimate | Why the range |
|---|---|---|
| 1. Cross-compile spike | 0.5–1 week | NDK toolchain friction (linker errors, API-level mismatches, vendored-openssl quirks) is the classic first-time tax |
| 2. Boot to a triangle | 1–2 weeks | Bevy's Android path is real but less traveled than desktop/wasm; asset-loading and window/lifecycle code here currently assumes desktop |
| 3. Touch core | 0.5–1 week | Core mechanic is cheap per §4 of the main plan; contained scope |
| 4. MWA bridge | 1–2 weeks | Genuinely new: first JNI (or embedded-WebView-IPC) work in this codebase; async callback across that boundary into Bevy's event loop is the fiddly part |
| 5. Camera + UI chrome | 1–2 weeks | Board camera gestures are contained, but this game has many other mouse/keyboard-built screens (matchmaking, tournament admin, wager UI) — budget more if all of them need touch parity for v1, less if only the core play loop ships first |
| 6. Lifecycle hardening | 1–2 weeks (reconnect/resume only) to 3–5 weeks (if push notifications via FCM are in scope for v1) | Open-ended if push is included — recommend deferring push to a fast-follow |
| 7. Packaging/distribution | 1–2 weeks of engineering **+ unpredictable store review latency** | Play Store's real-money-gaming policy review is the wildcard given the fee-vault/prize model (§6) — could be fast or could stall; dApp Store is typically friendlier/faster |

**Rough total: 6–11 person-weeks of engineering** for a working native
Android build with wallet signing, touch, and multiplayer — before store
review latency, which is outside your control and not included above. Given
this is new territory end to end, treat the low end as optimistic-if-nothing-
surprising and the high end as the realistic planning number.

## 9. Repo strategy: branch, not fork

**Don't fork — use a long-lived branch in this repo.** Reasoning:

- The Android port reuses ~95% of the *same* code (game logic, rendering,
  backend APIs, the on-chain program) — it is not a divergent product, it's
  a fourth build target for the existing game. A fork means every future
  change to that shared code (bug fixes, chess-logic updates, rendering
  work, backend API changes) has to be manually carried across two
  repositories forever. That's the specific, well-known pain of long-lived
  forks, and there's no offsetting benefit here since the "fork" and
  "upstream" are the same product.
- This workspace already builds Windows/Mac/Linux from one repo via
  target-conditional Cargo config (e.g. `tauri/Cargo.toml`'s
  `[target.'cfg(windows)'.dependencies]` block found during the codebase
  survey). Android is architecturally "a fourth target triple," following
  the exact pattern already established — `[target.'cfg(target_os =
  "android")'.dependencies]` for the NDK/JNI-specific pieces, same as
  Windows gets its own block today.
- A branch gives you the actual thing you probably want — freedom to break
  things without destabilizing `main` — without the permanent-divergence
  cost. Merge (or keep rebasing) back once the Android target is stable, the
  same way any other large feature would land.
- New Android-specific scaffolding (Gradle project, `gen/android/` from
  `cargo-ndk`/`tauri android init`, the Kotlin MWA shim if that route is
  taken) slots in next to `tauri/` the same way the desktop wrapper already
  does — this repo is already a monorepo housing five components (game
  client, backend, Solana program, web frontend, desktop wrapper); a mobile
  target is a sixth in kind, not a reason to split the repo.

If CI build time or contributor-permission boundaries become a real problem
later, that's solvable with CI job scoping or `CODEOWNERS`, not a fork.
