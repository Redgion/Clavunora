# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Input state independent of Qt and MIDI backend; sources/channels stay isolated."""
from dataclasses import dataclass


@dataclass(frozen=True)
class MidiEvent:
    type: str
    note: int = 0
    velocity: int = 0
    channel: int = 0
    control: int = 0
    value: int = 0
    source: str = "midi"
    timestamp: float = 0.0


class PianoState:
    def __init__(self):
        self.held: dict[tuple[str, int, int], int] = {}
        self.latched: dict[tuple[str, int, int], int] = {}
        self.pedals: set[tuple[str, int]] = set()
        self.last_velocity = 0
        self.note_count = 0

    @property
    def pressed(self) -> set[int]:
        return {key[2] for key in self.held}

    @property
    def sounding(self) -> set[int]:
        return self.pressed | {key[2] for key in self.latched}

    def apply(self, event: MidiEvent):
        key = (event.source, event.channel, event.note)
        lane = (event.source, event.channel)
        if event.type == "note_on" and event.velocity > 0:
            self.held[key] = event.velocity
            self.latched.pop(key, None)
            self.last_velocity = event.velocity
            self.note_count += 1
        elif event.type in ("note_off", "note_on"):
            velocity = self.held.pop(key, None)
            if velocity is not None and lane in self.pedals:
                self.latched[key] = velocity
        elif event.type == "control_change":
            if event.control == 64:
                if event.value >= 64:
                    self.pedals.add(lane)
                else:
                    self.pedals.discard(lane)
                    self.latched = {k: v for k, v in self.latched.items() if k[:2] != lane}
            elif event.control in (120, 123):
                self.held = {k: v for k, v in self.held.items() if k[:2] != lane}
                self.latched = {k: v for k, v in self.latched.items() if k[:2] != lane}
            elif event.control == 121:
                self.pedals.discard(lane)
                self.latched = {k: v for k, v in self.latched.items() if k[:2] != lane}

    def clear_source(self, source: str):
        self.held = {k: v for k, v in self.held.items() if k[0] != source}
        self.latched = {k: v for k, v in self.latched.items() if k[0] != source}
        self.pedals = {k for k in self.pedals if k[0] != source}

    def panic(self):
        self.held.clear()
        self.latched.clear()
        self.pedals.clear()
