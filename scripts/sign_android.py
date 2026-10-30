# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Sign a release APK locally; signing material never belongs in the repository."""
import argparse
from pathlib import Path
import secrets
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('apk', type=Path)
    parser.add_argument('--jdk', type=Path, required=True)
    parser.add_argument('--build-tools', type=Path, required=True)
    parser.add_argument('--keys', type=Path, default=Path('build/release-signing'))
    args = parser.parse_args()
    keys = args.keys.resolve(); keys.mkdir(parents=True, exist_ok=True); keys.chmod(0o700)
    password = keys/'store-password.txt'
    store = keys/'pianoflow-release.p12'
    if not password.exists():
        if store.exists(): raise RuntimeError('Existing signing key is missing its password file')
        password.write_text(secrets.token_urlsafe(48)); password.chmod(0o600)
    if not store.exists():
        subprocess.run([str(args.jdk/'bin/keytool'), '-genkeypair', '-keystore', str(store),
            '-storetype', 'PKCS12', '-alias', 'pianoflow', '-storepass:file', str(password),
            '-keyalg', 'RSA', '-keysize', '3072', '-validity', '10000',
            '-dname', 'CN=Clavunora, OU=Release, O=Redgion'], check=True)
        store.chmod(0o600)
    aligned = args.apk.with_name(args.apk.stem+'-aligned.apk')
    output = args.apk.with_name(args.apk.name.replace('-unsigned', ''))
    if output == args.apk: raise ValueError('Input must end with -unsigned.apk')
    subprocess.run([str(args.build_tools/'zipalign'), '-f', '-P', '16', '4', str(args.apk), str(aligned)], check=True)
    subprocess.run([str(args.build_tools/'apksigner'), 'sign', '--ks', str(store), '--ks-key-alias', 'pianoflow',
        '--ks-pass', 'file:'+str(password), '--out', str(output), str(aligned)], check=True)
    subprocess.run([str(args.build_tools/'apksigner'), 'verify', '--verbose', '--print-certs', str(output)], check=True)
    aligned.unlink()
    print(output)


if __name__ == '__main__': main()
