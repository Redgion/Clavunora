#!/bin/sh
set -eu
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
target="$data_home/Clavunora"
apps="$data_home/applications"
launcher_name=Clavunora.desktop
legacy="$data_home/PianoFlow"
# Reuse an installation belonging to this app, including pinned launcher IDs.
if [ -d "$legacy/_internal/pianoflow" ]; then
    target="$legacy"
    if [ -f "$apps/PianoFlow.desktop" ]; then launcher_name=PianoFlow.desktop; fi
fi
if [ "$source_dir" = "$target" ]; then
    printf '%s\n' 'Clavunora is already installed here.'
    exit 0
fi
if [ ! -x "$source_dir/Clavunora" ] || [ ! -d "$source_dir/_internal" ]; then
    printf '%s\n' 'Extract the complete archive before installing.' >&2
    exit 1
fi
mkdir -p "$target" "$apps"
cp -R "$source_dir/." "$target/"
# Desktop Entry Exec quoting; no system Python installation is required.
launcher=$(printf '%s' "$target/Clavunora" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g; s/%/%%/g')
cat > "$apps/$launcher_name" <<EOF
[Desktop Entry]
Type=Application
Name=Clavunora
Comment=Offline piano practice
Exec="$launcher"
Icon=$target/clavunora.png
Terminal=false
Categories=AudioVideo;Education;Music;
EOF
printf '%s\n' "Installed: $target" 'Open Clavunora from the application menu.'
