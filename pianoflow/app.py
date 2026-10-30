# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Application entry point; Qt selects Wayland/X11 from the desktop session."""
import sys
from importlib.resources import files
from PySide6.QtGui import QIcon
from PySide6.QtWidgets import QApplication, QMessageBox
from pianoflow.branding import APP_NAME, DATA_ID
from pianoflow.ui.theme import STYLE
from pianoflow.core.platform import is_android
from pianoflow.ui.window import MainWindow


def main():
    if '--self-test' in sys.argv:
        from pianoflow.selftest import run
        return run()
    app = QApplication(sys.argv)
    app.setApplicationName(DATA_ID)
    app.setApplicationDisplayName(APP_NAME)
    app.setOrganizationName(DATA_ID)
    app.setWindowIcon(QIcon(str(files("pianoflow").joinpath("resources/icons/pianoflow.png"))))
    app.setStyle("Fusion")
    app.setStyleSheet(STYLE + ("QPushButton, QComboBox { min-height: 28px; } QLabel#title { font-size: 23px; } QCheckBox::indicator { width: 24px; height: 24px; }" if is_android() else ""))
    try:
        window = MainWindow()
    except Exception as exc:
        QMessageBox.critical(None, f"{APP_NAME}: запуск не выполнен", str(exc))
        return 1
    window.show()
    return app.exec()
