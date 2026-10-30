# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from pathlib import Path
import tempfile
import unittest
from scripts.check_release_privacy import issues, check_files


class ReleasePrivacy(unittest.TestCase):
    def test_home_paths_and_windows_utf16_are_detected(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'program'
            for value in ('/' + 'home/tester/project/file.rs', '/' + 'Users/tester/project/file.rs', 'C:' + '\\Users\\tester\\project\\file.rs'):
                for encoding in ('utf-8', 'utf-16-le'):
                    path.write_bytes(value.encode(encoding))
                    self.assertIn('home path', issues(path))

    def test_attribution_urls_and_portable_paths_are_preserved(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'LICENSE'
            content = 'Copyright Example Author <author@example.org>\nhttps://crates.io/crates/home/0.5.12\n~/Clavunora/src/main.rs'
            path.write_text(content)
            self.assertEqual(issues(path), [])
            self.assertEqual(path.read_text(), content)

    def test_secrets_are_blocked_without_printing_their_values(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            path = root / 'fixture'
            token = 'ghp_' + 'x' * 36
            path.write_text(token)
            with self.assertRaises(ValueError) as error:
                check_files([path], root)
            self.assertNotIn(token, str(error.exception))
            private = root / 'profile.sqlite3'
            private.write_bytes(b'profile')
            self.assertIn('private file type', issues(private))
