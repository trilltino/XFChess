//! Shared JNI plumbing for Android: the Activity/Context handle and TLS init.
//!
//! Every Android-specific JNI need in this crate (this file's `init_tls_verifier`,
//! the future MWA bridge, `nativeLibraryDir` lookup for Stockfish, `WindowInsets`
//! for safe-area layout) starts from the same `AndroidApp`, so it is centralized
//! here rather than re-derived at each call site.

use bevy::android::android_activity::AndroidApp;
use jni::JavaVM;
use std::path::PathBuf;

/// The `AndroidApp` handle Bevy's `#[bevy_main]`-generated `android_main` stashed
/// before calling into our `main()`. Panics if called before that — which should
/// be impossible given the call order in the generated code.
fn android_app() -> &'static AndroidApp {
    bevy::android::ANDROID_APP
        .get()
        .expect("ANDROID_APP not set — must be called from within Bevy's android_main")
}

/// Wraps the raw `JavaVM` pointer `AndroidApp` already holds. Cheap: `JavaVM`
/// is a thin `Clone`-able handle backed by a process-wide singleton (jni 0.22
/// caches it in `JAVA_VM_SINGLETON` the first time `from_raw` is called), not a
/// new attachment — attaching happens per-call in `attach_current_thread`.
///
/// # Safety
/// The pointer comes from `android_activity`'s own `AndroidApp::vm_as_ptr`,
/// which is documented to return a valid `JavaVM*` for the lifetime of the
/// process; this only re-asserts that contract to satisfy `JavaVM::from_raw`'s
/// signature.
fn java_vm() -> JavaVM {
    unsafe { JavaVM::from_raw(android_app().vm_as_ptr() as *mut jni::sys::JavaVM) }
}

/// Initializes `rustls-platform-verifier`'s Android certificate verifier.
///
/// **Must be called before any HTTP client is constructed.** `reqwest`'s
/// `rustls` feature unconditionally depends on `rustls-platform-verifier`,
/// whose Android verifier calls into a Kotlin class
/// (`org.rustls.platformverifier.CertificateVerifier`, provided by the AAR
/// declared in `mobile/app/build.gradle.kts`) and needs an attached JNI `Env`
/// plus the Activity as a `Context` to do so. Skipping this makes every HTTPS
/// request on Android fail at the TLS handshake — see Cargo.toml's comment on
/// the `rustls-platform-verifier` dependency.
pub fn init_tls_verifier() -> Result<(), String> {
    let vm = java_vm();
    let activity_raw = android_app().activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // Safety: `activity_as_ptr` returns the Activity `android_activity`
        // itself holds a live reference to for the process's lifetime, which
        // is also a valid `Context` (Activity extends Context) — exactly the
        // object `init_with_env` expects.
        let context = unsafe { jni::objects::JObject::from_raw(env, activity_raw) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|e| format!("rustls_platform_verifier::android::init_with_env failed: {e}"))
}

/// Returns Android's install-time native library directory. Libraries placed
/// there by legacy JNI packaging are executable by the app, unlike files in
/// writable app storage.
pub fn native_library_dir() -> Result<PathBuf, String> {
    let vm = java_vm();
    let activity_raw = android_app().activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> Result<PathBuf, jni::errors::Error> {
        let activity = unsafe { jni::objects::JObject::from_raw(env, activity_raw) };
        let application = env
            .call_method(
                &activity,
                jni::jni_str!("getApplicationInfo"),
                jni::jni_str!("()Landroid/content/pm/ApplicationInfo;"),
                &[],
            )?
            .l()?;
        let native_dir = env
            .get_field(
                &application,
                jni::jni_str!("nativeLibraryDir"),
                jni::jni_str!("Ljava/lang/String;"),
            )?
            .l()?;
        let native_dir = jni::objects::JString::from(native_dir);
        let native_dir: String = env.get_string(&native_dir)?.into();
        Ok(PathBuf::from(native_dir))
    })
    .map_err(|e| format!("nativeLibraryDir lookup failed: {e}"))
}
