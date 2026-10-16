# Android release

## Build and device test

Install SDK 35, NDK `27.2.12479018`, CMake, JDK 17+, `cargo-ndk` and
the `aarch64-linux-android` Rust target. Set `ANDROID_HOME`,
`ANDROID_NDK_HOME` and `JAVA_HOME`. Check [Gradle](../../mobile/app/build.gradle.kts)
and [release CI](../../.github/workflows/release.yml) before changing these pins.

On Windows, `just build-android` creates:
`mobile/app/build/outputs/apk/release/app-release-unsigned.apk`.

That unsigned APK needs signing. For device testing after the native build:

```text
cd mobile
gradlew.bat assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

The native app targets Android 12+, arm64-v8a and landscape orientation.

## Distribution

The current CI Android job assembles and uploads an unsigned release APK; it
does not establish store signing/publication. Keep a long-lived release keystore
outside Git, reuse it for updates and verify the certificate fingerprint.
[Publishing](../PUBLISHING.md) lists the current artifact/signing boundaries.

Verify the APK includes `libxfchess.so` and `libstockfish.so`. On a physical
device test launch, touch play, Stockfish at all eight difficulty levels playing
both sides, wallet connection/rejection, a wagered match and background/resume mid-match. Publisher verification and
store review remain external steps.

Stockfish is the only AI engine. `just build-android` downloads the Stockfish 16
Android ARMv8 executable when it is absent. For a direct Cargo/Gradle build,
run `scripts/prepare_stockfish_android.ps1` first. Both paths place it at
`mobile/app/src/main/jniLibs/arm64-v8a/libstockfish.so`; release CI also downloads it.
Gradle extracts it to `nativeLibraryDir` for execution. Packaging cleanup must
retain both `libxfchess.so` and `libstockfish.so`.

The physical-device release gate is `scripts/test_stockfish_android.ps1` with
an installed debug APK and a connected ARM64 device. After its UCI/search check,
verify AI play, evaluation, leaving during a search, retry after a process failure,
and background/resume. Record the device/API version and result in release notes.
