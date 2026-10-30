# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from dataclasses import replace
from collections import Counter
from pianoflow.library.catalog import manifest, bundled_songs, install_catalog
from pianoflow.library.store import Library
from pianoflow.learning.engine import WaitPractice
from pianoflow.core.platform import settings_directory


def test_catalog_unique_sources_and_valid_scores():
    entries = manifest()['entries']
    assert len(entries) == 100
    assert len({e['id'] for e in entries}) == 100
    assert Counter(e['difficulty'] for e in entries) == {i:20 for i in range(1,6)}
    assert all(e.get('score') or e['status'] in ('existing', 'rights-pending') for e in entries)
    scores = dict(bundled_songs())
    for entry in entries:
        if 'score' not in entry: continue
        song = scores[entry['id']]
        assert song.notes and song.duration > 0 and song.source
        assert song.difficulty == entry['difficulty']
        assert all(n.duration > 0 and n.start >= 0 for n in song.notes)
        assert all(n.hand for n in song.notes if n.finger)


def test_final_five_editions_are_complete_and_honest_about_annotation():
    songs = dict(bundled_songs())
    for number, minimum_notes, minimum_duration in ((95,2500,300),(97,2500,200),(98,4500,300)):
        song = songs[f'repertoire-{number:03}']
        assert len(song.notes) >= minimum_notes
        assert song.duration >= minimum_duration
        assert not any(n.finger and not n.hand for n in song.notes)
    for number in (97,98):
        assert {n.hand for n in songs[f'repertoire-{number:03}'].notes} == {'L','R'}
    for number in (95,):
        assert not any(n.hand or n.finger for n in songs[f'repertoire-{number:03}'].notes)
    assert 'repertoire-048' not in songs and 'repertoire-076' not in songs
    assert len(songs) == 97  # Birthday is seeded separately.


def test_catalog_upgrade_preserves_edits_deletions_and_results(tmp_path):
    library = Library(tmp_path/'library.db')
    library.set_preference('seeded', True)  # An existing 0.3 user.
    install_catalog(library)
    identifier, song = next(iter(bundled_songs()))
    library.update(identifier, replace(song, title='Моя редакция'))
    library.favorite(identifier, True)
    library.save_attempt(identifier, {'accuracy':90})
    other = list(bundled_songs())[1][0]
    library.delete(other)
    count = len(library.songs())
    install_catalog(library)
    assert len(library.songs()) == count
    assert library.get(identifier).title == 'Моя редакция'
    assert library.get(other) is None
    assert len(library.attempts()) == 1
    assert next(f for i,s,f in library.songs() if i == identifier)
    library.close()


def test_windows_settings_path(monkeypatch, tmp_path):
    monkeypatch.setattr('sys.platform', 'win32')
    monkeypatch.setenv('APPDATA', str(tmp_path))
    assert settings_directory() == tmp_path/'PianoFlow'


def test_keyboard_touch_chord_release_on_hide(app):
    from pianoflow.ui.keyboard import PianoKeyboard
    keyboard=PianoKeyboard(); pressed=[]; released=[]
    keyboard.note_pressed.connect(pressed.append)
    keyboard.note_released.connect(released.append)
    keyboard.show(); app.processEvents()
    keyboard.update_touch_notes({60,64,67})
    keyboard.update_touch_notes({60,64,67})
    assert sorted(pressed)==[60,64,67]
    keyboard.update_touch_notes({60,65})
    assert sorted(released)==[64,67]
    keyboard.hide(); app.processEvents()
    assert sorted(released)==[60,64,65,67]
    assert not keyboard.touch_notes


def test_all_bundled_scores_can_complete_wait_groups():
    from pianoflow.core.state import MidiEvent
    for identifier, song in bundled_songs():
        engine = WaitPractice(song.notes)
        for _, pitches in engine.groups:
            pressed = set()
            for pitch in sorted(pitches):
                pressed.add(pitch)
                engine.accept(MidiEvent('note_on', pitch, 90), pressed)
        assert engine.completed == len(engine.groups), identifier


def test_unison_across_staves_requires_one_physical_press():
    from pianoflow.core.song import Note
    from pianoflow.core.state import MidiEvent
    from pianoflow.learning.engine import Performance
    score = [Note(60,0,.5,hand='R'), Note(60,0,1,track=1,hand='L')]
    performance = Performance(score)
    performance.feed(MidiEvent('note_on',60,90),0)
    performance.feed(MidiEvent('note_off',60),1)
    report = performance.finish()
    assert report['accuracy'] == 100
    assert report['total'] == 1
    assert not report['mistakes']
