# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Score matching uses musical seconds; UI supplies time and pressed pitches."""
from dataclasses import dataclass, asdict, replace
from bisect import bisect_left, bisect_right


class WaitPractice:
    def __init__(self, notes):
        grouped = {}
        for note in notes:
            grouped.setdefault(note.start, set()).add(note.pitch)
        self.groups = sorted(grouped.items())
        self.reset()

    def reset(self, position=0):
        self.index = next((i for i, (t, _) in enumerate(self.groups) if t >= position), len(self.groups))
        self.completed = 0
        self.accepted = set()

    def accept(self, event, pressed):
        if self.index >= len(self.groups) or event.type != 'note_on' or event.velocity <= 0:
            return False
        required = self.groups[self.index][1]
        if event.note in required:
            self.accepted.add(event.note)
        if required <= self.accepted and required <= pressed:
            self.index += 1
            self.completed += 1
            self.accepted.clear()
            return True
        return False


@dataclass
class Mistake:
    position: float
    pitch: int
    kind: str
    delta: float = 0


class Performance:
    def __init__(self, notes, speed=1., tolerance=.15, latency=0., start=0., end=None):
        # A unison shared by both staves is one physical piano key, not two hits.
        # Keep the longest duration when voices tie that key for different lengths.
        physical = {}
        for n in notes:
            if n.start < start or (end is not None and n.start >= end):
                continue
            key = (n.pitch, round(n.start, 7))
            if key not in physical or n.duration > physical[key].duration:
                physical[key] = n
        self.notes = sorted(physical.values(), key=lambda n: (n.start, n.pitch))
        self.pitch_index = {}
        for i, note in enumerate(self.notes):
            times, indices = self.pitch_index.setdefault(note.pitch, ([], []))
            times.append(note.start)
            indices.append(i)
        self.boundary = end
        self.ignore_release = {i for i,n in enumerate(self.notes) if end is not None and n.end >= end}
        self.speed = speed
        self.tolerance = tolerance * speed
        self.latency = latency * speed
        self.hit = set()
        self.active = {}
        self.mistakes = []
        self.finished = False

    def feed(self, event, position):
        if self.finished:
            return
        position -= self.latency
        key = (event.source, event.channel, event.note)
        if event.type == 'note_on' and event.velocity > 0:
            times, indices = self.pitch_index.get(event.note, ([], []))
            radius = max(.5 * self.speed, self.tolerance)
            first = bisect_left(times, position - radius)
            last = bisect_right(times, position + radius)
            candidates = [(abs(self.notes[i].start - position), i)
                          for i in indices[first:last] if i not in self.hit]
            if not candidates:
                self.mistakes.append(Mistake(max(0, position), event.note, 'Лишняя нота'))
                return
            _, i = min(candidates)
            self.hit.add(i)
            self.active.setdefault(key, []).append(i)
            delta = position - self.notes[i].start
            if abs(delta) > self.tolerance:
                self.mistakes.append(Mistake(self.notes[i].start, event.note, 'Поздно' if delta > 0 else 'Рано', delta / self.speed))
        elif event.type in ('note_off', 'note_on') and self.active.get(key):
            i = self.active[key].pop(0)
            delta = position - self.notes[i].end
            if i not in self.ignore_release and abs(delta) > max(self.tolerance, .2 * self.speed):
                self.mistakes.append(Mistake(self.notes[i].start, event.note, 'Долгое удержание' if delta > 0 else 'Короткое удержание', delta / self.speed))

    def finish(self, complete=True):
        if not self.finished:
            if complete:
                for i, note in enumerate(self.notes):
                    if i not in self.hit:
                        self.mistakes.append(Mistake(note.start, note.pitch, 'Пропущена'))
                for indices in self.active.values():
                    for i in indices:
                        if i in self.ignore_release: continue
                        n = self.notes[i]
                        self.mistakes.append(Mistake(n.start, n.pitch, 'Не отпущена'))
            self.finished = True
        return {'complete': complete, 'total': len(self.notes), 'hit': len(self.hit),
                'accuracy': round(100 * len(self.hit) / max(1, len(self.notes)), 1),
                'mistakes': [asdict(m) for m in self.mistakes]}


class LoopProgress:
    def __init__(self):
        self.streak = 0

    def next_speed(self, speed, accuracy, threshold=90, repetitions=3, step=.05, target=1.):
        self.streak = self.streak + 1 if accuracy >= threshold else 0
        if self.streak >= repetitions:
            self.streak = 0
            return min(target, speed + step) if speed < target else speed
        return speed
