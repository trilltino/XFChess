// Mirrors rust-mobile/rust-android-examples' agdk-winit-wgpu-egui settings.gradle
// (the android-activity crate maintainers' own reference project — verified
// current against android-activity 0.6.1, the exact version this workspace
// resolves), translated to Kotlin DSL for consistency with the rest of this
// repo's build scripts.
pluginManagement {
    repositories {
        gradlePluginPortal()
        google()
        mavenCentral()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "XFChess"
include(":app")
