// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
fn main() {
    #[cfg(target_os = "windows")]
    {
        let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("../../..");
        let icon = root.join("pianoflow/resources/icons/pianoflow.ico");
        let manifest = root.join("packaging/rust-windows/Clavunora.manifest");
        println!("cargo:rerun-if-changed={}", icon.display());
        println!("cargo:rerun-if-changed={}", manifest.display());
        winresource::WindowsResource::new()
            .set_icon(icon.to_str().unwrap())
            .set_manifest_file(manifest.to_str().unwrap())
            .set("ProductName", "Clavunora")
            .set("FileDescription", "Clavunora piano practice")
            .set(
                "LegalCopyright",
                "Copyright (C) 2026 redgion and Clavunora contributors",
            )
            .compile()
            .expect("Cannot embed Windows icon and application manifest");
    }
}
