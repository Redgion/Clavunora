# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Build and smoke-test the current host's portable distribution."""
import argparse
from pathlib import Path
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from importlib.metadata import distribution

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-tests', action='store_true')
    args = parser.parse_args()
    if sys.platform not in ('linux', 'win32'):
        parser.error('This build recipe supports Linux and Windows hosts.')
    env = dict(os.environ, QT_QPA_PLATFORM='offscreen', PYINSTALLER_CONFIG_DIR=str(ROOT / 'build' / 'pyinstaller-cache'))
    def run(*command):
        subprocess.run(command, cwd=ROOT, env=env, check=True)
    if not args.skip_tests:
        run(sys.executable, '-m', 'pytest', '-q', '-p', 'no:cacheprovider')
    run(sys.executable, '-m', 'PyInstaller', '--noconfirm', '--distpath', str(ROOT / 'dist'),
        '--workpath', str(ROOT / 'build' / 'desktop'), str(ROOT / 'packaging' / 'Clavunora.spec'))
    bundle = ROOT / 'dist' / 'Clavunora'
    executable = bundle / ('Clavunora.exe' if sys.platform == 'win32' else 'Clavunora')
    # Do not let source files in the checkout accidentally satisfy frozen imports.
    with tempfile.TemporaryDirectory(prefix='pianoflow-bundle-check-') as clean_dir:
        subprocess.run([str(executable), '--self-test'], cwd=clean_dir, env=env, check=True)
    shutil.copy2(ROOT / 'docs' / 'DISTRIBUTION.md', bundle / 'README.md')
    if sys.platform == 'linux':
        shutil.copy2(ROOT / 'packaging' / 'linux' / 'install.sh', bundle / 'install.sh')
    shutil.copy2(ROOT / 'pianoflow/resources/icons/pianoflow.png', bundle / 'clavunora.png')
    licenses = bundle / 'licenses'
    shutil.copytree(ROOT / 'pianoflow' / 'resources' / 'licenses', licenses, dirs_exist_ok=True)
    versions = []
    for package in ('PySide6', 'PySide6_Essentials', 'PySide6_Addons', 'shiboken6', 'mido', 'python-rtmidi', 'packaging'):
        dist = distribution(package)
        versions.append(f'{package}=={dist.version}')
        for file in dist.files or []:
            if any(word in str(file).lower() for word in ('license', 'copying', 'notice')):
                source = Path(dist.locate_file(file))
                if source.is_file():
                    target = licenses / package / str(file)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, target)
    (licenses / 'versions.txt').write_text('\n'.join(versions), encoding='utf-8')
    from pianoflow import __version__
    archive = ROOT / 'dist' / f'Clavunora-{__version__}-{platform.system().lower()}-{platform.machine()}'
    print(shutil.make_archive(str(archive), 'zip' if sys.platform == 'win32' else 'gztar', root_dir=bundle.parent, base_dir=bundle.name))


if __name__ == '__main__':
    main()
