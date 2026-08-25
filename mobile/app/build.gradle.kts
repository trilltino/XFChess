// Translated to Kotlin DSL from rust-mobile/rust-android-examples'
// agdk-winit-wgpu-egui/app/build.gradle — the android-activity crate
// maintainers' own reference, verified current against android-activity
// 0.6.1 (the exact version this workspace's Cargo.lock resolves via Bevy's
// `android-game-activity` feature).
//
// Deliberately NOT using `buildFeatures { prefab = true }` or linking the
// upstream GameActivity prefab C++ package — android-activity's own README
// is explicit that doing so links an incompatible native glue layer against
// the one android-activity itself provides. `libxfchess.so` already contains
// the Rust-side GameActivity glue (compiled in by android-activity's
// build.rs); this module only needs the JAVA-side `GameActivity` class from
// the games-activity AAR below.
//
// No `org.jetbrains.kotlin.android` plugin: AGP 9.0+ has Kotlin support
// built in by default for a new module (JetBrains' migration guidance,
// Jan 2026) — applying it separately is for pre-AGP-9 projects.
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.trilltino.xfchess"
    // Pinned to the NDK actually installed for this workspace (r27c) rather
    // than AGP 9.1's own default (r28c) — matches the plan's "NDK r27+"
    // floor and the toolchain `cargo ndk` was already verified against.
    ndkVersion = "27.2.12479018"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.trilltino.xfchess"
        // 31 is GameActivity's floor (see android-activity's own docs) and
        // matches the platform level `cargo ndk -P 31` targets — anything
        // lower fails to link (`-laaudio` isn't in the NDK sysroot stub set
        // below API 26, verified by hitting exactly that linker error at the
        // default `cargo ndk` platform of 21).
        minSdk = 31
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    packaging {
        jniLibs {
            // Stockfish is launched as a child process on Android. Legacy
            // packaging extracts native libraries to nativeLibraryDir, which
            // is required for that executable path to be usable at runtime.
            useLegacyPackaging = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    // `libxfchess.so` (and, later, any other native libs) lands here via
    // `cargo ndk -o app/src/main/jniLibs build` — Gradle's default
    // `jniLibs.srcDirs` already includes `src/main/jniLibs`, so no explicit
    // sourceSets entry is needed for it.
    sourceSets {
        getByName("main") {
            // Reference, not copy — the same 64 MB `assets/` the desktop
            // build reads, via `bevy_asset::io::android` at runtime. Editing
            // an asset once updates every platform. Gradle resolves this
            // relative to the *module* directory (mobile/app/, where this
            // build.gradle.kts lives), not src/main/ — so it's two levels up
            // to the repo root, not four (verified against this repo's
            // actual layout, not assumed from Bevy's own example, whose
            // module sits one directory deeper).
            assets.directories.add("../../assets")
        }
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.0")
    // Version pinned to exactly what android-activity 0.6.1's own README
    // states it supports ("Your Android package should depend on
    // androidx.games:games-activity:4.4.0") — not whatever is newest on
    // Maven, since a mismatch here is the kind of thing that fails at
    // runtime (JNI method-not-found), not at Gradle sync.
    implementation("androidx.games:games-activity:4.4.0")
    implementation("com.solanamobile:mobile-wallet-adapter-clientlib-ktx:2.0.3")
    implementation("com.solanamobile:web3-solana:0.2.5")
    implementation("com.solanamobile:rpc-core:0.2.7")
    implementation("io.github.funkatronics:multimult:0.2.3")
}
