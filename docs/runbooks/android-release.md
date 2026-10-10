# Android Release Runbook

## Local prerequisites

Install Android Studio with SDK 35, NDK `27.2.12479018`, CMake, JDK 17+,
`cargo-ndk`, and the Rust target `aarch64-linux-android`. Set
`ANDROID_HOME`, `ANDROID_NDK_HOME`, and `JAVA_HOME`.

## Build

From the repository root:

```bat
just build-android
```

The recipe builds the Rust client for `arm64-v8a`, packages native libraries
with legacy JNI extraction, and produces:

```text
mobile\app\build\outputs\apk\release\app-release-unsigned.apk
```

Sign the release APK before installing, or use a debug build for device tests:

```bat
cd mobile
gradlew.bat assembleDebug
adb install -r app\build\outputs\apk\debug\app-debug.apk
```

The device must run Android 12 or newer. The app is landscape locked.

## Signing

The dApp Store signing key is long-lived and must never be committed. Create it
once with the command from `docs/PUBLISHING.md`, store it in the CI secret
store, and reuse the same key for every update. Do not reuse a Google Play key.

Before enabling store publication, configure the Gradle release signing values
from CI secrets and verify the APK certificate fingerprint. A locally unsigned
APK is suitable only for device testing and must not be submitted.

## CI release gates

The `android` job in `.github/workflows/release.yml` must complete these gates:

1. `libxfchess.so` is built for `aarch64-linux-android`.
2. The ARMv8 Stockfish binary is present as `libstockfish.so`.
3. The release APK contains `lib/arm64-v8a/libxfchess.so`.
4. The APK is signed with the unchanged dApp Store key.
5. Device verification covers launch, touch play against both engines, wallet
   login, one wagered match, and five minutes of background/resume mid-match.

Physical wallet approval, publisher KYC/KYB, App NFT minting, and dApp Store
review are external gates and cannot be automated by this repository alone.
