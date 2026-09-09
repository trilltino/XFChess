#![cfg(target_os = "android")]

use std::path::PathBuf;

pub fn internal_data_dir() -> Option<PathBuf> {
    bevy::android::ANDROID_APP.get()?.internal_data_path()
}

pub fn external_data_dir() -> Option<PathBuf> {
    bevy::android::ANDROID_APP.get()?.external_data_path()
}
