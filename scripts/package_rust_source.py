#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Create reviewed corresponding source, including offline Cargo dependencies."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
try:
    from scripts.check_release_privacy import check_files
except ModuleNotFoundError:
    from check_release_privacy import check_files

ROOT = Path(__file__).resolve().parents[1]
DIRECTORIES = ('rust', 'pianoflow', 'scripts', 'tests', 'assets', 'packaging', '.github', 'docs')
EXCLUDED = {'target', '__pycache__', '.pytest_cache', '.venv', '.git', '.aws', '.codex', '.agents', 'screenshots'}
FORBIDDEN = {'.sqlite', '.sqlite3', '.db', '.pyc', '.log', '.clavunora-backup', '.pem', '.key', '.p12', '.pfx'}


def source_files(root=ROOT):
    """An allowlist, never a recursive archive of the developer's home/workspace."""
    files = []
    for name in ('LICENSE', 'COPYRIGHT', 'THIRD_PARTY_NOTICES.md', 'AI_USAGE.md', 'README.md', 'CHANGELOG.md', 'pyproject.toml', 'main.py', '.gitignore', '.gitattributes'):
        p = root / name
        if p.is_file():
            files.append(p)
    for name in DIRECTORIES:
        for p in (root / name).rglob('*'):
            relative = p.relative_to(root)
            if any(part in EXCLUDED or part.startswith('.env') for part in relative.parts):
                continue
            if p.is_symlink():
                raise ValueError(f'Symlink is not a release source: {relative}')
            if p.is_file() and p.suffix not in FORBIDDEN and not any(marker in p.name.lower() for marker in ('.sqlite', '.db-wal', '.db-shm')):
                files.append(p)
    return sorted(set(files))


def digest_files():
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in source_files()}


def public_tar_metadata(info):
    # Filesystem ownership is local machine metadata, not project authorship.
    info.uid = info.gid = 0
    info.uname = info.gname = ''
    info.pax_headers = {k: v for k, v in info.pax_headers.items()
                        if not k.startswith(('SCHILY.', 'LIBARCHIVE.'))}
    return info


def main():
    version = tomllib.loads((ROOT / 'rust/Cargo.toml').read_text())['workspace']['package']['version']
    name = f'Clavunora-Rust-{version}-source'
    out = ROOT / 'dist'
    out.mkdir(exist_ok=True)
    (ROOT / 'build').mkdir(exist_ok=True)
    before = digest_files()
    check_files(source_files(), ROOT)
    with tempfile.TemporaryDirectory(prefix='clavunora-source-', dir=ROOT / 'build') as tmp:
        stage = Path(tmp) / name
        stage.mkdir()
        for p in source_files():
            target = stage / p.relative_to(ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(p, target)
        # --offline ensures the release includes the exact cached, locked sources.
        subprocess.run(([str(ROOT / 'scripts/cargo-rust.sh')] if os.name != 'nt' else ['cargo']) + ['vendor', '--locked', '--offline', str(stage / 'vendor')], check=True, stdout=subprocess.DEVNULL)
        config = stage / 'rust/.cargo/config.toml'
        config.parent.mkdir(exist_ok=True)
        config.write_text('[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "../vendor"\n[net]\noffline = true\n')
        # Protect against packaging different source while a developer edits files.
        if before != digest_files():
            raise RuntimeError('Source changed while creating the archive; rebuild and retry')
        (stage / 'SOURCE_FILES.json').write_text(json.dumps(before, indent=2, ensure_ascii=False) + '\n')
        env = dict(os.environ)
        env['CARGO_TARGET_DIR'] = str(ROOT / 'build/source-metadata-check')
        cargo = ROOT / 'build/rust-toolchain/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo'
        command = str(cargo) if cargo.is_file() else 'cargo'
        if cargo.is_file():
            env['RUSTC'] = str(cargo.parent / 'rustc')
            env['RUSTDOC'] = str(cargo.parent / 'rustdoc')
            env['PATH'] = str(cargo.parent) + os.pathsep + env.get('PATH', '')
        # Parse and resolve the staged project entirely from its own vendor folder.
        subprocess.run([command, 'metadata', '--manifest-path', str(stage / 'rust/Cargo.toml'), '--locked', '--offline', '--format-version', '1'], cwd=stage / 'rust', env=env, check=True, stdout=subprocess.DEVNULL)
        target = out / (name + '.tar.gz')
        temporary = target.with_suffix('.tmp')
        with tarfile.open(temporary, 'w:gz') as archive:
            archive.add(stage, arcname=name, filter=public_tar_metadata)
        temporary.replace(target)
    sha = hashlib.sha256(target.read_bytes()).hexdigest()
    Path(str(target) + '.sha256').write_text(f'{sha}  {target.name}\n')
    print(target)


if __name__ == '__main__':
    main()
