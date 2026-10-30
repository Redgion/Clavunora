# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Reject Linux executables that require a newer glibc than the supported baseline."""
import argparse
import json
from pathlib import Path
import re
import subprocess


def version(value):
    return tuple(map(int, value.split('.')))


def inspect(binary):
    output = subprocess.run(['readelf', '--version-info', str(binary)], capture_output=True, text=True, check=True).stdout
    required = sorted(set(re.findall(r'GLIBC_(\d+(?:\.\d+)+)', output)), key=version)
    if not required:
        raise SystemExit('Cannot determine glibc requirements; refusing to package an unchecked binary')
    special = sorted(set(re.findall(r'Name: (GLIBC_[A-Z][A-Z0-9_]*)', output)))
    unknown = set(special) - {'GLIBC_ABI_DT_RELR'}
    if unknown:
        raise SystemExit(f'Unknown glibc ABI requirements: {sorted(unknown)}')
    # Packed relative relocations carry an ABI marker instead of a numbered symbol version.
    if 'GLIBC_ABI_DT_RELR' in special and version(required[-1]) < version('2.36'):
        required.append('2.36')
    dynamic = subprocess.run(['readelf', '-d', str(binary)], capture_output=True, text=True, check=True).stdout
    return {'glibc_required': required[-1], 'glibc_versions': required, 'glibc_abi_markers': special,
            'shared_libraries': re.findall(r'\(NEEDED\).*?\[(.*?)\]', dynamic)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--max-glibc', default='2.36')
    args = parser.parse_args()
    report = inspect(args.binary)
    report['glibc_baseline'] = args.max_glibc
    if version(report['glibc_required']) > version(args.max_glibc):
        raise SystemExit(f"Requires glibc {report['glibc_required']}, baseline is {args.max_glibc}. "
                         'Build with scripts/build_rust_linux_compat.sh instead of the host toolchain.')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
