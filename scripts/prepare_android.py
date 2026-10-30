# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Stage clean sources for Qt Android deployment; does not claim to build an APK."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stage', action='store_true', help='Copy a clean source tree to build/android-source')
    parser.add_argument('--output', type=Path, default=ROOT / 'build' / 'android-source')
    args = parser.parse_args()
    sdk = os.environ.get('ANDROID_SDK_ROOT') or os.environ.get('ANDROID_HOME')
    ndk = os.environ.get('ANDROID_NDK_HOME')
    report = {'host': sys.platform, 'sdk': sdk if sdk and Path(sdk).is_dir() else None,
              'ndk': ndk if ndk and Path(ndk).is_dir() else None,
              'java': shutil.which('java'), 'adb': shutil.which('adb'),
              'deploy': shutil.which('pyside6-android-deploy') or (str(Path(sys.executable).parent / 'pyside6-android-deploy') if (Path(sys.executable).parent / 'pyside6-android-deploy').exists() else None),
              'apk_built': False,
              'remaining': ['Matching PySide6/Shiboken Android wheels', 'Android MIDI adapter',
                            'Small-screen layout', 'Device audio and multitouch verification',
                            'Release signing and complete repertoire review']}
    if args.stage:
        dest = args.output.resolve()
        if dest.exists():
            parser.error(f'{dest} already exists; retain or remove the old staging folder explicitly.')
        dest.mkdir(parents=True)
        shutil.copy2(ROOT / 'main.py', dest / 'main.py')
        shutil.copytree(ROOT / 'pianoflow', dest / 'pianoflow', ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
        # Deployment recipe supplies Qt; pure Python dependencies are vendored
        # from the installed distributions, including their license metadata.
        from importlib.metadata import distribution
        for package in ('mido', 'packaging'):
            dist = distribution(package)
            for file in dist.files or []:
                if '..' in file.parts or '__pycache__' in file.parts or file.suffix == '.pyc':
                    continue
                source = Path(dist.locate_file(file))
                if source.is_file():
                    target = dest / str(file); target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, target)
        (dest / 'Clavunora.pyproject').write_text(json.dumps({'files': [str(p.relative_to(dest)).replace(os.sep, '/') for p in dest.rglob('*') if p.is_file()]}, indent=2))
        shutil.copy2(ROOT / 'docs' / 'ANDROID.md', dest / 'README.md')
        report['staged'] = str(dest)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
