# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Cross-language contract: Rust imports a live Python/WAL library without changing it."""
from dataclasses import replace
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from pianoflow.library.store import Library,validate_report,decode,encode
from pianoflow.library.catalog import install_catalog
from pianoflow.learning.course import course_songs, birthday_variants
from pianoflow.core.song import Note, Song


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    binary = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT/'build/rust-target/debug/clavunora-rust'
    if not binary.is_file():
        raise SystemExit('Build the Rust application first: scripts/cargo-rust.sh build -p clavunora-rust')
    # The native self-test creates only temporary data and exports its saved reports.
    probe=subprocess.run([str(binary),'--self-test'],text=True,capture_output=True,check=True,timeout=30)
    native=json.loads(probe.stdout.splitlines()[-1])
    from pianoflow.midi.loader import load_midi
    reference_midi=load_midi(ROOT/'assets/happy_birthday.mid')
    imported_midi=decode(native['midi_import_score'])
    assert len(imported_midi.notes)==len(reference_midi.notes)
    assert imported_midi.track_names==reference_midi.track_names
    assert imported_midi.time_signature==reference_midi.time_signature
    assert abs(imported_midi.bpm-reference_midi.bpm)<1e-9
    for actual,expected in zip(imported_midi.notes,reference_midi.notes):
        for key in ('pitch','velocity','track','channel','hand','finger'): assert getattr(actual,key)==getattr(expected,key)
        for key in ('start','duration'): assert abs(getattr(actual,key)-getattr(expected,key))<1e-9
    assert len(imported_midi.tempo_map)==len(reference_midi.tempo_map)
    for actual,expected in zip(imported_midi.tempo_map,reference_midi.tempo_map): assert all(abs(a-b)<1e-9 for a,b in zip(actual,expected))
    from pianoflow.midi.musicxml import load_musicxml
    reference_xml=load_musicxml(ROOT/'tests/fixtures/musicxml-import.musicxml')
    imported_xml=decode(native['musicxml_import_score'])
    assert imported_xml.title==reference_xml.title and imported_xml.composer==reference_xml.composer
    assert imported_xml.track_names==reference_xml.track_names and imported_xml.time_signature==reference_xml.time_signature
    assert len(imported_xml.notes)==len(reference_xml.notes)
    for actual,expected in zip(imported_xml.notes,reference_xml.notes):
        for key in ('pitch','velocity','track','channel','hand','finger'): assert getattr(actual,key)==getattr(expected,key)
        for key in ('start','duration'): assert abs(getattr(actual,key)-getattr(expected,key))<1e-8
    assert imported_xml.tempo_map==reference_xml.tempo_map
    song=decode(native['scoring_score'])
    for key in ('scoring_report','rhythm_report'):
        validate_report(native[key])
        assert native[key]['score_version']==hashlib.sha256(encode(song).encode()).hexdigest()
    lesson_song=decode(native['lesson_score'])
    validate_report(native['lesson_report'])
    assert native['lesson_report']['hand']=='L' and native['lesson_report']['full_song'] is False
    assert native['lesson_report']['score_version']==hashlib.sha256(encode(lesson_song).encode()).hexdigest()
    from pianoflow.learning.engine import Performance
    from pianoflow.core.state import MidiEvent
    performance=Performance(song.notes,end=song.duration)
    for note in song.notes[:2]:
        performance.feed(MidiEvent('note_on',note.pitch,90),note.start+.25)
        performance.feed(MidiEvent('note_off',note.pitch),note.end-.4)
    expected=performance.finish(True)
    actual={key:native['rhythm_report'][key] for key in expected}
    assert actual==expected,(actual,expected)
    with tempfile.TemporaryDirectory(prefix='clavunora-python-rust-') as temp:
        folder = Path(temp)
        consumer=Library(folder/'python-consumer.db')
        native_id=consumer.add(song,'rust-scored-song')
        consumer.save_attempt(native_id,native['rhythm_report'])
        lesson_id=consumer.add(lesson_song,'rust-selected-lesson')
        consumer.save_attempt(lesson_id,native['lesson_report'])
        backup=folder/'native-reports.zip';consumer.backup(backup)
        restored=Library(folder/'python-restored.db');restored.restore(restored.read_backup(backup))
        assert any(a['report']==native['rhythm_report'] for a in restored.attempts())
        assert any(a['report']==native['lesson_report'] for a in restored.attempts())
        consumer.close();restored.close()
        config = folder/'config'
        source = config/'PianoFlow/library.sqlite3'
        python = Library(source)
        python.db.execute('PRAGMA journal_mode=WAL')
        for identifier,song in course_songs()+birthday_variants():
            python.add(song,identifier)
        python.set_preference('seeded',True)
        install_catalog(python)
        python.update('course-1',replace(python.get('course-1'),title='Моя редакция · До',description='Сохранить пользовательскую разметку'))
        python.favorite('course-1',True)
        for identifier in ('course-2','birthday-melody','repertoire-002'):
            python.delete(identifier)
        python.add(Song('Своя пьеса', (Note(61,0,.7,hand='R',finger=2),),composer='Пользователь',difficulty=3), 'custom-test')
        python.save_attempt('course-1',{'title':'Моя редакция · До','mode':'Ожидание','complete':True,'hit':4,'total':4,'accuracy':100,'full_song':True,'both_hands':False,'difficulty':1})
        python.set_preference('achievements',{'first-notes':1234567890})
        python.save_recording('Моя запись',(ROOT/'assets/happy_birthday.mid').read_bytes())
        count=len(python.songs())
        original=digest(source)
        wal=source.with_name(source.name+'-wal')
        original_wal=digest(wal)
        destination=folder/'Rust-копия с пробелами'
        env=dict(os.environ,XDG_CONFIG_HOME=str(config))
        def run(*args):
            return subprocess.run([str(binary),*map(str,args)],cwd=folder,env=env,text=True,capture_output=True,timeout=30)
        result=run('--data-dir',destination,'--import-legacy',source,'--inspect')
        assert result.returncode==0,(result.stdout,result.stderr)
        report=json.loads(result.stdout.splitlines()[-1])
        assert report['songs']==count+18 and report['attempts']==1 and report['recordings']==1 and report['favorites']==1,report
        assert report['achievements']['first-notes']==1234567890
        assert digest(source)==original and digest(wal)==original_wal,'Source or WAL was modified'
        copy=Library(destination/'library.sqlite3')
        from pianoflow.learning.achievements import achievement_state
        achievement_state(copy)
        assert all(report['achievements'].get(k)==v for k,v in copy.preference('achievements').items()), 'Legacy Rust awards differ from Python'
        assert copy.get('course-1')==python.get('course-1')
        assert copy.attempts()==python.attempts()
        assert copy.recordings()[0]['data']==python.recordings()[0]['data']
        for identifier in ('course-2','birthday-melody','repertoire-002'):
            assert copy.get(identifier) is None,'Deleted built-in was resurrected'
        copy.favorite('course-1',False)
        copy.close()
        repeated=run('--data-dir',destination,'--import-legacy',source,'--inspect')
        assert repeated.returncode!=0,'Existing copy overwritten'
        protected=run('--data-dir',source.parent,'--inspect')
        assert protected.returncode!=0,'Python data directory was not protected'
        assert digest(source)==original and digest(wal)==original_wal
        # All six awards: duplicate dates, a gap, silent input, all difficulty levels,
        # fragments, and preserving an existing first-earned date.
        from datetime import datetime, timedelta
        awards=Library(folder/'awards-source.db')
        awards.add(Song('Awards', (Note(60,0,1,hand='R'),Note(48,0,1,hand='L')),difficulty=1),'award-song')
        awards.set_preference('achievements',{'first-notes':1234567890})
        start=datetime(2026,1,1,12)
        for i in range(9):
            for duplicate in range(2):
                awards.save_attempt('award-song',{'title':'Awards','mode':'Ожидание','complete':True,'hit':0 if i==1 else 2,'total':2,'accuracy':0 if i==1 else 100,'full_song':i!=0,'both_hands':True,'difficulty':i%5+1})
                # The Python store assigns time; set explicit calendar fixtures for this test only.
                last=awards.db.execute('SELECT id FROM attempts ORDER BY created DESC LIMIT 1').fetchone()[0]
                awards.db.execute('UPDATE attempts SET created=? WHERE id=?',((start+timedelta(days=i,seconds=duplicate)).timestamp(),last))
                awards.db.commit()
        achievement_state(awards)
        expected=awards.preference('achievements')
        assert len(expected)==6,expected
        awards.set_preference('achievements',{'first-notes':1234567890})
        all_awards=run('--data-dir',folder/'awards-rust','--import-legacy',folder/'awards-source.db','--inspect')
        assert all_awards.returncode==0,(all_awards.stdout,all_awards.stderr)
        assert all(json.loads(all_awards.stdout.splitlines()[-1])['achievements'].get(k)==v for k,v in expected.items())
        awards.close()
        python.close()
        print(json.dumps({'ok':True,'songs':count,'python_wal_source_unchanged':True,'custom_edits':True,'deletions':True,'attempts':True,'recordings':True,'achievements':True,'refuses_overwrite':True,'native_scoring_matches_python':True,'native_report_restores_in_python':True,'all_six_awards_match_python':True,'selected_lesson_restores_in_python':True,'midi_import_matches_python':True,'musicxml_import_matches_python':True},ensure_ascii=False))


if __name__=='__main__':
    main()
