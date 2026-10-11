---
name: android-native
description: Build or debug XFChess native Android integration using Kotlin, Gradle, GameActivity, JNI, cargo-ndk and Solana Mobile Wallet Adapter. Use for APK packaging, native lifecycle and mobile signing.
metadata:
  provenance: docs-derived
---

# Native Android

Start with `mobile/`, `src/android/` and `docs/runbooks/android-release.md`.
This app hosts the Rust Bevy game through GameActivity; it is not an Expo,
React Native or Tauri mobile application.

- Use the Gradle wrapper and existing AGP/Kotlin, JDK, SDK, NDK and ABI settings.
  Check toolchain compatibility before changing a version. Preserve cargo-ndk API
  alignment and the native library actually loaded by the Activity.
- Keep one native activity/glue integration. Do not add incompatible GameActivity
  prefab glue alongside android-activity's implementation.
- JNI references and JNIEnv are thread/lifetime scoped. Attach native worker
  threads correctly; retain global references only for the required lifecycle.
- Treat Activity recreation, pause/resume and process death as normal. Reconnect
  owned work without replaying user actions or retaining an obsolete Activity.
- Use the existing Kotlin Mobile Wallet Adapter boundary. Validate returned account,
  cluster, signed message/transaction, cancellation and session ownership before
  applying a result; keep wallet approval under the player's control.
- Package architecture-matched native libraries and real game assets. Keep desktop
  executables out of APK assets; inspect extraction settings for child-process engines.
- Preserve release versionCode progression and signing identity across updates.
  Keep keystore material outside the repository and logs.

Run relevant Gradle configuration/assets tasks, build the native library/APK for
the affected ABI, inspect contents and test lifecycle/signing on a device or emulator
when the change affects runtime behavior. Report which targets were actually exercised.

Sources: [GameActivity](https://developer.android.com/games/agdk/game-activity),
[JNI guidance](https://developer.android.com/training/articles/perf-jni),
[Gradle Android builds](https://developer.android.com/build),
[cargo-ndk](https://github.com/bbqsrc/cargo-ndk),
[Solana Mobile Kotlin](https://docs.solanamobile.com/get-started/kotlin/installation).
