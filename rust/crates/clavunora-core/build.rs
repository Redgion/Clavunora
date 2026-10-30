// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../../pianoflow/resources/catalog")
        .canonicalize()
        .expect("Existing catalog must be available at build time");
    let index = root.join("index.json");
    println!("cargo:rerun-if-changed={}", index.display());
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(index).unwrap()).unwrap();
    let mut generated = String::from("pub const BUNDLED_SCORES: &[(&str, &str)] = &[\n");
    for entry in manifest["entries"].as_array().unwrap() {
        if let Some(score) = entry["score"].as_str() {
            let path = root
                .join(score)
                .canonicalize()
                .expect("Missing catalog score");
            assert!(path.starts_with(&root), "Catalog path escapes its root");
            println!("cargo:rerun-if-changed={}", path.display());
            generated.push_str(&format!(
                "({:?}, {:?}),\n",
                entry["id"].as_str().unwrap(),
                score
            ));
        }
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("catalog.rs"),
        generated,
    )
    .unwrap();
}
