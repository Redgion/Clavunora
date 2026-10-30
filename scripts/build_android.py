# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Build an unsigned ARM64 release APK using the official Qt Android recipes."""
import argparse
import configparser
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
WHEELS = {
    'pyside6': '2cb8352ae6cd091c9f5e11d7bc4ba3bf54088548f63cd804adfdffe5c97c5eb2',
    'shiboken6': 'dc5dc4ca86c88327575a9d338c678a8f81f859a25a4dc0e6a539e465ab881660',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=Path, required=True)
    parser.add_argument('--ndk', type=Path, required=True)
    parser.add_argument('--stage', type=Path, default=ROOT/'build/android-package')
    parser.add_argument('--initialize-only', action='store_true')
    args = parser.parse_args()
    from pianoflow.branding import APP_NAME, ANDROID_PACKAGE
    sdk = args.sdk.resolve()
    # Qt's compatibility helper handles a flat cmdline-tools layout; CI SDKs
    # use cmdline-tools/latest, while Buildozer 1.6 still looks in tools/bin.
    if not (sdk/'tools/bin/sdkmanager').exists():
        for candidate in (sdk/'cmdline-tools/latest', sdk/'cmdline-tools'):
            if (candidate/'bin/sdkmanager').exists():
                (sdk/'tools').mkdir(exist_ok=True)
                for part in ('bin', 'lib'):
                    target = sdk/'tools'/part
                    if not target.exists(): target.symlink_to(candidate/part, target_is_directory=True)
                break
    if not (sdk/'tools/bin/sdkmanager').exists():
        raise RuntimeError('Android sdkmanager is missing')
    stage = args.stage.resolve()
    if not stage.exists():
        subprocess.run([sys.executable, str(ROOT/'scripts/prepare_android.py'), '--stage', '--output', str(stage)], check=True)
    wheels = ROOT/'build/android-wheels'
    wheels.mkdir(parents=True, exist_ok=True)
    for package, digest in WHEELS.items():
        name = f'{package}-6.12.0-6.12.0-cp310-abi3-android_aarch64.whl'
        path = wheels/name
        if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            temporary = path.with_suffix('.download')
            url = f'https://download.qt.io/official_releases/QtForPython/{package}/{name}'
            urllib.request.urlretrieve(url, temporary)
            if hashlib.sha256(temporary.read_bytes()).hexdigest() != digest:
                raise RuntimeError(f'Android wheel checksum mismatch: {package}')
            temporary.replace(path)
    deploy = [str(Path(sys.executable).parent/'pyside6-android-deploy'), '--name', APP_NAME,
              '--wheel-pyside', str(wheels/'pyside6-6.12.0-6.12.0-cp310-abi3-android_aarch64.whl'),
              '--wheel-shiboken', str(wheels/'shiboken6-6.12.0-6.12.0-cp310-abi3-android_aarch64.whl'),
              '--sdk-path', str(args.sdk.resolve()), '--ndk-path', str(args.ndk.resolve()),
              '--keep-deployment-files', '--no-install', '--force', '--init']
    subprocess.run(deploy, cwd=stage, check=True)
    spec = stage/'buildozer.spec'
    if not spec.exists(): raise RuntimeError('Qt did not generate buildozer.spec')
    config = configparser.ConfigParser(interpolation=None)
    config.read(spec)
    from pianoflow import __version__
    values = {
        'package.name': ANDROID_PACKAGE, 'package.domain': 'org.redgion',
        'version': __version__, 'android.numeric_version': '4002',
        'requirements': 'python3,sqlite3,shiboken6,PySide6',
        'source.include_exts': 'py,png,jpg,kv,atlas,qml,js,json,mid,midi,ly,ily,txt,mxl,mscx,md,musicxml,xml',
        'orientation': 'all', 'fullscreen': '0',
        'icon.filename': str(stage/'pianoflow/resources/icons/pianoflow.png'),
        'android.release_artifact': 'apk',
        'android.accept_sdk_license': 'True',
    }
    for key, value in values.items(): config.set('app', key, value)
    # A blank permissions token generates an invalid android.permission. entry.
    config.remove_option('app', 'android.permissions')
    with spec.open('w') as stream: config.write(stream)
    if args.initialize_only: return
    subprocess.run([sys.executable, '-m', 'buildozer', '-v', 'android', 'release'], cwd=stage, check=True)
    output = ROOT/'dist'
    output.mkdir(exist_ok=True)
    candidates = list(stage.rglob('*release*.apk'))
    if not candidates: raise RuntimeError('Build completed without a release APK')
    source = max(candidates, key=lambda p: p.stat().st_mtime)
    target = output/f'Clavunora-{__version__}-android-arm64-unsigned.apk'
    shutil.copy2(source, target)
    print(target)


if __name__ == '__main__': main()
