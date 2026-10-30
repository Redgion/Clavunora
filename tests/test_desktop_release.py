# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Exercise install/upgrade/uninstall only in a disposable user directory."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]


class PortableArchive(unittest.TestCase):
    def test_old_license_timestamps_do_not_prevent_packaging_or_change_notice_contents(self):
        from scripts.package_rust_desktop import portable_zip
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); bundle = root / 'bundle'; bundle.mkdir()
            notice = bundle / 'LICENSE'
            notice.write_bytes(b'Original upstream notice\n')
            os.utime(notice, (0, 0))
            archive = root / 'portable.zip'
            portable_zip(bundle, archive)
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(zipped.read('Clavunora/LICENSE'), notice.read_bytes())
                self.assertEqual(zipped.getinfo('Clavunora/LICENSE').date_time[0], 1980)


@unittest.skipUnless(os.name == 'posix' and __import__('sys').platform == 'linux', 'Linux user installer')
class LinuxInstallation(unittest.TestCase):
    def fixture(self, root):
        source = root / 'release bundle'
        source.mkdir()
        for name in ('install.sh', 'uninstall.sh'):
            shutil.copy2(ROOT / 'packaging/rust-linux' / name, source / name)
        (source / 'resources/catalog').mkdir(parents=True)
        (source / 'clavunora-rust').write_text('#!/bin/sh\nexit 0\n')
        (source / 'clavunora-rust').chmod(0o755)
        (source / 'clavunora.png').write_bytes(b'icon')
        env = dict(os.environ, XDG_DATA_HOME=str(root / 'Data 100% with spaces'), XDG_CONFIG_HOME=str(root / 'profiles'))
        return source, env

    def test_upgrade_switches_complete_release_and_uninstall_preserves_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source, env = self.fixture(root)
            profile = Path(env['XDG_CONFIG_HOME']) / 'Clavunora/library.sqlite3'
            profile.parent.mkdir(parents=True)
            profile.write_bytes(b'keep-progress')
            (source / 'obsolete.txt').write_text('first release')
            subprocess.run(['sh', str(source / 'install.sh')], env=env, check=True, capture_output=True)
            install = Path(env['XDG_DATA_HOME']) / 'ClavunoraRust'
            first = (install / 'current').resolve()
            self.assertTrue((first / 'obsolete.txt').exists())
            (source / 'obsolete.txt').unlink()
            (source / 'new.txt').write_text('second release')
            subprocess.run(['sh', str(source / 'install.sh')], env=env, check=True, capture_output=True)
            second = (install / 'current').resolve()
            self.assertNotEqual(first, second)
            self.assertTrue((second / 'new.txt').is_file())
            self.assertFalse((second / 'obsolete.txt').exists())
            self.assertTrue(first.is_dir())
            desktop = Path(env['XDG_DATA_HOME']) / 'applications/org.redgion.clavunora.desktop'
            self.assertIn('Data 100%% with spaces', desktop.read_text())
            subprocess.run([str(second / 'clavunora-rust')], env=env, check=True)
            subprocess.run(['sh', str(second / 'uninstall.sh')], env=env, check=True, capture_output=True)
            self.assertFalse(install.exists())
            self.assertFalse(desktop.exists())
            self.assertEqual(profile.read_bytes(), b'keep-progress')

    def test_unknown_installation_is_never_overwritten_or_deleted(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, env = self.fixture(Path(tmp))
            target = Path(env['XDG_DATA_HOME']) / 'ClavunoraRust'
            target.mkdir(parents=True)
            sentinel = target / 'unrelated.txt'
            sentinel.write_text('preserve')
            for script in ('install.sh', 'uninstall.sh'):
                result = subprocess.run(['sh', str(source / script)], env=env, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(sentinel.read_text(), 'preserve')


if __name__ == '__main__': unittest.main()
