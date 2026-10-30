#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
mkdir -p build
binary=${1:?Provide an absolute path to the tested Linux binary}
python3 "$project_root/scripts/check_linux_abi.py" "$binary" --max-glibc 2.36
cargo_command="$project_root/scripts/cargo-rust.sh"
version=$(python3 -c 'import tomllib; print(tomllib.load(open("rust/Cargo.toml", "rb"))["workspace"]["package"]["version"])')
mkdir -p "$project_root/dist"
stage=$(mktemp -d "$project_root/build/linux-package.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT HUP INT TERM
bundle="$stage/Clavunora-Rust-preview"
mkdir -p "$bundle/licenses/crates" "$bundle/resources/catalog" "$bundle/licenses/repertoire"
cp -R pianoflow/resources/catalog/. "$bundle/resources/catalog/"
cp "$binary" "$bundle/clavunora-rust.new"
mv -f "$bundle/clavunora-rust.new" "$bundle/clavunora-rust"
python3 "$project_root/scripts/check_linux_abi.py" "$bundle/clavunora-rust" --max-glibc 2.36 > "$bundle/linux-compatibility.json"
"$bundle/clavunora-rust" --self-test
if [ -x "$project_root/.venv/bin/python" ]; then
    "$project_root/.venv/bin/python" "$project_root/scripts/check_rust_compat.py" "$bundle/clavunora-rust"
fi
cp packaging/rust-linux/README.md "$bundle/README.md"
cp packaging/rust-linux/install.sh packaging/rust-linux/uninstall.sh "$bundle/"
cp pianoflow/resources/icons/pianoflow.png "$bundle/clavunora.png"
chmod +x "$bundle/install.sh" "$bundle/uninstall.sh"
cp LICENSE COPYRIGHT THIRD_PARTY_NOTICES.md AI_USAGE.md "$bundle/"
cp docs/BUILD_FROM_SOURCE.md "$bundle/BUILD_FROM_SOURCE.md"
source_name="Clavunora-Rust-$version-source.tar.gz"
printf '%s\n' "Corresponding source: $source_name" "Release: https://github.com/Redgion/PianoTrainer/releases/tag/rust-v$version" "Application code: GPL-3.0-only; separate resource licenses: THIRD_PARTY_NOTICES.md" > "$bundle/SOURCE.txt"
if [ "${CLAVUNORA_SKIP_SOURCE:-0}" != 1 ]; then
    python3 scripts/package_rust_source.py
fi
(cd dist && sha256sum -c "$source_name.sha256")
source_hash=$(sha256sum "dist/$source_name" | cut -d " " -f 1)
printf '%s\n' "Corresponding source: $source_name" "SHA256: $source_hash" "Code: GPL-3.0-only; resource licenses: THIRD_PARTY_NOTICES.md" > "$bundle/SOURCE.txt"
cp rust/Cargo.lock "$bundle/Cargo.lock"
cp rust/crates/clavunora-app/assets/FONT-LICENSE.txt "$bundle/licenses/Clavunora-brand-font-LICENSE.txt"
cp rust/crates/clavunora-app/assets/README.md "$bundle/licenses/Clavunora-artwork.md"
"$cargo_command" tree --locked -p clavunora-rust > build/release-dependencies.txt
python3 -c 'import os,sys; from pathlib import Path; text=Path("build/release-dependencies.txt").read_text(); Path(sys.argv[1]).write_text(text.replace(os.getcwd(), "~/Clavunora").replace(os.path.expanduser("~"), "~"))' "$bundle/dependencies.txt"
cp -R pianoflow/resources/licenses/. "$bundle/licenses/repertoire/"
mkdir -p "$bundle/licenses/rust"
cp -R packaging/rust-notices/. "$bundle/licenses/rust/"
registry_source="${CARGO_HOME:-$HOME/.cargo}/registry/src"
if [ -d "$project_root/build/rust-toolchain/cargo/registry/src" ]; then
    registry_source="$project_root/build/rust-toolchain/cargo/registry/src"
fi
# Include license/copyright notices for the downloaded registry packages, including font notices.
if [ -d "$registry_source" ]; then
    rg --files "$registry_source" | rg -i '/[^/]*(LICENSE|COPYING|NOTICE|COPYRIGHT)[^/]*$' > build/rust-license-files.txt
    while IFS= read -r source_file; do
        relative=${source_file#"$registry_source"/}
        destination="$bundle/licenses/crates/$relative"
        mkdir -p "$(dirname -- "$destination")"
        cp "$source_file" "$destination"
    done < build/rust-license-files.txt
fi
archive_name="Clavunora-Rust-$version-linux-$(uname -m)-glibc2.36.tar.gz"
python3 scripts/check_release_privacy.py "$bundle"
tar --owner=0 --group=0 --numeric-owner -czf "$project_root/dist/$archive_name" -C "$stage" Clavunora-Rust-preview
(cd "$project_root/dist" && sha256sum "$archive_name" > "$archive_name.sha256")
# Refresh the old download name too, so it no longer points to the Fedora-only binary.
generic_name="Clavunora-Rust-$version-linux-$(uname -m).tar.gz"
cp "$project_root/dist/$archive_name" "$project_root/dist/$generic_name.new"
mv -f "$project_root/dist/$generic_name.new" "$project_root/dist/$generic_name"
(cd "$project_root/dist" && sha256sum "$generic_name" > "$generic_name.sha256")
if [ -d "$project_root/dist/Clavunora-Rust-preview" ]; then
    mv "$project_root/dist/Clavunora-Rust-preview" "$project_root/build/previous-rust-bundle.$$"
fi
mv "$bundle" "$project_root/dist/Clavunora-Rust-preview"
printf '%s\n' "$project_root/dist/$archive_name"
