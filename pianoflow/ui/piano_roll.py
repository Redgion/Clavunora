# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Falling notes, aligned exactly to the keyboard below using shared geometry."""
from bisect import bisect_left, bisect_right
from PySide6.QtCore import QRectF, Qt
from PySide6.QtGui import QColor, QFont, QPainter, QPen
from PySide6.QtWidgets import QWidget
from pianoflow.core.notes import BLACK, note_name
from pianoflow.ui.theme import is_dark
from pianoflow.ui.keyboard import key_geometry

COLORS = ("#78d5af", "#b5b1ee", "#efc881", "#8ccfee")


class PianoRoll(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.show_labels = True
        self.show_fingers = True
        self.hand = ""
        self.song = None
        self.position = -3.0
        self.first, self.last = 21, 108
        self.horizon = 4.0
        self.visible_notes = []
        self.starts = []
        self.max_duration = 0
        self.selected_tracks = None
        self.setMinimumHeight(160)
        self.setAccessibleName("Падающие ноты")

    def set_song(self, song):
        self.song = song
        self.visible_notes = sorted(song.notes, key=lambda n: n.start)
        self.starts = [n.start for n in self.visible_notes]
        self.max_duration = max((n.duration for n in self.visible_notes), default=0)
        self.update()

    def note_rect(self, note, keys):
        key = keys[note.pitch]
        scale = (self.height() - 12) / self.horizon
        bottom = self.height() - 8 - (note.start - self.position) * scale
        return QRectF(key.left() + 1.5, bottom - note.duration * scale,
                      max(2, key.width() - 3), max(4, note.duration * scale))

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.fillRect(self.rect(), QColor("#1b2937" if is_dark() else "#eff9fd"))
        keys = key_geometry(self.first, self.last, self.width(), self.height())
        for pitch, rect in keys.items():
            if pitch % 12 not in BLACK:
                if pitch % 12 == 0:
                    painter.fillRect(QRectF(rect.left(), 0, rect.width(), self.height()), QColor("#203243" if is_dark() else "#e0eff5"))
                painter.setPen(QColor("#30465b" if is_dark() else "#c8e0eb"))
                painter.drawLine(int(rect.left()), 0, int(rect.left()), self.height())
        if self.song:
            scale = (self.height() - 12) / self.horizon
            # Horizontal lines mark seconds, not beats: arbitrary tempo maps remain honest.
            for second in range(int(self.position // 1), int(self.position + self.horizon) + 2):
                y = self.height() - 8 - (second - self.position) * scale
                painter.setPen(QColor("#3b5064" if is_dark() else "#bdd7e3"))
                painter.drawLine(0, int(y), self.width(), int(y))
            left = bisect_left(self.starts, self.position - self.max_duration)
            right = bisect_right(self.starts, self.position + self.horizon)
            for note in self.visible_notes[left:right]:
                if self.hand and note.hand != self.hand:
                    continue
                if note.pitch not in keys or note.end < self.position:
                    continue
                if self.selected_tracks is not None and note.track not in self.selected_tracks:
                    continue
                rect = self.note_rect(note, keys)
                painter.setPen(Qt.PenStyle.NoPen)
                painter.setBrush(QColor(COLORS[0 if note.hand == "R" else 1 if note.hand == "L" else note.track % len(COLORS)]))
                painter.drawRoundedRect(rect, 4, 4)
                if rect.width() >= 22 and rect.height() >= 18:
                    painter.setFont(QFont("Sans Serif", 8))
                    painter.setPen(QColor("#14242b"))
                    text = note_name(note.pitch) if self.show_labels else ""
                    if self.show_fingers and note.finger:
                        text += f" {note.hand}{note.finger}"
                    painter.drawText(rect, Qt.AlignmentFlag.AlignCenter, text)
        painter.setPen(QPen(QColor("#2188ac"), 2))
        painter.drawLine(0, self.height() - 8, self.width(), self.height() - 8)
        painter.end()
