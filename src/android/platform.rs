#![allow(unsafe_code)]

use bevy::android::android_activity::AndroidApp;
use jni::JavaVM;
use jni::{objects::JObject, refs::Global};
use std::path::PathBuf;

fn android_app() -> Result<&'static AndroidApp, String> {
    bevy::android::ANDROID_APP
        .get()
        .ok_or_else(|| "ANDROID_APP not set: call from within Bevy's android_main".to_string())
}

fn java_vm(app: &AndroidApp) -> JavaVM {
    // SAFETY: android-activity hands out the process's JavaVM, which outlives the app.
    unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }
}

pub fn init_tls_verifier() -> Result<(), String> {
    let app = android_app()?;
    let vm = java_vm(app);
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY: Android owns this global reference; `app` keeps it valid throughout
        // the attached callback. Borrow it without taking ownership of Android's ref.
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&activity_raw)? };
        let context = env.new_local_ref(&activity)?;
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|e| format!("rustls_platform_verifier::android::init_with_env failed: {e}"))
}

pub fn native_library_dir() -> Result<PathBuf, String> {
    let app = android_app()?;
    let vm = java_vm(app);
    let activity_raw = app.activity_as_ptr() as jni::sys::jobject;

    vm.attach_current_thread(|env| -> Result<PathBuf, jni::errors::Error> {
        // SAFETY: `app` outlives this callback and owns the borrowed global reference.
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&activity_raw)? };
        let application = env
            .call_method(
                &activity,
                jni::jni_str!("getApplicationInfo"),
                jni::jni_sig!("()Landroid/content/pm/ApplicationInfo;"),
                &[],
            )?
            .l()?;
        let native_dir = env
            .get_field(
                &application,
                jni::jni_str!("nativeLibraryDir"),
                jni::jni_sig!("Ljava/lang/String;"),
            )?
            .l()?;
        let native_dir = jni::objects::JString::cast_local(env, native_dir)?;
        let native_dir = native_dir.try_to_string(env)?;
        Ok(PathBuf::from(native_dir))
    })
    .map_err(|e| format!("nativeLibraryDir lookup failed: {e}"))
}
