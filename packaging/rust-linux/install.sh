#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
set -eu
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
case "$data_home" in /*) ;; *) printf '%s\n' 'XDG_DATA_HOME must be absolute.' >&2; exit 1;; esac
case "$data_home" in *'
'*) printf '%s\n' 'Installation path cannot contain a newline.' >&2; exit 1;; esac
target="$data_home/ClavunoraRust"
marker="$target/.clavunora-install"
if [ -d "$target" ] && [ ! -f "$marker" ]; then
    printf '%s\n' 'Installation folder is not owned by Clavunora. Move it before installing.' >&2
    exit 1
fi
if [ ! -x "$source_dir/clavunora-rust" ] || [ ! -d "$source_dir/resources/catalog" ]; then
    printf '%s\n' 'Extract the complete archive before installing.' >&2; exit 1
fi
mkdir -p "$target/releases" "$data_home/applications"
printf '%s\n' 'org.redgion.clavunora' > "$marker"
stage=$(mktemp -d "$target/releases/.install.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT HUP INT TERM
cp -R "$source_dir/." "$stage/"
# A versioned directory allows replacing an installed binary while it is running.
release="release-$(date +%s)-$$"
mv -- "$stage" "$target/releases/$release"
ln -s "releases/$release" "$target/.current-$$"
mv -Tf -- "$target/.current-$$" "$target/current"
launcher=$(printf '%s' "$target/current/clavunora-rust" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g; s/%/%%/g')
cat > "$data_home/applications/org.redgion.clavunora.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Clavunora
Comment=Offline piano practice
Exec="$launcher"
Icon=$target/current/clavunora.png
Terminal=false
Categories=AudioVideo;Education;Music;
StartupWMClass=org.redgion.clavunora
EOF
printf '%s\n' "Installed: $target/current" 'Open Clavunora from the application menu. Your profile is preserved.'
