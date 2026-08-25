@echo off
:: Cross-compiles libxfchess.so for Android (arm64-v8a) via cargo-ndk, then
:: assembles the release APK via the Gradle wrapper in mobile\.
::
:: Requires (see docs/INSTALL.md): Android SDK (platform 35, build-tools
:: 35.0.0, platform-tools), NDK r27+, cargo-ndk, JDK 17+, and the
:: aarch64-linux-android Rust target. ANDROID_HOME / ANDROID_NDK_HOME /
:: JAVA_HOME must be set in the environment.
::
:: -P 31 matches minSdk in mobile\app\build.gradle.kts — cargo-ndk defaults
:: to platform 21, which is missing the NDK sysroot stub for -laaudio
:: (cpal's Android backend); verified by hitting exactly that linker error
:: at the default.
::
:: Output: mobile\app\src\main\jniLibs\arm64-v8a\libxfchess.so
::         mobile\app\build\outputs\apk\release\app-release-unsigned.apk

echo [build_android] Building libxfchess.so (release, arm64-v8a)...

cargo ndk -t arm64-v8a -P 31 -o mobile\app\src\main\jniLibs build --release --lib --features solana
if errorlevel 1 (
    echo [build_android] Rust cross-compile FAILED
    exit /b 1
)

:: cargo-ndk's -o copies EVERY cdylib artifact from cargo's build cache for
:: this target, not just libxfchess.so — confirmed by inspection, not assumed:
:: xfchess-game (the Solana program crate, also crate-type=["cdylib"]) and a
:: handful of internal cdylib byproducts from iroh/solana-zk-sdk/irpc end up
:: in jniLibs too. Gradle would bundle all of them into the APK as loose,
:: pointless native libs otherwise. Delete everything except what this app
:: actually loads (System.loadLibrary("xfchess") in MainActivity.kt).
for %%F in (mobile\app\src\main\jniLibs\arm64-v8a\*.so) do (
    if /I not "%%~nxF"=="libxfchess.so" del "%%F"
)

echo.
echo [build_android] Assembling release APK...

pushd mobile
call gradlew.bat assembleRelease
if errorlevel 1 (
    popd
    echo [build_android] Gradle build FAILED
    exit /b 1
)
popd

echo.
echo [build_android] Done.
for %%F in (mobile\app\build\outputs\apk\release\app-release-unsigned.apk) do echo APK size: %%~zF bytes
echo Install with: adb install -r mobile\app\build\outputs\apk\release\app-release-unsigned.apk
