# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
from pianoflow.settings.store import SettingsStore
from pianoflow.ui.window import MainWindow
from pianoflow.core.state import MidiEvent
from pianoflow.learning.achievements import achievement_state


def test_centered_folder_search_theme_and_course(app,tmp_path):
    path=tmp_path/'s.json';w=MainWindow(SettingsStore(path),start_midi=False);w.show();app.processEvents()
    assert w.sidebar.height()<100 and not w.statusBar().isVisible()
    QTest.mouseClick(w.home_page.folders.tiles[2],Qt.MouseButton.LeftButton)
    assert w.pages.currentIndex()==2 and w.library_page.current_folder()=='level:3'
    assert not w.library_page.folders.isVisible()
    w.library_page.back.click();assert w.library_page.current_folder()=='root' and w.library_page.folders.isVisible()
    w.navigate(0);w.home_page.search.setText('Найдите до');QTest.keyClick(w.home_page.search,Qt.Key.Key_Return)
    assert w.pages.currentIndex()==2 and w.library_page.list.count()==1
    w.nav[7].click();assert w.pages.currentIndex()==7
    w.theme_button.click();assert w.settings.values['theme']=='dark' and app.property('darkTheme')
    w.navigate(4);assert w.theme_combo.currentIndex()==1
    w.close();w=MainWindow(SettingsStore(path),start_midi=False)
    assert w.settings.values['theme']=='dark' and app.property('darkTheme')
    w.close()


def test_lesson_awards_real_input_and_defers_toast(app,tmp_path):
    w=MainWindow(SettingsStore(tmp_path/'s.json'),start_midi=False);w.show();w.open_song('course-5');l=w.lesson
    l.wait_box.setChecked(True);l.seek(0);l.toggle()
    for start,pitches in l.groups:
        l.transport.seek(start);l.advance()
        for pitch in pitches: w.on_midi(MidiEvent('note_on',pitch,90))
        for pitch in pitches: w.on_midi(MidiEvent('note_off',pitch))
    l.transport.seek(l.transport.duration);l.advance()
    report=w.library.attempts()[0]['report']
    assert report['full_song'] and report['both_hands'] and report['difficulty']==2
    values,_=achievement_state(w.library);assert sum(bool(v['earned']) for v in values)==4
    assert not w.toast.isVisible()
    w.navigate(6);assert w.toast.isVisible()
    assert '4 из 6' in w.achievements.summary.text()
    assert 'Обе руки' in w.home_page.latest.text() or 'Первые ноты' in w.home_page.latest.text()
    w.close()


def test_lesson_controls_do_not_overlap_and_dark_views(app,tmp_path):
    from PySide6.QtWidgets import QComboBox,QPushButton,QLabel
    w=MainWindow(SettingsStore(tmp_path/'s.json'),start_midi=False);w.resize(1440,900);w.show();w.open_song('course-5');w.set_theme('dark');app.processEvents()
    l=w.lesson
    assert l.play_button.isVisible() and l.hand_combo.width()>=100
    assert not l.play_button.geometry().intersects(l.speed.geometry())
    assert l.roll.grab().toImage().pixelColor(0,0).lightness()<100
    assert l.staff.grab().toImage().pixelColor(0,0).lightness()<100
    for index in range(8):
        w.navigate(index);app.processEvents();assert w.pages.currentWidget().width()<=w.width()
    w.close()
