#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
if [ "$(uname -m)" != x86_64 ]; then
    printf '%s\n' 'This builder currently supports x86_64 Linux only.' >&2
    exit 1
fi
command -v podman >/dev/null
# The cached official Rust toolchain runs on older glibc; all C objects and final links use Debian.
toolchain="$project_root/build/rust-toolchain/rustup/toolchains/stable-x86_64-unknown-linux-gnu"
if [ ! -x "$toolchain/bin/cargo" ]; then
    printf '%s\n' 'Install the Rust toolchain into build/rust-toolchain before using this offline builder.' >&2
    exit 1
fi
mkdir -p build/rust-compat
podman build -f packaging/rust-linux/Containerfile.compat -t localhost/clavunora-rust-builder:debian12 packaging/rust-linux
podman run --rm --network=none --userns=keep-id --security-opt label=disable \
    -v "$project_root:/workspace:ro" \
    -v "$project_root/build/rust-toolchain/cargo:/cargo:rw" \
    -v "$project_root/build/rust-compat:/output:rw" \
    localhost/clavunora-rust-builder:debian12 sh /workspace/scripts/rust_compat_container_build.sh
python3 scripts/check_linux_abi.py "$project_root/build/rust-compat/target/release/clavunora-rust" --max-glibc 2.36
exec "$project_root/scripts/package_rust_linux.sh" "$project_root/build/rust-compat/target/release/clavunora-rust"
