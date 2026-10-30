# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
$ErrorActionPreference = 'Stop'
$version = python -c "import tomllib; print(tomllib.load(open('rust/Cargo.toml','rb'))['workspace']['package']['version'])"
$compiler = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (!(Test-Path $compiler)) { throw 'Inno Setup 6 is required' }
& $compiler "/DAppVersion=$version" packaging/rust-windows/Clavunora.iss
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
$installer = (Resolve-Path "dist/Clavunora-$version-windows-x64-setup.exe").Path
$target = Join-Path $env:RUNNER_TEMP 'Clavunora installer check'
$profile = Join-Path $env:APPDATA 'Clavunora'
New-Item -ItemType Directory -Force $profile | Out-Null
$sentinel = Join-Path $profile 'installer-check.txt'
Set-Content $sentinel 'preserve-progress'
foreach ($pass in 1..2) {
    $install = Start-Process $installer -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', ('/DIR="' + $target + '"')) -Wait -PassThru
    if ($install.ExitCode -ne 0) { throw "Install/upgrade failed: $($install.ExitCode)" }
    $check = Start-Process (Join-Path $target 'clavunora-rust.exe') -ArgumentList '--self-test' -Wait -PassThru
    if ($check.ExitCode -ne 0) { throw 'Installed self-test failed' }
    if ((Get-Content $sentinel) -ne 'preserve-progress') { throw 'Upgrade changed the profile' }
}
$remove = Start-Process (Join-Path $target 'unins000.exe') -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait -PassThru
if ($remove.ExitCode -ne 0) { throw 'Uninstall failed' }
if ((Get-Content $sentinel) -ne 'preserve-progress') { throw 'Uninstall removed the profile' }
python -c "from pathlib import Path; from scripts.package_rust_desktop import checksum; checksum(Path(r'$installer'))"
