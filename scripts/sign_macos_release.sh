#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
# Run on the owner's Mac after importing a Developer ID certificate into Keychain.
set -eu
app=${1:?Provide Clavunora.app}
identity=${CLAVUNORA_SIGNING_IDENTITY:?Set the Developer ID Application identity}
profile=${CLAVUNORA_NOTARY_PROFILE:?Set the existing notarytool Keychain profile name}
codesign --force --deep --options runtime --timestamp --sign "$identity" "$app"
codesign --verify --deep --strict --verbose=2 "$app"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
ditto -c -k --keepParent "$app" "$work/Clavunora.zip"
xcrun notarytool submit "$work/Clavunora.zip" --keychain-profile "$profile" --wait
xcrun stapler staple "$app"
xcrun stapler validate "$app"
spctl --assess --type execute --verbose=2 "$app"
printf '%s\n' 'Signed and notarized app ready. Recreate the ZIP/DMG and checksums from this app before publishing.'
