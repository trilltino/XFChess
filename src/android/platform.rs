//! Shared Android JNI plumbing for the activity handle, TLS, and native library path.

use bevy::android::android_activity::AndroidApp;
use jni::JavaVM;
use std::path::PathBuf;

/// Returns Bevy's process-wide Android activity handle.
fn android_app() -> &'static AndroidApp {
    bevy::android::ANDROID_APP
        .get()
        .expect("ANDROID_APP not set — must be called from within Bevy's android_main")
}

/// Wraps the process-lifetime VM pointer held by `AndroidApp`.
fn java_vm() -> JavaVM {
    unsafe { JavaVM::from_raw(android_app().vm_as_ptr() as *mut jni::sys::JavaVM) }
}

/// Initializes the Android certificate verifier before HTTP clients are created.
pub fn init_tls_verifier() -> Result<(), String> {
    let vm = java_vm();
    let activity_raw = android_app().activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // The activity is a live process-lifetime Context owned by Android.
        let context = unsafe { jni::objects::JObject::from_raw(env, activity_raw) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|e| format!("rustls_platform_verifier::android::init_with_env failed: {e}"))
}

/// Returns Android's install-time native library directory.
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
