#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
# Invoked inside the Debian 12 builder, with the source mounted read-only.
set -eu
cd /workspace/rust
toolchain=/workspace/build/rust-toolchain/rustup/toolchains/stable-x86_64-unknown-linux-gnu
export PATH="$toolchain/bin:$PATH"
export RUSTC="$toolchain/bin/rustc"
export RUSTDOC="$toolchain/bin/rustdoc"
export RUSTUP_HOME=/workspace/build/rust-toolchain/rustup
export RUSTUP_TOOLCHAIN=stable
export CARGO_HOME=/cargo
export CARGO_TARGET_DIR=/output/target
export CARGO_BUILD_JOBS=4
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=/cargo=~/.cargo --remap-path-prefix=/workspace=~/Clavunora"
ldd --version | head -n 1
rustc --version
cargo fmt --all -- --check
cargo test --offline --locked --workspace
cargo clippy --offline --locked --workspace --all-targets -- -D warnings
cargo build --offline --locked --release -p clavunora-rust
# readelf is the actual baseline gate, not the compiler's declared target.
readelf --version-info /output/target/release/clavunora-rust > /output/elf-versions.txt
/output/target/release/clavunora-rust --self-test > /output/self-test.json
# Exercise real X11/OpenGL in the old-libc environment using software rendering.
export LIBGL_ALWAYS_SOFTWARE=1
xvfb-run -a /output/target/release/clavunora-rust --data-dir /output/native-profile \
    --capture /output/compatibility-window.png --capture-page library-songs
