#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Package a natively built/tested Windows or macOS candidate, with matching source."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import struct
import subprocess
import tempfile
import tomllib
import zipfile
try:
    from scripts.check_release_privacy import check_files
except ModuleNotFoundError:
    from check_release_privacy import check_files

ROOT = Path(__file__).resolve().parents[1]


def pe_imports(path):
    """Read imports without requiring Visual Studio tools on an end-user machine."""
    raw = path.read_bytes()
    if raw[:2] != b'MZ':
        raise ValueError('Not a Windows executable')
    pe = struct.unpack_from('<I', raw, 0x3c)[0]
    if raw[pe:pe + 4] != b'PE\0\0':
        raise ValueError('Invalid PE header')
    machine, sections = struct.unpack_from('<HH', raw, pe + 4)
    if machine != 0x8664:
        raise ValueError('The Windows package requires an x64 executable')
    optional_size = struct.unpack_from('<H', raw, pe + 20)[0]
    optional = pe + 24
    if struct.unpack_from('<H', raw, optional)[0] != 0x20b:
        raise ValueError('Expected PE32+')
    table = optional + optional_size
    def offset(rva):
        for index in range(sections):
            entry = table + index * 40
            size, address, disk_size, disk = struct.unpack_from('<IIII', raw, entry + 8)
            if address <= rva < address + max(size, disk_size):
                return disk + rva - address
        raise ValueError('Invalid import address')
    import_rva = struct.unpack_from('<I', raw, optional + 120)[0]
    imports = []
    if import_rva:
        descriptor = offset(import_rva)
        while any(raw[descriptor:descriptor + 20]):
            name_rva = struct.unpack_from('<I', raw, descriptor + 12)[0]
            start = offset(name_rva)
            imports.append(raw[start:raw.index(b'\0', start)].decode('ascii'))
            descriptor += 20
    if any(re.match(r'(vcruntime|msvcp|msvcr)\d', name, re.I) for name in imports):
        raise ValueError('Build with -C target-feature=+crt-static before packaging')
    return sorted(imports)


def mac_metadata(version):
    numbers = [int(x) for x in version.split('-')[0].split('.')]
    rc = int(version.rsplit('.', 1)[1]) if '-rc.' in version else 99
    return dict(CFBundleName='Clavunora', CFBundleDisplayName='Clavunora',
                CFBundleIdentifier='org.redgion.clavunora', CFBundleExecutable='clavunora-rust',
                CFBundlePackageType='APPL', CFBundleIconFile='Clavunora.icns',
                CFBundleShortVersionString=version.split('-')[0],
                CFBundleVersion=str(numbers[0] * 1_000_000 + numbers[1] * 10_000 + numbers[2] * 100 + rc),
                LSMinimumSystemVersion='11.0', NSHighResolutionCapable=True,
                NSHumanReadableCopyright='Copyright © 2026 redgion and Clavunora contributors',
                NSPrincipalClass='NSApplication')


def notices(destination):
    for name in ('LICENSE', 'COPYRIGHT', 'THIRD_PARTY_NOTICES.md', 'AI_USAGE.md'):
        shutil.copy2(ROOT / name, destination / name)
    shutil.copy2(ROOT / 'docs/BUILD_FROM_SOURCE.md', destination / 'BUILD_FROM_SOURCE.md')
    shutil.copy2(ROOT / 'rust/Cargo.lock', destination / 'Cargo.lock')
    licenses = destination / 'licenses'
    shutil.copytree(ROOT / 'pianoflow/resources/licenses', licenses / 'repertoire', dirs_exist_ok=True)
    shutil.copy2(ROOT / 'rust/crates/clavunora-app/assets/FONT-LICENSE.txt', licenses / 'brand-font-LICENSE.txt')
    shutil.copy2(ROOT / 'rust/crates/clavunora-app/assets/README.md', licenses / 'artwork.md')
    registry = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')) / 'registry/src'
    for path in registry.rglob('*'):
        if path.is_file() and re.search(r'license|copying|notice|copyright', path.name, re.I):
            target = licenses / 'crates' / path.relative_to(registry)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
    shutil.copytree(ROOT / 'packaging/rust-notices', licenses / 'rust', dirs_exist_ok=True)
    sysroot = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
    rust_docs = sysroot / 'share/doc/rust'
    if (rust_docs / 'licenses').is_dir():
        shutil.copytree(rust_docs / 'licenses', licenses / 'rust', dirs_exist_ok=True)
        for path in rust_docs.glob('COPYRIGHT*'):
            shutil.copy2(path, licenses / 'rust' / path.name)


def checksum(path):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    Path(str(path) + '.sha256').write_text(f'{digest}  {path.name}\n')


def portable_zip(bundle, archive):
    # Registry license files may have reproducible Unix-epoch timestamps.
    # ZIP represents years 1980-2107; clamp metadata without altering contents.
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, strict_timestamps=False) as zipped:
        for path in sorted(bundle.rglob('*')):
            if path.is_file(): zipped.write(path, Path('Clavunora') / path.relative_to(bundle))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True)
    args = parser.parse_args()
    system = platform.system()
    if system not in ('Windows', 'Darwin'):
        raise SystemExit('Use package_rust_linux.sh for Linux; native packaging is required.')
    version = tomllib.loads((ROOT / 'rust/Cargo.toml').read_text())['workspace']['package']['version']
    source = args.source.resolve()
    if source.name != f'Clavunora-Rust-{version}-source.tar.gz' or not source.is_file():
        raise SystemExit('Supply the corresponding source archive for this exact version.')
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    out = ROOT / 'dist'; out.mkdir(exist_ok=True)
    if system == 'Windows':
        bundle = out / 'Clavunora-Windows'
        resource = bundle
        executable = bundle / 'clavunora-rust.exe'
    else:
        bundle = out / 'Clavunora.app'
        resource = bundle / 'Contents/Resources'
        executable = bundle / 'Contents/MacOS/clavunora-rust'
    if bundle.exists(): shutil.rmtree(bundle)
    resource.mkdir(parents=True)
    executable.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(args.binary, executable)
    catalog = resource / ('resources/catalog' if system == 'Windows' else 'catalog')
    shutil.copytree(ROOT / 'pianoflow/resources/catalog', catalog)
    notices(resource)
    (resource / 'SOURCE.txt').write_text(f'Corresponding source: {source.name}\nSHA256: {source_hash}\nCode: GPL-3.0-only. Separate resource licenses: THIRD_PARTY_NOTICES.md\n')
    (resource / 'README.txt').write_text('Clavunora '+version+'\nOffline piano practice. GPLv3.\nLibrary, settings and progress live in your user profile and survive application updates.\nWindows: run clavunora-rust.exe or use the setup installer.\nmacOS: drag Clavunora.app into Applications. This test candidate is ad-hoc signed, not notarized.\n', encoding='utf-8')
    if system == 'Windows':
        dependencies = pe_imports(executable)
    else:
        with (bundle / 'Contents/Info.plist').open('wb') as handle:
            plistlib.dump(mac_metadata(version), handle)
        with tempfile.TemporaryDirectory(suffix='.iconset') as tmp:
            iconset = Path(tmp)
            for size in (16, 32, 128, 256, 512):
                for scale in (1, 2):
                    target = iconset / f'icon_{size}x{size}{"@2x" if scale == 2 else ""}.png'
                    subprocess.run(['sips', '-z', str(size * scale), str(size * scale), str(ROOT / 'pianoflow/resources/icons/pianoflow.png'), '--out', str(target)], check=True, stdout=subprocess.DEVNULL)
            subprocess.run(['iconutil', '-c', 'icns', str(iconset), '-o', str(resource / 'Clavunora.icns')], check=True)
        dependencies = subprocess.check_output(['otool', '-L', str(executable)], text=True).splitlines()[1:]
        if any(not line.strip().startswith(('/System/Library/', '/usr/lib/')) for line in dependencies):
            raise SystemExit('Non-system dylib found; bundle and sign it before distribution.')
    check = subprocess.run([str(executable), '--self-test'], cwd=tempfile.gettempdir(), text=True, encoding="utf-8", capture_output=True, check=True, timeout=120)
    result = json.loads(check.stdout.splitlines()[-1])
    if not result.get('ok') or result.get('library_items') != 124:
        raise SystemExit('Packaged self-test failed')
    info = dict(version=version, platform=system, architecture=platform.machine(),
                source_archive=source.name, source_sha256=source_hash,
                binary_sha256_before_signing=hashlib.sha256(executable.read_bytes()).hexdigest(),
                self_test=True, library_items=124, dependencies=dependencies,
                signing='unsigned' if system == 'Windows' else 'ad-hoc, not notarized')
    (resource / 'BUILD_INFO.json').write_text(json.dumps(info, indent=2)+'\n')
    check_files([p for p in bundle.rglob('*') if p.is_file()], bundle)
    if system == 'Windows':
        archive = out / f'Clavunora-{version}-windows-x64-portable.zip'
        portable_zip(bundle, archive)
    else:
        subprocess.run(['codesign', '--force', '--deep', '--sign', '-', str(bundle)], check=True)
        subprocess.run(['codesign', '--verify', '--deep', '--strict', '--verbose=2', str(bundle)], check=True)
        arch = 'arm64' if platform.machine() == 'arm64' else 'x86_64'
        archive = out / f'Clavunora-{version}-macos-{arch}.zip'
        subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(bundle), str(archive)], check=True)
        dmg = out / f'Clavunora-{version}-macos-{arch}.dmg'
        if dmg.exists(): dmg.unlink()
        with tempfile.TemporaryDirectory() as tmp:
            staging = Path(tmp)
            shutil.copytree(bundle, staging / bundle.name)
            os.symlink('/Applications', staging / 'Applications')
            subprocess.run(['hdiutil', 'create', '-volname', 'Clavunora', '-srcfolder', str(staging), '-ov', '-format', 'UDZO', str(dmg)], check=True)
        checksum(dmg)
    checksum(archive)
    print(archive)


if __name__ == '__main__': main()
