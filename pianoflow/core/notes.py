# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Musical naming and conservative pitch-class chord recognition."""
NAMES = ("C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B")
BLACK = {1, 3, 6, 8, 10}


def note_name(note: int) -> str:
    return f"{NAMES[note % 12]}{note // 12 - 1}"


def chord_name(notes: set[int]) -> str:
    pitches = {n % 12 for n in notes}
    if len(pitches) < 3:
        return " · ".join(note_name(n) for n in sorted(notes)) or "Сыграйте ноту"
    patterns = (({0, 4, 7}, ""), ({0, 3, 7}, "m"), ({0, 3, 6}, "dim"),
                ({0, 4, 8}, "aug"), ({0, 2, 7}, "sus2"), ({0, 5, 7}, "sus4"),
                ({0, 4, 7, 10}, "7"), ({0, 4, 7, 11}, "maj7"), ({0, 3, 7, 10}, "m7"))
    bass = min(notes) % 12
    for root in [bass] + sorted(pitches - {bass}):
        for intervals, suffix in patterns:
            if {(root + i) % 12 for i in intervals} == pitches:
                return NAMES[root] + suffix + (f"/{NAMES[bass]}" if root != bass else "")
    return " · ".join(NAMES[n] for n in sorted(pitches))
