# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Monotonic playback clock. Animation frames never determine musical time."""
import time


class Transport:
    def __init__(self, clock=time.monotonic):
        self.clock = clock
        self.position = -3.0
        self.speed = 1.0
        self.playing = False
        self.duration = 0.0
        self._anchor = clock()
        self._position_anchor = self.position

    def current(self):
        if self.playing:
            return min(self.duration, self._position_anchor + (self.clock() - self._anchor) * self.speed)
        return self.position

    def seek(self, position):
        self.position = max(-3.0, min(self.duration, position))
        self._anchor = self.clock()
        self._position_anchor = self.position

    def play(self):
        self.seek(self.current())
        self.playing = True

    def pause(self):
        self.seek(self.current())
        self.playing = False

    def set_speed(self, speed):
        if not .25 <= speed <= 2:
            raise ValueError("Playback speed outside 25–200%")
        self.seek(self.current())
        self.speed = speed
