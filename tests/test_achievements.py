# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from datetime import datetime
from pianoflow.library.store import Library
from pianoflow.learning.achievements import achievement_state
from pianoflow.core.song import Song, Note


def report(**changes):
    return dict(hit=4,total=4,complete=True,full_song=True,both_hands=True,accuracy=100,difficulty=1) | changes


def record(store,data,day=0):
    store.save_attempt('song',data)
    stamp=datetime(2026,1,1,12).timestamp()+day*86400
    with store.db: store.db.execute('UPDATE attempts SET created=? WHERE rowid=(SELECT MAX(rowid) FROM attempts)',(stamp,))
    return stamp


def states(store):
    rows,new=achievement_state(store)
    return {r['achievement'].key:r for r in rows},new


def test_no_award_for_passive_playback_or_fragment(tmp_path):
    store=Library(tmp_path/'lib.db')
    record(store,report(hit=0,accuracy=0))
    values,new=states(store); assert not new and not any(s['earned'] for s in values.values())
    record(store,report(full_song=False),1)
    values,new=states(store)
    assert [a.key for a in new]==['first-notes']
    assert not values['first-piece']['earned'] and not values['accurate']['earned'] and not values['both-hands']['earned']
    store.close()


def test_full_piece_awards_survive_restart_and_backup(tmp_path):
    path=tmp_path/'lib.db'; store=Library(path)
    stamp=record(store,report())
    values,new=states(store)
    assert {a.key for a in new}=={'first-notes','first-piece','accurate','both-hands'}
    assert values['accurate']['earned']==stamp
    record(store,report(accuracy=50),1)
    assert not states(store)[1]
    store.close(); store=Library(path)
    assert states(store)[0]['accurate']['earned']==stamp
    assert not states(store)[1]
    archive=tmp_path/'backup.zip';store.backup(archive)
    restored=Library(tmp_path/'restore.db'); restored.restore(restored.read_backup(archive))
    assert states(restored)[0]['accurate']['earned']==stamp
    store.close();restored.close()


def test_streak_counts_distinct_consecutive_days_and_all_levels(tmp_path):
    store=Library(tmp_path/'lib.db')
    for day in (0,0,1,3,4,5,6,7,8): record(store,report(difficulty=day%5+1),day)
    values,_=states(store); assert values['seven-days']['progress']==6 and not values['seven-days']['earned']
    record(store,report(),9)
    values,new=states(store);assert values['seven-days']['earned'] and values['explorer']['earned']
    assert 'seven-days' in {a.key for a in new}
    store.close()


def test_bad_achievement_backup_is_rejected(tmp_path):
    import pytest
    from pianoflow.library.store import validate_preference,validate_report
    for value in ({'first-notes':'tomorrow'},{'first-notes':float('inf')},{'first-notes':True}):
        with pytest.raises(ValueError): validate_preference('achievements',value)
    for value in ({'hit':'one'},{'full_song':1},{'difficulty':99}):
        with pytest.raises(ValueError): validate_report(value)
