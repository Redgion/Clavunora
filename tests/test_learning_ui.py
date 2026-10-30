# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from PySide6.QtCore import Qt
from pianoflow.ui.window import MainWindow
from pianoflow.settings.store import SettingsStore
from pianoflow.core.song import Note, Song
from pianoflow.core.state import MidiEvent


def make(app,tmp_path):
    w=MainWindow(SettingsStore(tmp_path/'settings.json'),start_midi=False); w.show(); app.processEvents(); return w


def test_library_course_progress_persistence(app,tmp_path):
    from pianoflow.library.catalog import bundled_songs
    w=make(app,tmp_path); assert len(w.library.songs())==9+len(list(bundled_songs()))
    w.open_song('course-1'); lesson=w.lesson
    lesson.wait_box.setChecked(True); lesson.toggle()
    for n in lesson.song.notes:
        lesson.transport.seek(n.start); lesson.advance()
        w.on_midi(MidiEvent('note_on',n.pitch,90)); w.on_midi(MidiEvent('note_off',n.pitch))
    lesson.transport.seek(lesson.transport.duration); lesson.advance()
    assert w.library.preference('passed:course-1')
    assert w.library.attempts()[0]['report']['complete']
    w.close()
    w=make(app,tmp_path); assert w.library.preference('passed:course-1'); w.close()


def test_loop_does_not_wait_for_notes_before_fragment(app,tmp_path):
    w=make(app,tmp_path); w.open_song('course-2'); l=w.lesson
    l.loop_start=1.5; l.loop_end=3.; l.loop_enabled=True
    l.wait_box.setChecked(True); l.seek(.5); l.toggle(); l.transport.seek(.75); l.advance()
    assert not l.waiting
    l.transport.seek(1.5); l.advance(); assert l.waiting
    w.close()


def test_midi_commands_not_recorded_or_scored(app,tmp_path):
    w=make(app,tmp_path); w.open_song('course-1'); l=w.lesson
    l.control_enabled=True; w.recorder.start()
    w.on_midi(MidiEvent('note_on',21,90)); assert l.running
    w.on_midi(MidiEvent('note_on',21,90)); assert l.running
    w.on_midi(MidiEvent('note_off',21)); assert not w.state.pressed and not w.recorder.events
    w.on_midi(MidiEvent('note_on',21,90)); assert not l.running
    w.on_midi(MidiEvent('note_off',21)); w.close()


def test_hints_hand_filter_and_recording(app,tmp_path):
    w=make(app,tmp_path); w.open_song('course-5'); l=w.lesson
    l.hand_combo.setCurrentIndex(1); assert all(n.hand=='R' for n in l.practice_notes)
    l.seek(0); l.hint_combo.setCurrentIndex(2); assert not l.keyboard.expected and not l.keyboard.fingers
    w.toggle_recording(); w.on_midi(MidiEvent('note_on',60,90)); w.on_midi(MidiEvent('note_off',60)); w.toggle_recording()
    assert len(w.library.recordings())==1
    w.close()


def test_smart_loop_increases_speed_and_saves_once(app,tmp_path):
    w=make(app,tmp_path); w.open_song('course-1'); l=w.lesson
    l.loop_enabled=True; l.loop_start=0; l.loop_end=.7; l.smart=True; l.loop_repetitions=1
    l.set_speed_value(.5); l.wait_box.setChecked(True); l.seek(0); l.toggle(); l.advance()
    w.on_midi(MidiEvent('note_on',60,90)); w.on_midi(MidiEvent('note_off',60))
    l.transport.seek(.7); l.advance()
    assert abs(l.transport.speed-.55)<.0001
    assert len(w.library.attempts())==1
    assert l.running and l.transport.current()<0
    w.close()


def test_pause_and_mode_change_save_correct_mode(app,tmp_path):
    w=make(app,tmp_path); w.open_song('course-1'); l=w.lesson
    l.wait_box.setChecked(True); l.seek(0); l.toggle(); l.advance()
    w.on_midi(MidiEvent('note_on',60,90)); w.on_midi(MidiEvent('note_off',60))
    l.wait_box.setChecked(False)
    report=w.library.attempts()[0]['report']
    assert report['mode']=='Ожидание' and not report['complete']
    w.close()


def test_practice_dialog_scroll_and_accept(app,tmp_path):
    from PySide6.QtCore import QTimer
    from PySide6.QtWidgets import QApplication,QDialogButtonBox,QScrollArea
    w=make(app,tmp_path); w.open_song('course-1')
    def accept():
        d=QApplication.activeModalWidget()
        assert d.findChild(QScrollArea)
        d.findChild(QDialogButtonBox).button(QDialogButtonBox.StandardButton.Ok).click()
    QTimer.singleShot(20,accept)
    w.lesson.practice_options()
    assert w.library.preference('practice')['tolerance']==.15
    w.close()


def test_library_folders_navigation_search_and_restore(app,tmp_path):
    w=make(app,tmp_path); page=w.library_page
    page.select_folder('level:3')
    assert page.list.count() > 0
    for i in range(page.list.count()):
        identifier=page.list.item(i).data(Qt.ItemDataRole.UserRole)
        assert w.library.get(identifier).difficulty==3
    identifier=page.selected()
    w.library.favorite(identifier,True)
    page.select_folder('favorites'); assert page.list.count()==1
    page.search.setText('Нет такой песни'); assert page.list.count()==0
    page.select_folder('course'); assert page.list.count()==6
    page.select_folder('favorites'); w.close()
    w=make(app,tmp_path)
    assert w.library_page.current_folder()=='favorites'
    assert w.library_page.selected()==identifier
    w.open_course(); assert w.library_page.current_folder()=='course'
    w.close()


def test_phone_layout_keeps_piano_and_navigation_accessible(app, tmp_path):
    from pianoflow.ui.window import MainWindow
    from pianoflow.settings.store import SettingsStore
    window = MainWindow(SettingsStore(tmp_path/'phone.json'), start_midi=False)
    assert window.pages.currentIndex() == 0
    window.resize(390, 844); window.show(); app.processEvents()
    assert window.width() == 390
    assert window.mobile_menu.isVisible() and not window.sidebar.isVisible()
    window.open_birthday(); app.processEvents()
    assert window.lesson.keyboard.width() <= 390
    assert window.lesson.keyboard.last - window.lesson.keyboard.first == 12
    assert window.lesson.header_scroll.height() <= 200
    before = window.lesson.keyboard.first
    window.lesson.shift_octave(1)
    assert window.lesson.keyboard.first == before + 12
    for width, height in ((800,1280), (844,390)):
        window.resize(width,height); app.processEvents()
        assert window.width() == width and window.height() == height
        keyboard = window.lesson.keyboard
        assert keyboard.geometry().bottom() <= window.lesson.height()
    window.resize(1200, 900); app.processEvents()
    assert window.sidebar.isVisible() and not window.mobile_menu.isVisible()
    assert not window.lesson.octave_bar.isVisible()
    window.close()


def test_rebrand_reuses_existing_library_and_progress(app, tmp_path, monkeypatch):
    from pianoflow.core.platform import settings_directory
    from pianoflow.library.store import Library
    from pianoflow.core.song import birthday_song
    from pianoflow.branding import APP_NAME, DATA_ID
    monkeypatch.setenv('XDG_CONFIG_HOME', str(tmp_path))
    monkeypatch.setenv('APPDATA', str(tmp_path))
    legacy = tmp_path/'PianoFlow'
    legacy.mkdir()
    assert settings_directory() == legacy
    old = Library(legacy/'library.sqlite3')
    identifier = old.add(birthday_song(), 'rename-kept')
    old.favorite(identifier, True)
    old.save_attempt(identifier, {'accuracy': 91})
    old.close()
    window = MainWindow(start_midi=False)
    assert APP_NAME == 'Clavunora' and DATA_ID == 'PianoFlow'
    assert window.windowTitle().startswith('Clavunora')
    assert window.library.get(identifier) is not None
    assert len(window.library.attempts()) == 1
    assert next(f for i,s,f in window.library.songs() if i == identifier)
    assert window.settings.path.parent == legacy
    window.close()
