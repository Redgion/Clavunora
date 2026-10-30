# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from pianoflow.ui.responsive import FlowLayout
"""Practice page: score transport, waiting for notes, MIDI import and rendering."""
from bisect import bisect_left, bisect_right
from pathlib import Path
from pianoflow.core.files import local_document
from PySide6.QtCore import QThread, Qt, QTimer, Signal
from PySide6.QtWidgets import (
    QWidget, QLabel, QPushButton, QVBoxLayout, QHBoxLayout, QComboBox,
    QSlider, QCheckBox, QFileDialog, QStackedWidget, QScrollArea, QFrame,
)
from pianoflow.core.notes import note_name
from pianoflow.core.song import birthday_song
from pianoflow.core.transport import Transport
from pianoflow.midi.loader import load_midi
from pianoflow.ui.keyboard import PianoKeyboard
from pianoflow.ui.piano_roll import PianoRoll
from pianoflow.audio.preview import PreviewAudio
from pianoflow.learning.engine import WaitPractice
from pianoflow.ui.practice_tools import PracticeTools
from pianoflow.ui.staff import StaffView


class ImportThread(QThread):
    loaded = Signal(object)
    failed = Signal(str)

    def __init__(self, path, parent):
        super().__init__(parent)
        self.path = path

    def run(self):
        try:
            with local_document(self.path, '.mid') as local:
                if local.suffix.lower() in (".musicxml", ".xml"):
                    from pianoflow.midi.musicxml import load_musicxml
                    self.loaded.emit(load_musicxml(local))
                else:
                    self.loaded.emit(load_midi(local))
        except Exception as exc:
            self.failed.emit(str(exc) or "Некорректный MIDI-файл")


def text_label(text, name=None):
    label = QLabel(text)
    label.setWordWrap(True)
    if name:
        label.setObjectName(name)
    return label


class LessonPage(PracticeTools, QWidget):
    input_note = Signal(str, int)
    attempt_saved = Signal()

    def __init__(self, state, parent=None, store=None):
        super().__init__(parent)
        self.state = state
        self.transport = Transport()
        self.song = birthday_song()
        self.running = False
        self.waiting = False
        self.groups = []
        self.group_index = 0
        self.completed = 0
        self.accepted_notes = set()
        self.audio = None
        self.importer = None
        self.selected_tracks = None
        outer_layout = QVBoxLayout(self)
        outer_layout.setContentsMargins(30,18,30,18)
        self.header_body=QWidget()
        layout = QVBoxLayout(self.header_body)
        layout.setContentsMargins(0,0,0,0)
        layout.setSpacing(7)
        layout.setAlignment(Qt.AlignmentFlag.AlignTop)
        header = QHBoxLayout()
        self.title = text_label(self.song.title, "title")
        header.addWidget(self.title, 1)
        self.import_button = QPushButton("Импорт…")
        self.import_button.clicked.connect(self.choose_midi)
        header.addWidget(self.import_button)
        layout.addLayout(header)
        self.meta = text_label("", "muted")
        layout.addWidget(self.meta)
        self.transport_bar = QWidget()
        transport_controls = FlowLayout(self.transport_bar)
        layout.addWidget(self.transport_bar)
        controls = transport_controls
        self.play_button = QPushButton("▶  Начать")
        self.play_button.setObjectName("primary")
        self.play_button.clicked.connect(self.toggle)
        transport_controls.addWidget(self.play_button)
        restart = QPushButton("↺  Сначала")
        restart.clicked.connect(self.restart)
        transport_controls.addWidget(restart)
        self.speed = QComboBox()
        for speed in (.25, .5, .75, 1., 1.25, 1.5):
            self.speed.addItem(f"{speed * 100:g}%", speed)
        self.speed.setCurrentIndex(3)
        self.speed.currentIndexChanged.connect(self.speed_changed)
        transport_controls.addWidget(self.speed)
        self.zoom_box = QCheckBox("По мелодии")
        self.zoom_box.setChecked(True)
        self.zoom_box.toggled.connect(self.update_range)
        controls.addWidget(self.zoom_box)
        self.touch_octave = None
        self.keyboard_size = 88
        self.wait_box = QCheckBox("Ждать ноту")
        self.wait_box.setChecked(False)
        self.wait_box.toggled.connect(self.wait_changed)
        controls.addWidget(self.wait_box)
        self.sound_box = QCheckBox("Звук")
        self.sound_box.setToolTip("Включить звук; тембр и устройство — в «Практика…»")
        self.sound_box.toggled.connect(self.sound_changed)
        controls.addWidget(self.sound_box)
        controls.addStretch()
        track_row = FlowLayout()
        self.track_combo = QComboBox()
        self.track_combo.setSizeAdjustPolicy(QComboBox.SizeAdjustPolicy.AdjustToMinimumContentsLengthWithIcon)
        self.track_combo.setMinimumContentsLength(16)
        self.track_combo.currentIndexChanged.connect(self.track_changed)
        track_row.addWidget(self.track_combo)
        track_row.addWidget(text_label("Правая рука — зелёная, левая — сиреневая. Без разметки цвет обозначает дорожку.", "muted"), 1)
        layout.addLayout(track_row)
        self.init_tools(layout, store)
        self.message = text_label("Нажмите «Начать»: ноты движутся к светящейся линии над клавишами.", "eyebrow")
        layout.addWidget(self.message)
        layout.removeWidget(self.finger_label)
        layout.removeWidget(self.message)
        # Scroll settings separately so the keyboard stays reachable on a phone.
        self.header_scroll = QScrollArea()
        self.header_scroll.setWidgetResizable(True)
        self.header_scroll.setFrameShape(QFrame.Shape.NoFrame)
        self.header_scroll.setWidget(self.header_body)
        layout=outer_layout
        layout.addWidget(self.header_scroll)
        layout.addWidget(self.finger_label)
        layout.addWidget(self.message)
        # Zero spacing and identical widths preserve exact roll-to-key alignment.
        piano = QVBoxLayout()
        piano.setSpacing(0)
        self.roll = PianoRoll()
        self.keyboard = PianoKeyboard()
        self.keyboard.setMinimumHeight(120)
        self.keyboard.setMaximumHeight(160)
        self.keyboard.note_pressed.connect(lambda n: self.input_note.emit("note_on", n))
        self.keyboard.note_released.connect(lambda n: self.input_note.emit("note_off", n))
        self.staff = StaffView()
        self.visuals = QStackedWidget()
        self.visuals.addWidget(self.roll)
        self.visuals.addWidget(self.staff)
        piano.addWidget(self.visuals, 1)
        self.octave_bar = QWidget()
        octave_layout = QHBoxLayout(self.octave_bar)
        octave_layout.setContentsMargins(0, 0, 0, 0)
        for title, direction in [('← Октава', -1), ('Октава →', 1)]:
            b = QPushButton(title)
            b.clicked.connect(lambda checked=False, d=direction: self.shift_octave(d))
            octave_layout.addWidget(b)
        self.octave_bar.hide()
        piano.addWidget(self.octave_bar)
        piano.addWidget(self.keyboard)
        layout.addLayout(piano, 1)
        timeline = QHBoxLayout()
        self.position_label = text_label("00:00 / 00:00", "muted")
        self.position_label.setMinimumWidth(130)
        timeline.addWidget(self.position_label)
        self.slider = QSlider(Qt.Orientation.Horizontal)
        self.slider.sliderPressed.connect(self.scrub_start)
        self.slider.sliderReleased.connect(self.scrub_end)
        timeline.addWidget(self.slider, 1)
        layout.addLayout(timeline)
        self.desktop_hint = text_label("A W S E D F T G Y H U J K = C4–C5 · B♭4 — клавиша U · Пробел — педаль", "muted")
        layout.addWidget(self.desktop_hint)
        self.frame_timer = QTimer(self)
        self.frame_timer.setTimerType(Qt.TimerType.PreciseTimer)
        self.frame_timer.setInterval(16)
        self.frame_timer.timeout.connect(self.advance)
        self.set_song(self.song)

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if hasattr(self, 'practice_notes'):
            self.keyboard.release_mouse()
            self.update_range()

    def filtered_notes(self):
        return [n for n in self.song.notes if (self.selected_tracks is None or n.track in self.selected_tracks) and (not self.hand or n.hand == self.hand)]

    def set_song(self, song):
        self.pause()
        self.song = song
        self.touch_octave = None
        self.song_id = None
        self.loop_enabled = False
        self.loop_start = 0.
        self.loop_end = song.duration
        self.hand = ""
        self.hand_combo.blockSignals(True)
        self.hand_combo.setCurrentIndex(0)
        self.hand_combo.blockSignals(False)
        self.roll.hand = ""
        self.selected_tracks = None
        self.title.setText(song.title)
        self.transport.duration = song.duration + .5
        self.roll.set_song(song)
        self.staff.set_song(song)
        self.slider.setRange(0, int(self.transport.duration * 1000))
        self.track_combo.blockSignals(True)
        self.track_combo.clear()
        self.track_combo.addItem("Все дорожки", None)
        for track in sorted({n.track for n in song.notes}):
            name = song.track_names[track] if track < len(song.track_names) else str(track + 1)
            self.track_combo.addItem(f"{track + 1}. {name}", track)
        self.track_combo.blockSignals(False)
        self.meta.setText(f"{len(song.notes)} нот · {song.bpm:g} BPM (начальный) · {song.time_signature[0]}/{song.time_signature[1]} · Мелодия / MIDI")
        self.build_groups()
        self.update_range()
        self.restart()

    def build_groups(self):
        self.practice_notes = sorted(self.filtered_notes(), key=lambda n: n.start)
        self.practice_starts = [n.start for n in self.practice_notes]
        self.max_duration = max((n.duration for n in self.practice_notes), default=0)
        self.wait_engine = WaitPractice(self.practice_notes)
        self.groups = self.wait_engine.groups
        self.group_times = [start for start, _ in self.groups]

    def toggle(self):
        if self.running:
            self.pause()
        else:
            if self.transport.current() >= self.transport.duration:
                self.restart()
            self.start_attempt()
            self.running = True
            if not self.waiting:
                self.transport.play()
            self.play_button.setText("Ⅱ  Пауза")
            self.frame_timer.start()
            self.advance()

    def pause(self):
        self.finish_attempt(False)
        self.transport.pause()
        self.running = False
        self.frame_timer.stop()
        self.play_button.setText("▶  Продолжить")
        if self.audio:
            self.audio.set_notes(set())

    def restart(self):
        self.pause()
        self.transport.seek(max(-3., self.loop_start - 1) if self.loop_enabled else -3.)
        self.group_index = bisect_left(self.group_times, self.loop_start) if self.loop_enabled else 0
        self.completed = 0
        self.waiting = False
        self.accepted_notes.clear()
        self.play_button.setText("▶  Начать")
        self.message.setText("Три секунды на подготовку. Играйте, когда нота касается светящейся линии.")
        self.render_position()

    def speed_changed(self):
        self.finish_attempt(False)
        self.transport.set_speed(self.speed.currentData())
        if self.running: self.start_attempt()

    def wait_changed(self):
        self.seek(self.transport.current())

    def track_changed(self):
        self.finish_attempt(False)
        value = self.track_combo.currentData()
        self.selected_tracks = None if value is None else {value}
        self.roll.selected_tracks = self.selected_tracks
        self.build_groups()
        self.update_range()
        self.seek(self.transport.current())

    def seek(self, position):
        self.finish_attempt(False)
        self.transport.seek(position)
        self.group_index = bisect_left(self.group_times, max(0, position, self.loop_start if self.loop_enabled else 0))
        self.waiting = False
        self.accepted_notes.clear()
        self.completed = 0
        if self.running:
            self.start_attempt()
            self.transport.play()
        self.render_position()

    def scrub_start(self):
        self.pause()

    def scrub_end(self):
        self.seek(self.slider.value() / 1000)
        self.message.setText("Позиция изменена. Нажмите «Продолжить».")

    def advance(self):
        position = self.transport.current()
        if self.loop_tick(position):
            self.render_position()
            return
        if self.running and self.wait_box.isChecked() and not self.waiting and self.group_index < len(self.groups):
            start, notes = self.groups[self.group_index]
            if position >= start:
                self.transport.pause()
                self.transport.seek(start)
                self.waiting = True
                self.accepted_notes.clear()
                self.message.setText("Сыграйте: " + " + ".join(note_name(n) for n in sorted(notes)))
        if self.running and not self.waiting and position >= self.transport.duration:
            report = self.finish_attempt(True)
            self.pause()
            self.message.setText(f"Мелодия закончилась! Верных групп нот: {self.completed}. Нажмите «Сначала» для повтора." if self.wait_box.isChecked() else "Мелодия закончилась. Попробуйте режим «Ждать ноту» или снизьте скорость.")
            if report:
                self.message.setText(self.message.text() + f" Точность нот: {report['accuracy']:g}%. Подробности — в «Статистика».")
                if report['accuracy'] >= 90 and not report['mistakes'] and self.hints < 2:
                    self.message.setText(self.message.text() + ' Попробуйте следующий уровень подсказок.')
        self.render_position()

    def note_input(self, event):
        self.practice_input(event)
        if not self.running or not self.waiting or event.type != "note_on" or event.velocity <= 0:
            return
        self.wait_engine.index = self.group_index
        self.wait_engine.completed = self.completed
        self.wait_engine.accepted = self.accepted_notes
        if self.wait_engine.accept(event, self.state.pressed):
            self.completed = self.wait_engine.completed
            self.group_index = self.wait_engine.index
            self.waiting = False
            self.transport.play()
            self.message.setText(f"Верно! Пройдено групп: {self.completed}. Продолжайте…")

    def render_position(self):
        position = self.transport.current()
        self.staff.show_fingers = self.hints < 2
        self.staff.position = position
        self.staff.hand = self.hand
        self.staff.selected_tracks = self.selected_tracks
        self.staff.update()
        self.roll.position = position
        self.roll.update()
        left = bisect_left(self.practice_starts, position - self.max_duration)
        right = bisect_right(self.practice_starts, position)
        active = {n.pitch for n in self.practice_notes[left:right] if n.start <= position < n.end}
        self.keyboard.expected = self.groups[self.group_index][1] if self.waiting else active
        self.update_hints(position, active)
        self.refresh_input()
        shown = max(0, int(position))
        total = int(self.transport.duration)
        countdown = f"Начало через {max(1, int(-position) + 1)}…" if position < 0 else f"{shown // 60:02}:{shown % 60:02} / {total // 60:02}:{total % 60:02}"
        self.position_label.setText(countdown)
        if not self.slider.isSliderDown():
            self.slider.setValue(max(0, int(position * 1000)))
        if self.audio:
            if self.accompany and self.hand:
                active = {n.pitch for n in self.song.notes if n.hand and n.hand != self.hand and n.start <= position < n.end}
            self.audio.set_notes((active if self.running and not self.waiting else set()) | (self.state.sounding if self.isVisible() else set()))

    def refresh_input(self):
        self.keyboard.pressed = self.state.pressed
        self.keyboard.sounding = self.state.sounding
        self.keyboard.update()
        if self.audio and not self.running:
            self.audio.set_notes(self.state.sounding if self.isVisible() else set())

    def sound_changed(self, enabled):
        if enabled:
            try:
                if self.audio_engine == "fluid":
                    from pianoflow.audio.fluidsynth import FluidAudio
                    self.audio = FluidAudio(self,self.soundfont,self.audio_device,self.volume)
                else:
                    self.audio = PreviewAudio(self,self.audio_device,self.volume)
            except Exception as exc:
                self.sound_box.blockSignals(True)
                self.sound_box.setChecked(False)
                self.sound_box.blockSignals(False)
                self.message.setText(f"Звук недоступен: {exc}. Падающие ноты работают без звука.")
        elif self.audio:
            self.audio.close()
            self.audio = None

    def choose_midi(self):
        path, _ = QFileDialog.getOpenFileName(self, "Открыть MIDI", "", "Музыка (*.mid *.midi *.musicxml *.xml)")
        if path:
            self.import_path(path)

    def import_path(self, path):
        if self.importer and self.importer.isRunning():
            return
        self.pause()
        self.import_button.setEnabled(False)
        self.message.setText(f"Загрузка: {Path(path).name}…")
        self.importer = ImportThread(path, self)
        self.importer.loaded.connect(self.imported_song)
        self.importer.failed.connect(lambda error: self.message.setText("Ошибка импорта: " + error))
        self.importer.finished.connect(lambda: self.import_button.setEnabled(True))
        self.importer.start()

    def imported_song(self, song):
        self.set_song(song)
        if self.store:
            try:
                self.song_id = self.store.add(song)
            except Exception as exc:
                self.message.setText(f"Мелодия открыта, но не сохранена: {exc}")

    def set_keyboard_size(self, size):
        self.keyboard_size = size
        self.update_range()

    def shift_octave(self, direction):
        self.keyboard.release_mouse()
        self.touch_octave = min(96, max(24, self.keyboard.first + direction * 12))
        self.update_range()

    def update_range(self, *_):
        self.keyboard.set_size(self.keyboard_size)
        if self.zoom_box.isChecked() and self.practice_notes:
            low = min(n.pitch for n in self.practice_notes)
            high = max(n.pitch for n in self.practice_notes)
            self.keyboard.first = max(21, (low // 12 - 1) * 12)
            self.keyboard.last = min(108, (high // 12 + 1) * 12)
        if self.width() < 600:
            first = self.touch_octave
            if first is None:
                first = (self.practice_notes[0].pitch // 12) * 12 if self.practice_notes else 60
            self.keyboard.first = min(96, max(24, first))
            self.keyboard.last = self.keyboard.first + 12
        self.roll.first, self.roll.last = self.keyboard.first, self.keyboard.last
        self.keyboard.update()
        self.roll.update()

    def shutdown(self):
        self.pause()
        if self.audio:
            self.audio.close()
        if self.importer:
            self.importer.wait()
