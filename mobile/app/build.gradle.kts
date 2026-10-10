// Use android-activity native glue from libxfchess.so; linking GameActivity
// prefab adds incompatible glue. AGP 9 provides Kotlin support without a separate plugin.
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.trilltino.xfchess"
    // Match the installed NDK r27c toolchain used by cargo ndk.
    ndkVersion = "27.2.12479018"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.trilltino.xfchess"
        // Match cargo-ndk API 31 and GameActivity's minimum; lower sysroots lack aaudio.
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
            // Legacy packaging extracts executable libraries to nativeLibraryDir for child processes.
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

    // The default jniLibs source set already includes the cargo output.
    sourceSets {
        getByName("main") {
            // Reference shared assets relative to the Gradle module directory.
            assets.directories.add("../../assets")
        }
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.0")
    // Keep the games-activity version compatible with android-activity native glue.
    implementation("androidx.games:games-activity:4.4.0")
    implementation("com.solanamobile:mobile-wallet-adapter-clientlib-ktx:2.0.3")
    implementation("com.solanamobile:web3-solana:0.2.5")
    implementation("com.solanamobile:rpc-core:0.2.7")
    implementation("io.github.funkatronics:multimult:0.2.3")
}
