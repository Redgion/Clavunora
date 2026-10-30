# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Small additive preview synthesizer, no SoundFont/downloads required."""
import math
import struct
import threading
from PySide6.QtCore import QIODevice
from PySide6.QtMultimedia import QAudioFormat, QAudioSink, QMediaDevices


class ToneStream(QIODevice):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.lock = threading.Lock()
        self.targets = set()
        self.click_remaining = 0
        self.click_frequency = 1200
        self.voices = {}
        self.open(QIODevice.OpenModeFlag.ReadOnly)

    def set_notes(self, notes):
        with self.lock:
            self.targets = set(sorted(notes)[:24])

    def bytesAvailable(self):
        return 4096 + super().bytesAvailable()

    def readData(self, maxlen):
        count = maxlen // 2
        with self.lock:
            for pitch in self.targets:
                self.voices.setdefault(pitch, [0.0, 0.0])
            samples = []
            for _ in range(count):
                value = 0.0
                if self.click_remaining > 0:
                    value = .15 * math.sin(self.click_remaining * 2 * math.pi * self.click_frequency / 22050) * min(1, self.click_remaining / 100)
                    self.click_remaining -= 1
                for pitch, voice in self.voices.items():
                    target = .13 if pitch in self.targets else 0.0
                    voice[1] += (target - voice[1]) * .012
                    voice[0] = (voice[0] + 2 * math.pi * (440 * 2 ** ((pitch - 69) / 12)) / 22050) % (2 * math.pi)
                    value += (math.sin(voice[0]) + .18 * math.sin(2 * voice[0])) * voice[1]
                samples.append(int(max(-.95, min(.95, value)) * 32767))
            self.voices = {p: v for p, v in self.voices.items() if p in self.targets or v[1] > .0001}
        return struct.pack(f"<{len(samples)}h", *samples)

    def writeData(self, data):
        return -1


class PreviewAudio:
    def __init__(self, parent, device=None, volume=.65):
        self.sink = None
        self.stream = None
        device = device or QMediaDevices.defaultAudioOutput()
        if device.isNull():
            raise RuntimeError("Звуковое устройство не найдено")
        fmt = QAudioFormat()
        fmt.setSampleRate(22050)
        fmt.setChannelCount(1)
        fmt.setSampleFormat(QAudioFormat.SampleFormat.Int16)
        if not device.isFormatSupported(fmt):
            raise RuntimeError("Устройство не поддерживает звук 22050 Гц / mono PCM16")
        self.stream = ToneStream(parent)
        self.sink = QAudioSink(device, fmt, parent)
        self.sink.setBufferSize(4096)
        self.sink.setVolume(volume)
        self.sink.start(self.stream)

    def set_notes(self, notes):
        if self.stream:
            self.stream.set_notes(notes)

    def click(self, accent=False):
        if self.stream:
            with self.stream.lock:
                self.stream.click_frequency = 1600 if accent else 1000
                self.stream.click_remaining = 660

    def close(self):
        if self.sink:
            self.sink.stop()
        if self.stream:
            self.stream.close()
