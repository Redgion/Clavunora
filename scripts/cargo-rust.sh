#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [ -x "$project_root/build/rust-toolchain/cargo/bin/cargo" ]; then
    export CARGO_HOME="$project_root/build/rust-toolchain/cargo"
    export RUSTUP_HOME="$project_root/build/rust-toolchain/rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi
export CARGO_TARGET_DIR="$project_root/build/rust-target"
cd "$project_root/rust"
exec cargo "$@"
