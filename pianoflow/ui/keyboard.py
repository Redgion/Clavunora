# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Responsive piano geometry shared by painting and mouse hit testing."""
from PySide6.QtCore import QEvent, QRectF, Qt, Signal
from PySide6.QtGui import QColor, QPainter, QFont, QLinearGradient
from PySide6.QtWidgets import QWidget
from pianoflow.core.notes import BLACK, note_name


def key_geometry(first: int, last: int, width: float, height: float):
    whites = [n for n in range(first, last + 1) if n % 12 not in BLACK]
    unit = width / len(whites)
    keys = {n: QRectF(i * unit, 0, unit, height) for i, n in enumerate(whites)}
    preceding = 0
    for n in range(first, last + 1):
        if n % 12 not in BLACK:
            preceding += 1
        else:
            keys[n] = QRectF(preceding * unit - unit * .31, 0, unit * .62, height * .62)
    return keys


class PianoKeyboard(QWidget):
    note_pressed = Signal(int)
    note_released = Signal(int)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.first, self.last = 21, 108
        self.pressed, self.sounding, self.expected = set(), set(), set()
        self.fingers = {}
        self.show_labels = True
        self.mouse_note = None
        self.touch_notes = set()
        self.setAttribute(Qt.WidgetAttribute.WA_AcceptTouchEvents)
        self.setMinimumHeight(150)
        self.setMouseTracking(True)
        self.setAccessibleName("Виртуальная клавиатура пианино")

    def set_size(self, size):
        self.release_mouse()
        self.first, self.last = {88: (21, 108), 61: (36, 96), 49: (36, 84)}[size]
        self.update()

    def hit(self, point):
        keys = key_geometry(self.first, self.last, self.width(), self.height() - 1)
        for black in (True, False):
            for note, rect in keys.items():
                if (note % 12 in BLACK) == black and rect.contains(point):
                    return note
        return None

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        keys = key_geometry(self.first, self.last, self.width(), self.height() - 1)
        for black in (False, True):
            for note, rect in keys.items():
                if (note % 12 in BLACK) != black:
                    continue
                color = "#253e50" if black else "#f3fbff"
                if note in self.expected:
                    color = "#a6a0ec"
                if note in self.sounding:
                    color = "#468e88"
                if note in self.pressed:
                    color = "#73e0c1"
                painter.setPen(QColor("#678c9f"))
                surface = QLinearGradient(0, rect.top(), 0, rect.bottom())
                surface.setColorAt(0, QColor(color).lighter(125))
                surface.setColorAt(.16, QColor(color))
                surface.setColorAt(.9, QColor(color))
                surface.setColorAt(1, QColor(color).darker(112))
                painter.setBrush(surface)
                painter.drawRoundedRect(rect.adjusted(.6, 0, -.6, -.5), 3, 3)
                if note in self.fingers:
                    painter.setPen(QColor("#28304b"))
                    painter.setFont(QFont("Sans Serif", 10))
                    painter.drawText(rect.adjusted(0, 8, 0, -rect.height()+35), Qt.AlignmentFlag.AlignCenter, self.fingers[note])
                if self.show_labels and (not black or rect.width() > 20):
                    painter.setPen(QColor("#d6e8f2" if black and note not in self.expected and note not in self.pressed else "#345269"))
                    painter.setFont(QFont("Sans Serif", 8 if rect.width() > 18 else 6))
                    label = note_name(note) if note % 12 == 0 or rect.width() > 28 else ""
                    painter.drawText(rect.adjusted(0, rect.height() - 28, 0, -4), Qt.AlignmentFlag.AlignCenter, label)
        painter.end()

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.move_mouse(event.position())

    def mouseMoveEvent(self, event):
        if event.buttons() & Qt.MouseButton.LeftButton:
            self.move_mouse(event.position())

    def move_mouse(self, point):
        note = self.hit(point)
        if note != self.mouse_note:
            self.release_mouse()
            self.mouse_note = note
            if note is not None:
                self.note_pressed.emit(note)

    def release_mouse(self):
        self.update_touch_notes(set())
        if self.mouse_note is not None:
            note, self.mouse_note = self.mouse_note, None
            self.note_released.emit(note)

    def update_touch_notes(self, notes):
        for note in self.touch_notes - notes:
            self.note_released.emit(note)
        for note in notes - self.touch_notes:
            self.note_pressed.emit(note)
        self.touch_notes = set(notes)

    def event(self, event):
        kind = event.type()
        if kind in (QEvent.Type.TouchBegin, QEvent.Type.TouchUpdate, QEvent.Type.TouchEnd):
            from PySide6.QtGui import QEventPoint
            notes = {self.hit(p.position()) for p in event.points()
                     if p.state() != QEventPoint.State.Released}
            self.update_touch_notes(notes - {None})
            event.accept()
            return True
        if kind in (QEvent.Type.TouchCancel, QEvent.Type.Hide, QEvent.Type.WindowDeactivate):
            self.release_mouse()
        return super().event(event)

    def mouseReleaseEvent(self, event):
        self.release_mouse()
