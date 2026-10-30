# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import time
from pathlib import Path
from pianoflow.branding import APP_NAME
from pianoflow.core.files import local_document, write_document
import tempfile
from PySide6.QtCore import QEvent, Qt, QTimer, Slot
from PySide6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QFrame, QHBoxLayout, QVBoxLayout,
    QSizePolicy, QBoxLayout, QLabel, QPushButton, QStackedWidget, QComboBox, QCheckBox, QScrollArea,
    QMenu, QFileDialog, QMessageBox, QLineEdit, QSpinBox, QDoubleSpinBox, QDialog, QListWidget, QDialogButtonBox,
)
from pianoflow.core.platform import is_android
from pianoflow.core.notes import chord_name, note_name
from pianoflow.core.state import MidiEvent, PianoState
from pianoflow.midi.service import MidiService
from pianoflow.settings.store import SettingsStore
from pianoflow.ui.theme import AeroBackground, apply_theme
from pianoflow.ui.home import HomePage
from pianoflow.ui.achievements import AchievementsPage
from pianoflow.ui.course import CoursePage
from pianoflow.learning.achievements import achievement_state
from pianoflow.ui.keyboard import PianoKeyboard
from pianoflow.ui.lesson import LessonPage
from pianoflow.library.store import Library
from pianoflow.library.catalog import install_catalog
from pianoflow.learning.course import course_songs, birthday_variants
from pianoflow.learning.recording import Recorder
from pianoflow.ui.library import LibraryPage, StatisticsPage
from pianoflow.ui.metronome import Metronome

# Physical keys by Qt key code; one octave, including black keys.
KEYS = {ord(k): n for k, n in zip("AWSEDFTGYHUJK", range(60, 73))}


def label(text, name=None):
    widget = QLabel(text)
    if name:
        widget.setObjectName(name)
    widget.setWordWrap(True)
    return widget


def button(text, callback, name=None):
    widget = QPushButton(text)
    if name:
        widget.setObjectName(name)
    widget.clicked.connect(callback)
    return widget


def card():
    widget = QFrame()
    widget.setObjectName("card")
    layout = QVBoxLayout(widget)
    layout.setContentsMargins(24, 22, 24, 22)
    layout.setSpacing(12)
    return widget, layout


class MainWindow(QMainWindow):
    def __init__(self, settings=None, start_midi=True):
        super().__init__()
        self.settings = settings or SettingsStore()
        self.library = Library(self.settings.path.parent / "library.sqlite3")
        if not self.library.preference('seeded', False):
            for identifier, song in course_songs() + birthday_variants():
                self.library.add(song, identifier)
            self.library.set_preference('seeded', True)
        install_catalog(self.library)
        self.recorder = Recorder()
        self.state = PianoState()
        self.computer_held = set()
        self.connected = False
        self.ports = []
        self.auto_attempted = set()
        self.started = time.monotonic()
        self.setWindowTitle(f"{APP_NAME} · Свободная игра")
        self.resize(1440, 900)
        self.setMinimumSize(320, 320)
        root = AeroBackground()
        self.setCentralWidget(root)
        outer = QVBoxLayout(root)
        outer.setContentsMargins(0, 0, 0, 0)
        self.mobile_menu = QPushButton(f"☰  {APP_NAME} · Разделы")
        menu = QMenu(self.mobile_menu)
        for i, title in enumerate(("Главная", "Свободная игра", "Библиотека", "Прогресс", "Настройки", "Занятие", "Достижения", "Обучение")):
            menu.addAction(title, lambda checked=False, index=i: self.navigate(index))
        menu.addSeparator()
        menu.addAction('Начать / сохранить запись', self.toggle_recording)
        menu.addAction('Мои записи…', self.show_recordings)
        self.mobile_menu.setMenu(menu)
        self.mobile_menu.setMinimumHeight(48)
        outer.addWidget(self.mobile_menu)
        self.page_titles = ['Главная','Свободная игра','Библиотека','Прогресс','Настройки','Занятие','Достижения','Обучение']
        self.sidebar = QFrame()
        self.sidebar.setObjectName('topNav')
        side = QHBoxLayout(self.sidebar); side.setContentsMargins(18,8,18,8); side.setSpacing(5)
        self.pages = QStackedWidget()
        self.nav = []
        for index,title in enumerate(self.page_titles):
            nav=button(title,lambda checked=False,i=index:self.navigate(i),'nav'); nav.setCheckable(True); self.nav.append(nav)
        for index in (0,2): side.addWidget(self.nav[index])
        side.addWidget(self.nav[7])
        for index in (6,3,1): side.addWidget(self.nav[index])
        self.nav[5].setParent(self); self.nav[5].hide()
        side.addStretch()
        self.theme_button=button('☾  Тёмная',self.toggle_theme); self.theme_button.setAccessibleName('Переключить светлую и тёмную тему'); side.addWidget(self.theme_button)
        side.addWidget(self.nav[4])
        outer.addWidget(self.sidebar)
        self.connection_label=label('○  MIDI не подключён','muted')
        outer.addWidget(self.pages,1)
        self.toast=QFrame(); self.toast.setObjectName('toast'); toast_layout=QHBoxLayout(self.toast)
        self.toast_text=QLabel(); toast_layout.addWidget(self.toast_text); toast_layout.addStretch()
        toast_layout.addWidget(button('Достижения →',lambda:self.navigate(6)))
        outer.addWidget(self.toast); self.toast.hide()
        self.toast_timer=QTimer(self); self.toast_timer.setSingleShot(True); self.toast_timer.timeout.connect(self.toast.hide)
        self.make_home()
        self.make_freeplay()
        self.make_library()
        self.statistics = StatisticsPage(self.library)
        self.statistics.practice_error.connect(self.open_error)
        self.pages.addWidget(self.statistics)
        self.make_settings()
        self.lesson = LessonPage(self.state, store=self.library)
        self.pages.addWidget(self.lesson)
        self.achievements=AchievementsPage(); self.pages.addWidget(self.achievements)
        self.course_page=CoursePage(self.library); self.course_page.open_song.connect(self.open_song)
        course_scroll=QScrollArea(); course_scroll.setWidgetResizable(True); course_scroll.setWidget(self.course_page); self.pages.addWidget(course_scroll)
        self.lesson.attempt_saved.connect(self.on_attempt_saved)
        self.refresh_achievements()
        self.apply_saved_theme()
        self.record_button = QPushButton('● Записать MIDI')
        self.record_button.clicked.connect(self.toggle_recording)
        self.statusBar().addWidget(self.connection_label,1)
        self.statusBar().addPermanentWidget(self.record_button)
        recording_list = QPushButton('Мои записи…')
        recording_list.clicked.connect(self.show_recordings)
        self.statusBar().addPermanentWidget(recording_list)
        self.lesson.input_note.connect(lambda kind, note: self.simulate(kind, note, "mouse"))
        self.lesson.keyboard.show_labels = self.settings.values["show_labels"]
        self.lesson.set_keyboard_size(self.settings.values["keyboard_size"])
        self.midi = MidiService(self)
        self.midi.event_received.connect(self.on_midi, Qt.ConnectionType.QueuedConnection)
        self.midi.ports_changed.connect(self.on_ports, Qt.ConnectionType.QueuedConnection)
        self.midi.status_changed.connect(self.on_status, Qt.ConnectionType.QueuedConnection)
        self.midi.disconnected.connect(self.on_disconnect, Qt.ConnectionType.QueuedConnection)
        QApplication.instance().installEventFilter(self)
        self.timer = QTimer(self)
        self.timer.timeout.connect(self.tick)
        self.timer.start(1000)
        self.navigate(0)
        self.refresh_view()
        if self.settings.error:
            self.statusBar().showMessage(self.settings.error)
        # Long content remains reachable on phones; lesson keeps its piano visible.
        for index in range(5):
            page = self.pages.widget(index)
            self.pages.removeWidget(page)
            scroll = QScrollArea()
            scroll.setWidgetResizable(True)
            scroll.setFrameShape(QFrame.Shape.NoFrame)
            scroll.setWidget(page)
            self.pages.insertWidget(index, scroll)
        QApplication.instance().applicationStateChanged.connect(self.application_state_changed)
        self.update_responsive()
        self.navigate(0)
        if start_midi:
            self.midi.start()

    def application_state_changed(self, state):
        if state != Qt.ApplicationState.ApplicationActive:
            self.lesson.pause()
            self.free_metronome.stop()
            self.panic()
            if is_android() and self.recorder.recording:
                self.stop_recording()

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if hasattr(self, 'lesson'):
            self.update_responsive()

    def update_responsive(self):
        compact = self.width() < 900
        self.sidebar.setVisible(not compact)
        self.mobile_menu.setVisible(compact)
        self.statusBar().setVisible(not compact and self.pages.currentIndex() in (1,5))
        if compact != getattr(self, '_compact', None):
            self._compact = compact
            for scroll in [self.pages.widget(i) for i in range(5)]:
                page = scroll.widget() if isinstance(scroll, QScrollArea) else scroll
                if page.layout(): page.layout().setContentsMargins(*((12, 12, 12, 12) if compact else (30, 25, 30, 25)))
            self.lesson.layout().setContentsMargins(*((8, 8, 8, 8) if compact else (34, 22, 34, 22)))
        self.home_page.bottom.setDirection(QBoxLayout.Direction.TopToBottom if compact else QBoxLayout.Direction.LeftToRight)
        for label_widget in self.findChildren(QLabel):
            if not label_widget.pixmap().isNull(): continue
            label_widget.setWordWrap(True)
            label_widget.setMinimumWidth(0)
            label_widget.setSizePolicy(QSizePolicy.Policy.Ignored if compact else QSizePolicy.Policy.Preferred, QSizePolicy.Policy.Preferred)
        for scroll in [self.pages.widget(i) for i in range(5)]:
            if isinstance(scroll, QScrollArea):
                scroll.widget().setMinimumWidth(0)
                if compact: scroll.widget().setMaximumWidth(max(280, self.width()-16))
                else: scroll.widget().setMaximumWidth(16777215)
        self.setStyleSheet('QLabel#title { font-size: 23px; } QPushButton, QComboBox { min-height: 26px; }' if compact else '')
        short = self.height() < 600
        self.lesson.header_scroll.setMaximumHeight(80 if short else 200 if compact else 240)
        self.lesson.header_scroll.setMinimumHeight(60 if short else 180 if compact else 220)
        self.lesson.layout().setSpacing(4 if short else 10)
        self.lesson.roll.setMinimumHeight(60 if short else 100 if compact else 160)
        self.lesson.visuals.setMinimumHeight(60 if short else 100 if compact else 180)
        self.lesson.keyboard.setMinimumHeight(65 if short else 100 if compact else 120)
        self.lesson.desktop_hint.setVisible(not compact)
        self.lesson.octave_bar.setVisible(self.width() < 600 and not short)
        self.lesson.update_range()

    def page(self, title, subtitle):
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setContentsMargins(34, 30, 34, 26)
        layout.setSpacing(20)
        layout.addWidget(label(title, "title"))
        layout.addWidget(label(subtitle, "muted"))
        self.pages.addWidget(page)
        return layout

    def make_home(self):
        self.home_page=HomePage(self.library)
        self.home_page.folder_selected.connect(self.open_folder)
        self.home_page.search_requested.connect(self.search_library)
        self.home_page.continue_requested.connect(self.open_song)
        self.home_page.achievements_requested.connect(lambda:self.navigate(6))
        self.pages.addWidget(self.home_page)

    def open_folder(self,key):
        self.library_page.select_folder(key); self.navigate(2)

    def search_library(self,text):
        self.library_page.select_folder('all'); self.library_page.search.setText(text); self.navigate(2)

    def apply_saved_theme(self):
        dark=self.settings.values['theme']=='dark'
        apply_theme(dark)
        self.theme_button.setText('☀  Светлая' if dark else '☾  Тёмная')
        if hasattr(self,'theme_combo'):
            self.theme_combo.blockSignals(True); self.theme_combo.setCurrentIndex(int(dark)); self.theme_combo.blockSignals(False)

    def toggle_theme(self):
        self.set_theme('light' if self.settings.values['theme']=='dark' else 'dark')

    def set_theme(self,value):
        self.settings.values['theme']=value; self.apply_saved_theme(); self.save_settings()

    def refresh_achievements(self,notify=False):
        states,new=achievement_state(self.library)
        self.achievements.refresh(states); self.home_page.refresh(states)
        if notify and new:
            self.toast_text.setText('Новая награда: '+', '.join(a.title for a in new))
            # No moving overlay over the notes: announce after leaving the lesson.
            self.pending_achievements=True
            if self.pages.currentIndex()!=5: self.toast.show(); self.toast_timer.start(7000); self.pending_achievements=False

    def on_attempt_saved(self):
        self.refresh_achievements(notify=True)

    def make_freeplay(self):
        layout = self.page("Свободная игра", "Слушайте свой инструмент, исследуйте гармонию и не бойтесь ошибаться.")
        toolbar = QHBoxLayout()
        toolbar.addWidget(label("ЖИВОЙ ВВОД", "eyebrow"))
        toolbar.addStretch()
        toolbar.addWidget(button("Подключить MIDI", lambda: self.navigate(4)))
        toolbar.addWidget(button("Сбросить ноты", self.panic))
        layout.addLayout(toolbar)
        self.free_metronome = Metronome()
        layout.addWidget(self.free_metronome)
        panel, inner = card()
        inner.addWidget(label("НОТА / АККОРД", "eyebrow"))
        self.chord = label("Сыграйте ноту", "chord")
        inner.addWidget(self.chord)
        self.notes_label = label("Клавиатура готова", "muted")
        inner.addWidget(self.notes_label)
        inner.addStretch()
        metrics = QHBoxLayout()
        self.velocity = label("Динамика  —", "muted")
        self.pedal = label("SUSTAIN  ○", "muted")
        self.session = label("00:00  ·  0 нот", "muted")
        for metric in (self.velocity, self.pedal, self.session):
            metrics.addWidget(metric)
        inner.addLayout(metrics)
        layout.addWidget(panel, 1)
        layout.addWidget(label("A W S E D F T G Y H U J K  ·  C4–C5     /     Пробел — педаль Sustain", "muted"))
        self.keyboard = PianoKeyboard()
        self.keyboard.set_size(self.settings.values["keyboard_size"])
        self.keyboard.show_labels = self.settings.values["show_labels"]
        self.keyboard.note_pressed.connect(lambda n: self.simulate("note_on", n, "mouse"))
        self.keyboard.note_released.connect(lambda n: self.simulate("note_off", n, "mouse"))
        layout.addWidget(self.keyboard)
        layout.addWidget(label("Звук живой игры идёт от инструмента. Для программного тембра откройте урок и включите «Звук».", "muted"))

    def open_course(self):
        self.library_page.select_folder('course')
        self.navigate(7)

    def open_birthday(self):
        if self.library.get('birthday-melody'):
            self.open_song('birthday-melody')
        else:
            from pianoflow.core.song import birthday_song
            self.lesson.set_song(birthday_song())
            self.navigate(5)

    def open_song(self, identifier):
        song = self.library.get(identifier)
        if not song:
            self.statusBar().showMessage('Песня удалена из библиотеки.')
            return
        self.library.set_preference('last-song',identifier)
        self.lesson.set_song(song)
        self.lesson.song_id = identifier
        options = self.library.preference('practice', {})
        for key in ('tolerance','latency','metronome','accompany'):
            if key in options: setattr(self.lesson,key,options[key])
        loop = self.library.preference('loop:'+identifier,{})
        if loop and 0 <= loop['start'] < loop['end'] <= song.duration:
            self.lesson.loop_start=loop['start']; self.lesson.loop_end=loop['end']; self.lesson.loop_enabled=loop['enabled']
        self.lesson.message.setText(song.description or 'Нажмите «Начать». Настройки повторов и ритма — в «Практика…».')
        self.navigate(5)

    def open_error(self, identifier, position):
        if not self.library.get(identifier):
            QMessageBox.information(self,'Песня недоступна','Эта песня удалена из библиотеки.'); return
        self.open_song(identifier)
        self.lesson.loop_start=max(0,position-1)
        self.lesson.loop_end=min(self.lesson.song.duration,position+3)
        self.lesson.loop_enabled=True
        self.lesson.seek(self.lesson.loop_start-1)
        self.lesson.message.setText('Фрагмент с ошибкой готов к повторению. Нажмите «Продолжить».')

    def make_library(self):
        self.library_page = LibraryPage(self.library)
        self.library_page.open_song.connect(self.open_song)
        self.pages.addWidget(self.library_page)

    def open_midi(self):
        self.navigate(5)
        self.lesson.choose_midi()

    def make_future(self, title, heading, text):
        layout = self.page(title, "Возможности следующих этапов")
        panel, content = card()
        content.addWidget(label(heading, "title"))
        content.addWidget(label(text, "muted"))
        content.addWidget(button("Перейти к свободной игре →", lambda: self.navigate(1), "primary"))
        layout.addWidget(panel)
        layout.addStretch()

    def make_settings(self):
        layout = self.page("Настройки", "Устройство и клавиатура · настройки сохраняются локально")
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        body = QWidget()
        content = QVBoxLayout(body)
        content.setContentsMargins(0, 0, 0, 0)
        panel, inner = card()
        inner.addWidget(label('ОФОРМЛЕНИЕ','eyebrow'))
        self.theme_combo=QComboBox(); self.theme_combo.addItems(['Светлая тема','Тёмная тема']); self.theme_combo.setCurrentIndex(int(self.settings.values['theme']=='dark'))
        self.theme_combo.currentIndexChanged.connect(lambda i:self.set_theme('dark' if i else 'light')); inner.addWidget(self.theme_combo)
        inner.addWidget(label('Выбор сохраняется и применяется ко всем разделам приложения.','muted')); content.addWidget(panel)
        panel, inner = card()
        inner.addWidget(label("MIDI-УСТРОЙСТВО", "eyebrow"))
        self.port_combo = QComboBox()
        self.port_combo.setSizeAdjustPolicy(QComboBox.SizeAdjustPolicy.AdjustToMinimumContentsLengthWithIcon)
        self.port_combo.setMinimumContentsLength(20)
        inner.addWidget(self.port_combo)
        row = QHBoxLayout()
        row.addWidget(button("Подключить", self.connect_selected, "primary"))
        row.addWidget(button("Обновить", lambda: self.midi.refresh()))
        row.addWidget(button("Отключить", self.manual_disconnect))
        inner.addLayout(row)
        self.auto = QCheckBox("Подключаться автоматически к сохранённому устройству")
        self.auto.setChecked(self.settings.values["auto_connect"])
        self.auto.toggled.connect(self.change_settings)
        inner.addWidget(self.auto)
        self.midi_detail = label("Поиск MIDI-портов…", "muted")
        inner.addWidget(self.midi_detail)
        inner.addWidget(label("Bluetooth MIDI работает через MIDI-транспорт ОС. Обычного Bluetooth-соединения недостаточно: если MIDI-порт не появился, проверьте поддержку BLE-MIDI инструментом и системой.", "muted"))
        content.addWidget(panel)
        panel, inner = card()
        inner.addWidget(label("КЛАВИАТУРА", "eyebrow"))
        self.size_combo = QComboBox()
        for size in (88, 61, 49):
            self.size_combo.addItem(f"{size} клавиш", size)
        self.size_combo.setCurrentIndex((88, 61, 49).index(self.settings.values["keyboard_size"]))
        self.size_combo.currentIndexChanged.connect(self.change_settings)
        inner.addWidget(self.size_combo)
        self.labels = QCheckBox("Показывать названия нот")
        self.labels.setChecked(self.settings.values["show_labels"])
        self.labels.toggled.connect(self.change_settings)
        inner.addWidget(self.labels)
        inner.addWidget(label("88 клавиш: A0–C8 · 61 клавиша: C2–C7 · 49 клавиш: C2–C6", "muted"))
        content.addWidget(panel)
        panel, inner = card()
        inner.addWidget(label('РЕЗЕРВНАЯ КОПИЯ', 'eyebrow'))
        inner.addWidget(button('Сохранить библиотеку и прогресс…', self.backup))
        inner.addWidget(button('Восстановить из копии…', self.restore))
        inner.addWidget(label('Включает песни, разметку, результаты, записи и учебные настройки. При восстановлении можно сохранить текущие записи или заменить совпадающие из копии.', 'muted'))
        content.addWidget(panel)
        content.addStretch()
        scroll.setWidget(body)
        layout.addWidget(scroll)

    def navigate(self, index):
        previous = self.pages.currentIndex()
        if previous == 1 and index != 1: self.free_metronome.stop()
        if previous in (1, 5) and previous != index:
            self.release_computer()
            self.keyboard.release_mouse()
            if hasattr(self, "lesson"):
                self.lesson.keyboard.release_mouse()
                if previous == 5:
                    self.lesson.pause()
                    if self.lesson.audio:
                        self.lesson.audio.set_notes(set())
        if index == 2: self.library_page.refresh()
        if index == 3: self.statistics.refresh()
        if index in (0,6): self.refresh_achievements()
        if index==7: self.course_page.refresh()
        if index!=5 and getattr(self,'pending_achievements',False):
            self.toast.show(); self.toast_timer.start(7000); self.pending_achievements=False
        if index==5: self.toast.hide()
        self.statusBar().setVisible(self.width()>=900 and index in (1,5))
        self.pages.setCurrentIndex(index)
        self.setWindowTitle(f"{APP_NAME} · " + (self.lesson.song.title if index == 5 else self.page_titles[index]))
        for i, nav in enumerate(self.nav):
            nav.setChecked(i == (2 if index == 5 else index))

    def simulate(self, kind, note, source):
        self.on_midi(MidiEvent(kind, note=note, velocity=90 if kind == "note_on" else 0, source=source))

    @Slot(object)
    def on_midi(self, event):
        if hasattr(self, 'lesson') and self.pages.currentIndex() == 5 and self.lesson.handle_command(event):
            return
        self.recorder.feed(event)
        self.state.apply(event)
        if hasattr(self, "lesson") and self.pages.currentIndex() == 5:
            self.lesson.note_input(event)
        self.refresh_view()

    def refresh_view(self):
        self.keyboard.pressed = self.state.pressed
        self.keyboard.sounding = self.state.sounding
        self.keyboard.update()
        self.chord.setText(chord_name(self.state.sounding))
        self.notes_label.setText(" · ".join(note_name(n) for n in sorted(self.state.sounding)) or "Клавиатура готова · сыграйте первую ноту")
        self.velocity.setText(f"Динамика  {self.state.last_velocity} / 127")
        self.pedal.setText("SUSTAIN  ●" if self.state.pedals else "SUSTAIN  ○")
        if hasattr(self, "lesson"):
            self.lesson.refresh_input()
        self.tick()

    def tick(self):
        elapsed = int(time.monotonic() - self.started)
        self.session.setText(f"{elapsed // 60:02}:{elapsed % 60:02}  ·  {self.state.note_count} нот за сеанс")

    @Slot(list)
    def on_ports(self, names):
        self.ports = names
        selected = self.port_combo.currentText()
        self.port_combo.clear()
        self.port_combo.addItems(names)
        saved = self.settings.values["midi_port"]
        if selected in names or saved in names:
            self.port_combo.setCurrentText(selected if selected in names else saved)
        self.midi_detail.setText(f"Доступно портов: {len(names)}" if names else "MIDI-порты не найдены. Подключите инструмент по USB; ввод с экрана доступен всегда.")
        self.auto_attempted.intersection_update(names)
        if self.auto.isChecked() and not self.connected and saved in names and saved not in self.auto_attempted:
            self.auto_attempted.add(saved)
            self.midi.connect_port(saved)

    @Slot(str, bool)
    def on_status(self, text, connected):
        self.connected = connected
        self.connection_label.setText(("●  " if connected else "○  ") + text)
        self.midi_detail.setText(text)
        self.statusBar().showMessage(text)

    @Slot()
    def on_disconnect(self):
        if self.recorder.recording: self.stop_recording()
        if hasattr(self, 'lesson'):
            self.lesson.pause()
            self.lesson.command_held.clear()
        self.connected = False
        self.state.clear_source("midi")
        self.refresh_view()

    def connect_selected(self):
        if self.port_combo.count():
            name = self.port_combo.currentText()
            self.settings.values["midi_port"] = name
            self.save_settings()
            self.midi.connect_port(name)
        else:
            self.midi_detail.setText("Нет доступного MIDI-порта. Подключите инструмент и нажмите «Обновить».")

    def manual_disconnect(self):
        self.auto_attempted.update(self.ports)
        self.midi.disconnect_port()

    def change_settings(self, *_):
        self.settings.values.update(show_labels=self.labels.isChecked() if hasattr(self, "labels") else self.settings.values["show_labels"],
                                    keyboard_size=self.size_combo.currentData() if hasattr(self, "size_combo") else self.settings.values["keyboard_size"],
                                    auto_connect=self.auto.isChecked())
        self.keyboard.show_labels = self.settings.values["show_labels"]
        self.keyboard.set_size(self.settings.values["keyboard_size"])
        if hasattr(self, "lesson"):
            self.lesson.keyboard.show_labels = self.settings.values["show_labels"]
            self.lesson.set_keyboard_size(self.settings.values["keyboard_size"])
        self.save_settings()

    def save_settings(self):
        try:
            self.settings.save()
        except OSError as exc:
            self.statusBar().showMessage(f"Не удалось сохранить настройки: {exc}")

    def release_computer(self):
        for source,channel,note in list(self.state.held):
            if source == 'computer':
                self.recorder.feed(MidiEvent('note_off',note=note,channel=channel,source=source))
        self.computer_held.clear()
        self.state.clear_source("computer")
        self.refresh_view()

    def panic(self):
        self.keyboard.release_mouse()
        if hasattr(self, "lesson"):
            self.lesson.keyboard.release_mouse()
        self.computer_held.clear()
        self.state.panic()
        self.refresh_view()

    def eventFilter(self, watched, event):
        if event.type() == QEvent.Type.ApplicationDeactivate:
            self.release_computer()
            self.keyboard.release_mouse()
            self.lesson.keyboard.release_mouse()
            self.lesson.pause()
        editing = isinstance(QApplication.focusWidget(), (QLineEdit, QSpinBox, QDoubleSpinBox)) or QApplication.activeModalWidget() is not None
        if not editing and self.pages.currentIndex() in (1, 5) and event.type() in (QEvent.Type.KeyPress, QEvent.Type.KeyRelease):
            key = event.key()
            if key in KEYS or key == Qt.Key.Key_Space:
                if event.isAutoRepeat():
                    return True
                down = event.type() == QEvent.Type.KeyPress
                if down:
                    if key in self.computer_held:
                        return True
                    self.computer_held.add(key)
                else:
                    self.computer_held.discard(key)
                if key == Qt.Key.Key_Space:
                    self.on_midi(MidiEvent("control_change", control=64, value=127 if down else 0, source="computer"))
                else:
                    self.simulate("note_on" if down else "note_off", KEYS[key], "computer")
                return True
        return super().eventFilter(watched, event)

    def toggle_recording(self):
        if self.recorder.recording:
            self.stop_recording()
        else:
            self.recorder.start()
            self.record_button.setText('■ Сохранить запись')
            self.mobile_menu.setText('● Идёт запись · Разделы')
            self.statusBar().showMessage('Записывается живой ввод. Нажмите ещё раз для сохранения.')

    def stop_recording(self):
        self.recorder.stop()
        if self.recorder.events:
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'recording.mid'
                self.recorder.save(path)
                self.library.save_recording('Игра ' + time.strftime('%d.%m.%Y %H:%M'),path.read_bytes())
            self.statusBar().showMessage('Запись сохранена в «Мои записи».')
        self.record_button.setText('● Записать MIDI')
        self.mobile_menu.setText(f'☰  {APP_NAME} · Разделы')

    def show_recordings(self):
        dialog=QDialog(self); dialog.setWindowTitle('Мои записи'); dialog.resize(580,400)
        layout=QVBoxLayout(dialog); listing=QListWidget(); layout.addWidget(listing)
        rows=self.library.recordings()
        for row in rows: listing.addItem(row['name'])
        if rows: listing.setCurrentRow(0)
        def export():
            index=listing.currentRow()
            if index<0: return
            path,_=QFileDialog.getSaveFileName(dialog,'Экспорт записи','recording.mid','MIDI (*.mid)')
            if path:
                try: write_document(path, rows[index]['data'])
                except OSError as e: QMessageBox.warning(dialog,'Ошибка',str(e))
        def listen():
            index=listing.currentRow()
            if index<0: return
            from pianoflow.midi.loader import load_midi
            try:
                with tempfile.TemporaryDirectory() as directory:
                    path=Path(directory)/'recording.mid'; path.write_bytes(rows[index]['data']); song=load_midi(path)
                self.lesson.set_song(song); self.navigate(5); self.lesson.sound_box.setChecked(True)
                dialog.accept(); self.lesson.toggle()
            except Exception as e: QMessageBox.warning(dialog,'Не удалось открыть запись',str(e))
        layout.addWidget(button('Прослушать',listen)); layout.addWidget(button('Экспорт MIDI…',export)); dialog.exec()

    def backup(self):
        path,_=QFileDialog.getSaveFileName(self,'Резервная копия','Clavunora.zip','ZIP (*.zip)')
        if path:
            try:
                self.library.set_preference('app_settings',self.settings.values)
                with local_document(path, '.zip', output=True) as local:
                    self.library.backup(local)
                self.statusBar().showMessage('Резервная копия сохранена.')
            except Exception as e: QMessageBox.warning(self,'Ошибка копирования',str(e))

    def restore(self):
        path,_=QFileDialog.getOpenFileName(self,'Восстановить библиотеку','','ZIP (*.zip)')
        if not path: return
        try:
            with local_document(path, '.zip') as local:
                data=self.library.read_backup(local)
            dialog=QDialog(self); dialog.setWindowTitle('Восстановление'); layout=QVBoxLayout(dialog)
            layout.addWidget(label(f"В копии: {len(data['songs'])} песен, {len(data['attempts'])} попыток, {len(data['recordings'])} записей."))
            policy=QComboBox(); policy.addItems(['Сохранять текущие записи при совпадении ID','Заменять совпадающие записи данными из копии']); layout.addWidget(policy)
            layout.addWidget(label('Выбор MIDI-устройства и внешние SoundFont-файлы автоматически не переносятся.'))
            buttons=QDialogButtonBox(QDialogButtonBox.StandardButton.Ok|QDialogButtonBox.StandardButton.Cancel)
            buttons.accepted.connect(dialog.accept); buttons.rejected.connect(dialog.reject); layout.addWidget(buttons)
            if not dialog.exec(): return
            self.lesson.pause()
            self.library.restore(data,overwrite=policy.currentIndex()==1)
            self.library_page.refresh(); self.statistics.refresh(); self.refresh_achievements()
            self.lesson.hint_combo.setCurrentIndex(self.library.preference('hints',0))
            self.statusBar().showMessage('Данные восстановлены. Учебные настройки применятся при открытии песни.')
        except Exception as e: QMessageBox.warning(self,'Копия не восстановлена',str(e))

    def closeEvent(self, event):
        QApplication.instance().removeEventFilter(self)
        self.timer.stop()
        self.free_metronome.stop()
        self.lesson.shutdown()
        self.library_page.shutdown()
        if self.recorder.recording: self.stop_recording()
        self.midi.close()
        self.save_settings()
        self.library.close()
        super().closeEvent(event)
