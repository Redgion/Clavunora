# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import time
import pytest
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
from pianoflow.core.song import Note, Song
from pianoflow.core.state import MidiEvent
from pianoflow.settings.store import SettingsStore
from pianoflow.ui.window import MainWindow
from pianoflow.ui.theme import STYLE
from pianoflow.audio.preview import ToneStream


def window_for(app, tmp_path):
    window = MainWindow(SettingsStore(tmp_path / "settings.json"), start_midi=False)
    window.show()
    window.open_birthday()
    app.processEvents()
    return window


def test_play_pause_speed_and_note_alignment(app, tmp_path):
    window = window_for(app, tmp_path)
    lesson = window.lesson
    assert lesson.keyboard.first <= 60 and lesson.keyboard.last >= 72
    lesson.toggle()
    assert lesson.running and lesson.frame_timer.isActive()
    lesson.seek(2)
    lesson.speed.setCurrentIndex(1)
    assert lesson.transport.speed == .5
    lesson.pause()
    assert not lesson.running and not lesson.frame_timer.isActive()
    from pianoflow.ui.keyboard import key_geometry
    keys = key_geometry(lesson.keyboard.first, lesson.keyboard.last, lesson.keyboard.width(), lesson.keyboard.height())
    rect = lesson.roll.note_rect(lesson.song.notes[0], keys)
    assert rect.center().x() == pytest.approx(keys[60].center().x())
    window.navigate(0)
    assert not lesson.running
    window.close()


def test_wait_requires_fresh_repeated_notes_and_whole_chord(app, tmp_path):
    window = window_for(app, tmp_path)
    lesson = window.lesson
    lesson.set_song(Song("test", (Note(60, 0, .3), Note(60, .5, .3), Note(64, .5, .3))))
    lesson.wait_box.setChecked(True)
    lesson.seek(0)
    lesson.toggle()
    lesson.advance()
    assert lesson.waiting
    QTest.keyPress(window, Qt.Key.Key_D)  # Wrong note does not advance.
    assert lesson.waiting and lesson.completed == 0
    QTest.keyRelease(window, Qt.Key.Key_D)
    QTest.keyPress(window, Qt.Key.Key_A)
    assert not lesson.waiting and lesson.completed == 1
    lesson.transport.seek(.5)
    lesson.advance()
    assert lesson.waiting  # The previous held C must not satisfy the next chord.
    QTest.keyPress(window, Qt.Key.Key_D)
    assert lesson.waiting
    QTest.keyRelease(window, Qt.Key.Key_A)
    QTest.keyPress(window, Qt.Key.Key_A)
    assert not lesson.waiting and lesson.completed == 2
    lesson.restart()
    assert lesson.transport.current() == -3
    assert lesson.completed == 0
    window.close()


def test_import_failure_keeps_song_and_worker_closes(app, tmp_path):
    window = window_for(app, tmp_path)
    lesson = window.lesson
    path = tmp_path / "broken.mid"
    path.write_bytes(b"broken")
    lesson.import_path(path)
    deadline = time.monotonic() + 3
    while not lesson.import_button.isEnabled() and time.monotonic() < deadline:
        app.processEvents()
        time.sleep(.01)
    assert "Ошибка импорта" in lesson.message.text()
    assert lesson.song.title == "С днём рождения"
    window.close()
    assert not lesson.importer.isRunning()


def test_lesson_render_two_resolutions(app, tmp_path):
    app.setStyleSheet(STYLE)
    window = window_for(app, tmp_path)
    for width, height in ((1000, 700), (1920, 1080)):
        window.resize(width, height)
        app.processEvents()
        window.lesson.seek(1.0)
        image = window.grab().toImage()
        assert image.width() == width and image.height() == height
        assert window.lesson.roll.width() == window.lesson.keyboard.width()
        assert window.lesson.roll.height() >= 160
    window.close()


def test_audio_pcm_without_device(app):
    stream = ToneStream()
    stream.set_notes({60, 64, 67})
    data = stream.readData(4096)
    assert len(data) == 4096
    assert data != bytes(4096)
    stream.set_notes(set())
    for _ in range(5):
        stream.readData(4096)
    assert not stream.voices
    stream.close()


def test_complete_birthday_in_wait_mode(app, tmp_path):
    window = window_for(app, tmp_path)
    lesson = window.lesson
    lesson.wait_box.setChecked(True)
    lesson.toggle()
    for note in lesson.song.notes:
        lesson.transport.seek(note.start)
        lesson.advance()
        assert lesson.waiting
        window.on_midi(MidiEvent("note_on", note.pitch, 100))
        assert not lesson.waiting
        window.on_midi(MidiEvent("note_off", note.pitch))
    assert lesson.completed == 25
    lesson.transport.seek(lesson.transport.duration)
    lesson.advance()
    assert not lesson.running
    assert "закончилась" in lesson.message.text()
    window.close()


def test_import_success(app, tmp_path):
    from pathlib import Path
    window = window_for(app, tmp_path)
    lesson = window.lesson
    lesson.import_path(Path(__file__).resolve().parents[1] / "assets" / "happy_birthday.mid")
    deadline = time.monotonic() + 3
    while not lesson.import_button.isEnabled() and time.monotonic() < deadline:
        app.processEvents()
        time.sleep(.01)
    assert lesson.song.title == "happy_birthday"
    assert len(lesson.song.notes) == 25
    assert lesson.song.bpm == 96
    assert lesson.song.time_signature == (3, 4)
    window.close()
