# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from pianoflow.core.state import MidiEvent, PianoState
from pianoflow.core.notes import chord_name, note_name


def test_chord_and_velocity_zero():
    state = PianoState()
    for note in (60, 64, 67):
        state.apply(MidiEvent("note_on", note, 100))
    assert state.pressed == {60, 64, 67}
    assert chord_name(state.pressed) == "C"
    state.apply(MidiEvent("note_on", 64, 0))
    assert state.pressed == {60, 67}
    assert state.note_count == 3


def test_sustain_and_channel_isolation():
    state = PianoState()
    state.apply(MidiEvent("control_change", control=64, value=127))
    state.apply(MidiEvent("note_on", 60, 80))
    state.apply(MidiEvent("note_on", 60, 90, channel=1))
    state.apply(MidiEvent("note_off", 60))
    assert state.pressed == {60}
    state.apply(MidiEvent("note_off", 60, channel=1))
    assert not state.pressed
    assert state.sounding == {60}
    state.apply(MidiEvent("control_change", control=64, value=0, channel=1))
    assert state.sounding == {60}
    state.apply(MidiEvent("control_change", control=64, value=0))
    assert not state.sounding


def test_disconnect_does_not_clear_other_sources():
    state = PianoState()
    for source in ("midi", "computer"):
        state.apply(MidiEvent("note_on", 60, 90, source=source))
    state.clear_source("midi")
    assert state.pressed == {60}
    state.panic()
    assert not state.sounding and not state.pedals


def test_controller_reset():
    state = PianoState()
    state.apply(MidiEvent("note_on", 60, 90))
    state.apply(MidiEvent("control_change", control=123))
    assert not state.sounding


def test_chords_and_range():
    assert note_name(21) == "A0"
    assert note_name(108) == "C8"
    assert chord_name({60, 63, 67}) == "Cm"
    assert chord_name({64, 67, 72}) == "C/E"
    assert chord_name({60, 64, 67, 71}) == "Cmaj7"
    assert chord_name(set()) == "Сыграйте ноту"
