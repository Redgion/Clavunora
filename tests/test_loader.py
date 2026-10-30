# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import mido
import pytest
from pianoflow.midi.loader import load_midi


def test_global_tempo_map_channels_chord_and_velocity_zero(tmp_path):
    midi = mido.MidiFile(ticks_per_beat=480)
    tempo = mido.MidiTrack([
        mido.MetaMessage("set_tempo", tempo=500_000),
        mido.MetaMessage("time_signature", numerator=3, denominator=4),
        mido.MetaMessage("set_tempo", tempo=1_000_000, time=480),
    ])
    notes = mido.MidiTrack([
        mido.Message("note_on", note=60, velocity=90, channel=0),
        mido.Message("note_on", note=64, velocity=80, channel=1),
        mido.Message("note_off", note=60, channel=0, time=960),
        mido.Message("note_on", note=64, velocity=0, channel=1),
    ])
    midi.tracks.extend([tempo, notes])
    path = tmp_path / "tempo.mid"
    midi.save(path)
    song = load_midi(path)
    assert len(song.notes) == 2
    assert song.notes[0].duration == pytest.approx(1.5)
    assert song.notes[1].duration == pytest.approx(1.5)
    assert song.notes[0].start == song.notes[1].start == 0
    assert song.notes[1].channel == 1
    assert song.time_signature == (3, 4)
    assert song.bpm == 120


def test_overlapping_notes_fifo_and_unclosed(tmp_path):
    midi = mido.MidiFile()
    midi.tracks.append(mido.MidiTrack([
        mido.Message("note_on", note=60, velocity=70),
        mido.Message("note_on", note=60, velocity=90, time=240),
        mido.Message("note_off", note=60, time=240),
        mido.MetaMessage("end_of_track", time=480),
    ]))
    path = tmp_path / "overlap.mid"
    midi.save(path)
    song = load_midi(path)
    assert len(song.notes) == 2
    assert song.notes[0].duration == .5
    assert song.notes[1].start == .25
    assert song.notes[1].duration == .75


def test_corrupt_empty_and_type2(tmp_path):
    path = tmp_path / "bad.mid"
    path.write_bytes(b"not midi")
    with pytest.raises(ValueError):
        load_midi(path)
    midi = mido.MidiFile()
    midi.tracks.append(mido.MidiTrack())
    midi.save(path)
    with pytest.raises(ValueError, match="нет нот"):
        load_midi(path)
    midi.type = 2
    midi.save(path)
    with pytest.raises(ValueError, match="type 2"):
        load_midi(path)
