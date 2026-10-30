# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from dataclasses import replace
import json
import zipfile
import pytest
from pianoflow.core.song import Note, Song
from pianoflow.core.state import MidiEvent
from pianoflow.learning.engine import Performance, LoopProgress
from pianoflow.learning.recording import Recorder
from pianoflow.learning.course import course_songs, birthday_variants
from pianoflow.library.store import Library, encode, decode
from pianoflow.midi.loader import load_midi


def test_performance_timing_duration_and_missing():
    p=Performance([Note(60,0,1),Note(62,2,.5),Note(64,3,.5)])
    p.feed(MidiEvent('note_on',60,90),.25)
    p.feed(MidiEvent('note_off',60),.5)
    p.feed(MidiEvent('note_on',66,90),2)
    report=p.finish()
    assert report['hit']==1
    assert {m['kind'] for m in report['mistakes']}=={'Поздно','Короткое удержание','Лишняя нота','Пропущена'}
    assert p.finish()==report


def test_speed_and_latency_are_wall_time_adjusted():
    p=Performance([Note(60,1,.5)],speed=.5,tolerance=.1,latency=.1)
    p.feed(MidiEvent('note_on',60,90),1.05)
    p.feed(MidiEvent('note_off',60),1.55)
    assert not p.finish()['mistakes']
    p=Performance([Note(60,0,1)])
    assert p.finish(False)['mistakes']==[]


def test_library_roundtrip_backup_merge_and_invalid_archive(tmp_path):
    a=Library(tmp_path/'a.db'); b=Library(tmp_path/'b.db')
    song=birthday_variants()[0][1]
    identifier=a.add(song); assert a.add(song)==identifier
    a.favorite(identifier,True); a.set_preference('passed:'+identifier,True)
    a.save_attempt(identifier,{'complete':True,'accuracy':100})
    archive=tmp_path/'backup.zip'; a.backup(archive)
    b.restore(b.read_backup(archive)); b.restore(b.read_backup(archive))
    assert b.get(identifier)==song
    assert len(b.songs())==1 and len(b.attempts())==1
    assert b.songs()[0][2] and b.preference('passed:'+identifier)
    a.delete(identifier); assert not a.get(identifier); assert a.attempts()
    with zipfile.ZipFile(tmp_path/'bad.zip','w') as z: z.writestr('../escape','oops')
    with pytest.raises(ValueError): b.read_backup(tmp_path/'bad.zip')
    assert b.get(identifier)==song
    data=b.read_backup(archive); data['songs'].append({'id':'broken','score':'{}'})
    with pytest.raises(ValueError): b.restore(data)
    assert len(b.songs())==1
    a.close(); b.close()


def test_recording_preserves_pedal_and_closes_held_notes(tmp_path):
    clock=[10.]; r=Recorder(lambda:clock[0]); r.start()
    r.feed(MidiEvent('note_on',60,99,channel=2)); r.feed(MidiEvent('control_change',channel=2,control=64,value=127))
    clock[0]=11.; r.stop(); path=tmp_path/'recording.mid'; r.save(path)
    song=load_midi(path)
    assert song.notes[0].duration==pytest.approx(1)
    assert song.notes[0].channel==2 and song.notes[0].velocity==99
    import mido
    events=list(mido.MidiFile(path))
    assert any(m.type=='control_change' and m.control==64 and m.value==0 for m in events)


def test_course_and_loop_progress():
    for identifier,song in course_songs()+birthday_variants():
        assert decode(encode(song))==song
        assert all(n.finger in range(1,6) and n.hand in ('L','R') for n in song.notes)
    progress=LoopProgress()
    assert progress.next_speed(.5,100)==.5
    assert progress.next_speed(.5,100)==.5
    assert progress.next_speed(.5,100)==pytest.approx(.55)
    assert progress.next_speed(1.5,100,repetitions=1,target=1)==1.5


def test_backup_rejects_corrupt_preferences_without_changes(tmp_path):
    library=Library(tmp_path/'library.db'); song=course_songs()[0][1]; identifier=library.add(song)
    archive=tmp_path/'copy.zip'; library.backup(archive)
    with zipfile.ZipFile(archive) as z: data=json.loads(z.read('pianoflow.json'))
    data['preferences']=[{'key':'hints','value':'"invalid"'}]
    with zipfile.ZipFile(archive,'w') as z: z.writestr('pianoflow.json',json.dumps(data))
    with pytest.raises(ValueError,match='настройки'): library.read_backup(archive)
    assert library.get(identifier)==song
    library.close()


def test_loop_clipped_tail_is_not_reported_as_unreleased():
    p=Performance([Note(60,1,2)],start=1,end=2)
    p.feed(MidiEvent('note_on',60,90),1)
    assert p.finish()['mistakes']==[]


def test_corrupt_note_data_is_rejected():
    raw=json.loads(encode(course_songs()[0][1])); raw['notes'][0]['channel']=.5
    with pytest.raises(ValueError): decode(raw)


def test_restore_can_replace_seeded_arrangement_edits(tmp_path):
    a=Library(tmp_path/'a.db'); b=Library(tmp_path/'b.db')
    identifier,song=course_songs()[0]
    a.add(replace(song,title='Моя разметка'),identifier); b.add(song,identifier)
    path=tmp_path/'copy.zip'; a.backup(path); data=b.read_backup(path)
    b.restore(data); assert b.get(identifier).title==song.title
    b.restore(data,overwrite=True); assert b.get(identifier).title=='Моя разметка'
    a.close(); b.close()


def test_long_score_matching_with_repeated_pitches():
    notes=[Note(60+i%12,i*.1,.07) for i in range(3000)]
    performance=Performance(notes,tolerance=.03)
    for note in notes:
        performance.feed(MidiEvent('note_on',note.pitch,90),note.start)
        performance.feed(MidiEvent('note_off',note.pitch),note.end)
    report=performance.finish()
    assert report['hit']==3000 and report['accuracy']==100
    assert not report['mistakes']
