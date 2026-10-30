#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
set -eu
data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
case "$data_home" in /*) ;; *) printf '%s\n' 'XDG_DATA_HOME must be absolute.' >&2; exit 1;; esac
target="$data_home/ClavunoraRust"
if [ ! -f "$target/.clavunora-install" ] || [ "$(cat "$target/.clavunora-install")" != 'org.redgion.clavunora' ]; then
    printf '%s\n' 'Clavunora installation was not found; nothing removed.' >&2; exit 1
fi
rm -f -- "$data_home/applications/org.redgion.clavunora.desktop"
rm -rf -- "$target"
printf '%s\n' 'Clavunora removed. Your library, settings and progress were preserved.'
