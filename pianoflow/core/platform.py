# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Platform decisions kept outside the music/learning model."""
import os
from pathlib import Path
import sys
from pianoflow.branding import DATA_ID


def is_android():
    return sys.platform == 'android' or hasattr(sys, 'getandroidapilevel')


def settings_directory():
    if is_android():
        from PySide6.QtCore import QStandardPaths
        return Path(QStandardPaths.writableLocation(QStandardPaths.StandardLocation.AppDataLocation))
    if sys.platform == 'win32':
        return Path(os.environ.get('APPDATA', Path.home() / 'AppData' / 'Roaming')) / DATA_ID
    # Retain the established Linux location so upgrades keep existing results.
    return Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config')) / DATA_ID
