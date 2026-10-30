# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Standard MIDI parser with global tempo map and paired track/channel notes."""
from collections import defaultdict, deque
from pathlib import Path
import mido
from pianoflow.core.song import Note, Song

MAX_BYTES = 32 * 1024 * 1024
MAX_EVENTS = 500_000


def load_midi(path: str | Path) -> Song:
    path = Path(path)
    if path.stat().st_size > MAX_BYTES:
        raise ValueError("MIDI-файл больше 32 МБ")
    try:
        midi = mido.MidiFile(path)
        if midi.type == 2:
            raise ValueError("MIDI type 2 содержит независимые последовательности; используйте type 0 или 1")
        if midi.ticks_per_beat <= 0:
            raise ValueError("MIDI с SMPTE timing не поддерживается")
        events = []
        names = []
        for track_id, track in enumerate(midi.tracks):
            tick = 0
            names.append(track.name or f"Дорожка {track_id + 1}")
            for index, message in enumerate(track):
                tick += message.time
                events.append((tick, track_id, index, message))
                if len(events) > MAX_EVENTS:
                    raise ValueError("Слишком много MIDI-событий (лимит 500 000)")
        events.sort(key=lambda item: item[:3])
        tempo = 500_000
        initial_tempo = None
        tempo_map = [(0., 120.)]
        signature = (4, 4)
        signature_seen = False
        tick_before = 0
        seconds = 0.0
        pending = defaultdict(deque)
        notes = []
        for tick, track_id, _, message in events:
            seconds += mido.tick2second(tick - tick_before, midi.ticks_per_beat, tempo)
            tick_before = tick
            if message.type == "set_tempo":
                if message.tempo <= 0:
                    raise ValueError("Некорректный темп MIDI")
                tempo = message.tempo
                point = (seconds, mido.tempo2bpm(tempo))
                if tempo_map[-1][0] == seconds: tempo_map[-1] = point
                else: tempo_map.append(point)
                if tick == 0 and initial_tempo is None:
                    initial_tempo = tempo
            elif message.type == "time_signature" and not signature_seen:
                signature = (message.numerator, message.denominator)
                signature_seen = True
            elif message.type in ("note_on", "note_off"):
                key = (track_id, message.channel, message.note)
                if message.type == "note_on" and message.velocity > 0:
                    pending[key].append((seconds, message.velocity))
                elif pending[key]:
                    start, velocity = pending[key].popleft()
                    notes.append(Note(message.note, start, max(.01, seconds - start), velocity, track_id, message.channel))
        # Unclosed notes end at file end rather than hanging forever.
        for (track_id, channel, pitch), entries in pending.items():
            for start, velocity in entries:
                notes.append(Note(pitch, start, max(.1, seconds - start), velocity, track_id, channel))
        if not notes:
            raise ValueError("В файле нет нот")
        notes.sort(key=lambda n: (n.start, n.track, n.pitch))
        return Song(path.stem, tuple(notes), mido.tempo2bpm(initial_tempo or 500_000), signature, tuple(names), tempo_map=tuple(tempo_map))
    except (OSError, EOFError, KeyError, TypeError) as exc:
        raise ValueError(f"Не удалось прочитать MIDI: {exc}") from exc
