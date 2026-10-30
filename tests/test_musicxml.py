# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
import pytest
from pianoflow.midi.musicxml import load_musicxml


def test_musicxml_chords_voices_tempo_ties_and_fingering(tmp_path):
    path=tmp_path/'score.musicxml'
    path.write_text('''<score-partwise><work><work-title>Study</work-title></work><part id="P1">
    <measure number="1"><attributes><divisions>2</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="120"/></direction>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="start"/><notations><technical><fingering>1</fingering></technical></notations></note>
    <note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration></note>
    <direction><sound tempo="60"/></direction>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/></note>
    <backup><duration>4</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>2</voice><staff>2</staff></note>
    </measure></part></score-partwise>''')
    song=load_musicxml(path)
    assert song.title=='Study' and len(song.notes)==3
    c=next(n for n in song.notes if n.pitch==60)
    assert c.duration==pytest.approx(1.5) and c.finger==1 and c.hand==''
    assert next(n for n in song.notes if n.pitch==48).duration==pytest.approx(1.5)
    assert song.tempo_map==((0.,120.),(.5,60.))


def test_musicxml_rejects_repeat_and_entities(tmp_path):
    path=tmp_path/'x.xml'
    path.write_text('<score-partwise><part><measure><barline><repeat direction="backward"/></barline></measure></part></score-partwise>')
    with pytest.raises(ValueError,match='repeat'): load_musicxml(path)
    path.write_text('<!DOCTYPE x [<!ENTITY a "a">]><score-partwise/>')
    with pytest.raises(ValueError,match='сущност'): load_musicxml(path)
