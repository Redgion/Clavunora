// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
#![cfg(target_os = "android")]
mod aero;
mod app;
mod audio;
mod backups;
mod dialogs;
mod keyboard_view;
mod midi_import;
mod midi_input;
mod platform;
mod score_editor;
mod staff_view;
use std::{ffi::CString, io::Read};

fn launch(android: android_activity::AndroidApp) -> anyhow::Result<()> {
    let data = android
        .internal_data_path()
        .ok_or_else(|| anyhow::anyhow!("No Android storage"))?;
    let resources = data.join("resources/catalog");
    std::fs::create_dir_all(&resources)?;
    let assets = android.asset_manager();
    let index_name = CString::new("catalog/index.json")?;
    let mut index = String::new();
    assets
        .open(&index_name)
        .ok_or_else(|| anyhow::anyhow!("Missing bundled catalog"))?
        .read_to_string(&mut index)?;
    let manifest: serde_json::Value = serde_json::from_str(&index)?;
    for entry in manifest["entries"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Invalid catalog"))?
    {
        if let Some(relative) = entry["score"].as_str() {
            let path = std::path::Path::new(relative);
            anyhow::ensure!(
                path.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
                "Unsafe resource path"
            );
            let name = CString::new(format!("catalog/{relative}"))?;
            let mut bytes = Vec::new();
            assets
                .open(&name)
                .ok_or_else(|| anyhow::anyhow!("Missing score: {relative}"))?
                .read_to_end(&mut bytes)?;
            let destination = resources.join(path);
            std::fs::create_dir_all(destination.parent().unwrap())?;
            std::fs::write(destination, bytes)?;
        }
    }
    let _ = clavunora_core::catalog::ANDROID_CATALOG.set(resources);
    let _ = platform::ANDROID_DATA.set(data.clone());
    let store = clavunora_core::store::Library::open(&data.join("library.sqlite3"))?;
    let app = app::PreviewApp::new(store, data, false)?;
    let options = eframe::NativeOptions {
        android_app: Some(android),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Clavunora",
        options,
        Box::new(move |cc| {
            aero::install_fonts(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

#[unsafe(no_mangle)]
fn android_main(android: android_activity::AndroidApp) {
    if let Err(error) = launch(android) {
        eprintln!("Clavunora Android: {error:#}");
    }
}
