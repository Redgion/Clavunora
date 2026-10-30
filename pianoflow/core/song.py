# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Immutable score in seconds; no UI or playback clock dependencies."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Note:
    pitch: int
    start: float
    duration: float
    velocity: int = 88
    track: int = 0
    channel: int = 0
    hand: str = ""
    finger: int = 0

    @property
    def end(self):
        return self.start + self.duration


@dataclass(frozen=True)
class Song:
    title: str
    notes: tuple[Note, ...]
    bpm: float = 96.0
    time_signature: tuple[int, int] = (3, 4)
    track_names: tuple[str, ...] = ("Мелодия",)
    difficulty: int = 0
    composer: str = ""
    work: str = ""
    description: str = ""
    source: str = ""
    tempo_map: tuple[tuple[float, float], ...] = ()

    @property
    def duration(self):
        return max((note.end for note in self.notes), default=0.0)


def birthday_song() -> Song:
    """Our simple monophonic arrangement in F major, within C4–C5."""
    phrases = (
        ((60, .75), (60, .25), (62, 1), (60, 1), (65, 1), (64, 2)),
        ((60, .75), (60, .25), (62, 1), (60, 1), (67, 1), (65, 2)),
        ((60, .75), (60, .25), (72, 1), (69, 1), (65, 1), (64, 1), (62, 1)),
        ((70, .75), (70, .25), (69, 1), (65, 1), (67, 1), (65, 2)),
    )
    notes = []
    beat = 0.0
    seconds = 60 / 96
    for phrase in phrases:
        for pitch, length in phrase:
            notes.append(Note(pitch, beat * seconds, length * seconds * .9))
            beat += length
    return Song("С днём рождения", tuple(notes))
