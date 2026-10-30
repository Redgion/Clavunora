# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Offline smoke check shared by source and frozen builds; uses no user data."""
import json
from pathlib import Path
import tempfile


def run():
    from PySide6.QtWidgets import QApplication
    from pianoflow.library.catalog import manifest, bundled_songs, install_catalog
    from pianoflow.library.store import Library
    from pianoflow.settings.store import SettingsStore
    from pianoflow.ui.window import MainWindow
    app = QApplication.instance() or QApplication([])
    from pianoflow.branding import APP_NAME, DATA_ID
    app.setApplicationName(DATA_ID)
    app.setApplicationDisplayName(APP_NAME)
    app.setOrganizationName(DATA_ID)
    with tempfile.TemporaryDirectory(prefix='pianoflow-check-') as directory:
        root = Path(directory)
        window = MainWindow(SettingsStore(root / 'settings.json'), start_midi=False)
        for identifier, song in bundled_songs():
            assert window.library.get(identifier) == song
        window.show(); app.processEvents()
        for theme in ('light','dark'):
            window.set_theme(theme)
            for index in range(8):
                window.navigate(index); app.processEvents()
                assert not window.grab().isNull()
        assert len(window.home_page.folders.tiles)==8
        assert window.achievements.list.count()==6
        assert len(window.course_page.cards)==6
        window.open_song('repertoire-041'); app.processEvents()
        count = len(window.library.songs())
        window.close(); app.processEvents()
        database = Library(root / 'library.sqlite3')
        install_catalog(database)
        assert len(database.songs()) == count
        database.close()
    print(json.dumps({'ok': True, 'library_items': count, 'planned': len(manifest()['entries'])}))
    return 0
