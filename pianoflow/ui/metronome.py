# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import time
from PySide6.QtCore import QTimer, Qt
from PySide6.QtWidgets import QWidget,QHBoxLayout,QCheckBox,QSpinBox,QLabel
from pianoflow.audio.preview import PreviewAudio


class Metronome(QWidget):
    def __init__(self,parent=None):
        super().__init__(parent)
        row=QHBoxLayout(self); row.setContentsMargins(0,0,0,0)
        self.enabled=QCheckBox('Метроном'); self.bpm=QSpinBox(); self.bpm.setRange(30,240); self.bpm.setValue(80); self.bpm.setSuffix(' BPM')
        self.beats=QSpinBox(); self.beats.setRange(1,12); self.beats.setValue(4); self.beats.setPrefix('Долей: ')
        self.indicator=QLabel('○'); self.indicator.setWordWrap(True)
        for w in (self.enabled,self.bpm,self.beats,self.indicator): row.addWidget(w)
        row.addStretch(); self.audio=None; self.started=0.; self.previous=-1; self.error=''
        self.timer=QTimer(self); self.timer.setInterval(10); self.timer.setTimerType(Qt.TimerType.PreciseTimer); self.timer.timeout.connect(self.advance)
        self.enabled.toggled.connect(self.toggle); self.bpm.valueChanged.connect(self.reset); self.beats.valueChanged.connect(self.reset)

    def reset(self,*_): self.started=time.monotonic(); self.previous=-1

    def toggle(self,enabled):
        if enabled:
            self.reset(); self.error=''
            try: self.audio=PreviewAudio(self)
            except Exception as e: self.error=' · без звука: '+str(e)
            self.timer.start(); self.advance()
        else:
            self.timer.stop()
            if self.audio: self.audio.close(); self.audio=None
            self.indicator.setText('○')

    def advance(self):
        beat=int((time.monotonic()-self.started)*self.bpm.value()/60)
        if beat!=self.previous:
            self.previous=beat
            self.indicator.setText(f'● {beat%self.beats.value()+1}'+self.error)
            if self.audio: self.audio.click(beat%self.beats.value()==0)

    def stop(self): self.enabled.setChecked(False)
