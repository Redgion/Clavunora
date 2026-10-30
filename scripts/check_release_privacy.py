#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Block release inputs containing credentials, profiles or local home paths.

This is a release guard, not proof that arbitrary personal data can be detected.
Copyright notices and public attribution must remain intact.
"""
import argparse
from pathlib import Path
import re


PATTERNS = {
    'private key': rb'-----BEGIN ' + rb'(?:RSA |EC |OPENSSH )?PRIVATE KEY-----',
    'access token': rb'(?:gh[pousr]_' + rb'[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|AKIA[0-9A-Z]{16}|sk-proj-[A-Za-z0-9_-]{30,})',
    # Require a path boundary: crates.io/crates/home/ is not a home folder.
    'home path': (rb'(?<![A-Za-z0-9./:])(?:/' + rb'home/[^/\s\x00]+/|/' +
                  rb'Users/[^/\s\x00]+/|[A-Za-z]:[\\/]Users[\\/][^\\/\s\x00]+[\\/])'),
}
FORBIDDEN_NAMES = {'credentials', 'id_rsa', 'id_ed25519', '.ds_store', 'thumbs.db'}
FORBIDDEN_SUFFIXES = {'.sqlite', '.sqlite3', '.db', '.key', '.pem', '.p12', '.pfx', '.clavunora-backup', '.log'}


def issues(path):
    path = Path(path)
    name = path.name.lower()
    result = []
    if (name in FORBIDDEN_NAMES or name.startswith('.env') or
            path.suffix.lower() in FORBIDDEN_SUFFIXES or '.sqlite' in name or
            name.endswith(('.db-wal', '.db-shm'))):
        result.append('private file type')
    if path.is_symlink():
        return result + ['unexpected symbolic link']
    data = path.read_bytes()
    # Also inspect UTF-16 strings emitted by Windows toolchains.
    for kind, pattern in PATTERNS.items():
        if re.search(pattern, data) or re.search(pattern, data.replace(b'\x00', b'')):
            result.append(kind)
    return result


def check_files(files, root):
    findings = []
    for path in files:
        for kind in issues(path):
            # Report only relative locations and categories, never matching secrets.
            findings.append(f'{Path(path).relative_to(root)}: {kind}')
    if findings:
        raise ValueError('Release privacy check failed:\n' + '\n'.join(findings))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('folder', type=Path)
    args = parser.parse_args()
    root = args.folder.resolve()
    files = [p for p in root.rglob('*') if p.is_file()]
    check_files(files, root)
    print(f'Release privacy check passed: {len(files)} files')


if __name__ == '__main__':
    main()
