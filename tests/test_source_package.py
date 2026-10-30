# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import importlib.util
from pathlib import Path
import tempfile
import unittest
import tarfile

spec = importlib.util.spec_from_file_location('source_package', Path(__file__).resolve().parents[1] / 'scripts/package_rust_source.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class SourcePackageTests(unittest.TestCase):
    def test_tar_metadata_has_no_local_user_or_group(self):
        info = tarfile.TarInfo('source/main.rs')
        info.uid, info.gid = 1000, 1000
        info.uname, info.gname = 'local-user', 'local-group'
        info.pax_headers = {'SCHILY.xattr.user.comment': 'private metadata'}
        public = package.public_tar_metadata(info)
        self.assertEqual((public.uid, public.gid, public.uname, public.gname), (0, 0, '', ''))
        self.assertEqual(public.pax_headers, {})

    def test_allowlist_excludes_profiles_credentials_and_screenshots(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name in ('LICENSE', 'rust/src/lib.rs', '.aws/credentials', '.codex/settings.json', '.env', 'rust/profile.sqlite3', 'rust/profile.sqlite3-wal', 'rust/private.key', 'rust/target/secret', 'docs/screenshots/history.png'):
                p = root / name
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text('fixture')
            files = {str(p.relative_to(root)) for p in package.source_files(root)}
            self.assertEqual(files, {'LICENSE', 'rust/src/lib.rs'})

    def test_symlinks_in_allowed_sources_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'rust').mkdir()
            (root / 'secret').write_text('private')
            (root / 'rust/link').symlink_to(root / 'secret')
            with self.assertRaises(ValueError):
                package.source_files(root)
