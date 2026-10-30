#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
# Distribution builds must not inherit a newer glibc from the developer's host.
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "$project_root/scripts/build_rust_linux_compat.sh" "$@"
