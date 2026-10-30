# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from PySide6.QtCore import QPointF, Qt
from PySide6.QtTest import QTest
from pianoflow.core.state import MidiEvent
from pianoflow.settings.store import SettingsStore
from pianoflow.ui.keyboard import key_geometry
from pianoflow.ui.window import MainWindow
from pianoflow.ui.theme import STYLE


def test_keyboard_geometry():
    keys = key_geometry(21, 108, 1040, 200)
    assert len(keys) == 88
    assert keys[21].left() == 0
    assert keys[108].right() == 1040
    assert keys[61].height() < keys[60].height()
    assert keys[60].center().x() < keys[61].center().x() < keys[62].center().x()


def test_window_input_navigation_settings(app, tmp_path):
    window = MainWindow(SettingsStore(tmp_path / "settings.json"), start_midi=False)
    window.show()
    window.navigate(1)
    app.processEvents()
    QTest.keyPress(window, Qt.Key.Key_A)
    QTest.keyPress(window, Qt.Key.Key_D)
    QTest.keyPress(window, Qt.Key.Key_G)
    assert window.state.pressed == {60, 64, 67}
    assert window.chord.text() == "C"
    QTest.keyPress(window, Qt.Key.Key_Space)
    QTest.keyRelease(window, Qt.Key.Key_A)
    assert 60 in window.state.sounding and 60 not in window.state.pressed
    QTest.keyRelease(window, Qt.Key.Key_Space)
    assert 60 not in window.state.sounding
    window.navigate(4)
    assert not window.state.sounding
    window.size_combo.setCurrentIndex(1)
    window.labels.setChecked(False)
    assert window.keyboard.first == 36 and window.keyboard.last == 96
    window.close()
    assert SettingsStore(tmp_path / "settings.json").values["show_labels"] is False


def test_mouse_black_key_and_midi_disconnect(app, tmp_path):
    window = MainWindow(SettingsStore(tmp_path / "s.json"), start_midi=False)
    window.show()
    window.navigate(1)
    app.processEvents()
    keyboard = window.keyboard
    keys = key_geometry(21, 108, keyboard.width(), keyboard.height() - 1)
    pos = keys[61].center().toPoint()
    assert keyboard.hit(QPointF(pos)) == 61
    QTest.mousePress(keyboard, Qt.MouseButton.LeftButton, pos=pos)
    assert window.state.pressed == {61}
    window.on_midi(MidiEvent("note_on", 67, 100))
    window.on_disconnect()
    assert window.state.pressed == {61}
    QTest.mouseRelease(keyboard, Qt.MouseButton.LeftButton, pos=pos)
    assert not window.state.pressed
    window.close()


def test_render_all_pages(app, tmp_path):
    app.setStyleSheet(STYLE)
    window = MainWindow(SettingsStore(tmp_path / "s.json"), start_midi=False)
    window.show()
    for width, height in ((1000, 700), (1920, 1080)):
        window.resize(width, height)
        for i in range(5):
            window.navigate(i)
            app.processEvents()
            image = window.grab().toImage()
            assert image.width() == width and image.height() == height
    window.close()
