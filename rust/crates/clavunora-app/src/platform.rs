// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
#[cfg(target_os = "linux")]
use anyhow::ensure;
use anyhow::{Context, Result};
use std::{
    env,
    path::{Path, PathBuf},
};

#[cfg(target_os = "android")]
pub static ANDROID_DATA: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn config_root() -> Result<PathBuf> {
    #[cfg(target_os = "android")]
    {
        Ok(ANDROID_DATA
            .get()
            .context("Android data directory not initialized")?
            .clone())
    }
    #[cfg(target_os = "windows")]
    {
        Ok(PathBuf::from(
            env::var_os("APPDATA").context("Не найдена папка AppData")?,
        ))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(
            PathBuf::from(env::var_os("HOME").context("Не найдена домашняя папка")?)
                .join("Library/Application Support"),
        )
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
            let path = PathBuf::from(path);
            ensure!(
                path.is_absolute(),
                "XDG_CONFIG_HOME должен быть абсолютным путём"
            );
            return Ok(path);
        }
        Ok(
            PathBuf::from(env::var_os("HOME").context("Не найдена домашняя папка")?)
                .join(".config"),
        )
    }
}
pub fn profile_dir(root: &Path) -> PathBuf {
    let stable = root.join("Clavunora");
    let previous = root.join("ClavunoraRustPreview");
    // Reuse an existing Rust profile in place. No implicit copying or merging of SQLite files.
    if !stable.join("library.sqlite3").exists() && previous.join("library.sqlite3").exists() {
        previous
    } else {
        stable
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_reuses_existing_rust_profile_but_never_selects_python_or_merges_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(profile_dir(root), root.join("Clavunora"));
        std::fs::create_dir(root.join("PianoFlow")).unwrap();
        std::fs::write(root.join("PianoFlow/library.sqlite3"), b"python").unwrap();
        assert_eq!(profile_dir(root), root.join("Clavunora"));
        std::fs::create_dir(root.join("ClavunoraRustPreview")).unwrap();
        std::fs::write(
            root.join("ClavunoraRustPreview/library.sqlite3"),
            b"preview",
        )
        .unwrap();
        assert_eq!(profile_dir(root), root.join("ClavunoraRustPreview"));
        std::fs::create_dir(root.join("Clavunora")).unwrap();
        std::fs::write(root.join("Clavunora/library.sqlite3"), b"stable").unwrap();
        assert_eq!(profile_dir(root), root.join("Clavunora"));
        assert_eq!(
            std::fs::read(root.join("ClavunoraRustPreview/library.sqlite3")).unwrap(),
            b"preview"
        );
    }
}

/// Keep the activity and its profile alive when the system Back gesture is used.
#[cfg(target_os = "android")]
pub fn minimize_android() -> Result<()> {
    let context = ndk_context::android_context();
    // ndk-context is initialized by android-activity for the activity lifetime.
    let vm = unsafe { jni::JavaVM::from_raw(context.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    let activity = unsafe { jni::objects::JObject::from_raw(context.context().cast()) };
    let result = env.call_method(
        &activity,
        "moveTaskToBack",
        "(Z)Z",
        &[jni::objects::JValue::Bool(1)],
    );
    if result.is_err() && env.exception_check()? {
        env.exception_clear()?;
    }
    anyhow::ensure!(result?.z()?, "Android refused to move task to background");
    Ok(())
}
