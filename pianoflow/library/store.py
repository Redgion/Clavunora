# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""SQLite records contain portable scores, never user-owned source paths."""
import base64
from dataclasses import asdict
import hashlib
import json
import math
from pathlib import Path
import sqlite3
import time
import tempfile
import uuid
import zipfile
from pianoflow.core.song import Note, Song


def encode(song):
    return json.dumps(asdict(song), ensure_ascii=False, allow_nan=False)


def decode(raw):
    data = json.loads(raw) if isinstance(raw, str) else raw
    if not isinstance(data, dict) or not isinstance(data.get('notes'), list) or not 1 <= len(data['notes']) <= 100000:
        raise ValueError('Некорректная композиция')
    notes = []
    for item in data['notes']:
        note = Note(**item)
        if (type(note.pitch) is not int or not 0 <= note.pitch <= 127 or
            not all(isinstance(x, (int, float)) and math.isfinite(x) for x in (note.start, note.duration)) or
            note.start < 0 or note.duration <= 0 or
            type(note.velocity) is not int or not 0 <= note.velocity <= 127 or
            type(note.channel) is not int or not 0 <= note.channel <= 15 or
            type(note.track) is not int or not 0 <= note.track <= 10000 or
            type(note.finger) is not int or note.hand not in ('', 'L', 'R') or note.finger not in range(6)):
            raise ValueError('Некорректные данные ноты')
        notes.append(note)
    data['notes'] = tuple(notes)
    data['track_names'] = tuple(data.get('track_names', ['Мелодия']))
    data['time_signature'] = tuple(data.get('time_signature', [4, 4]))
    data['tempo_map'] = tuple(tuple(p) for p in data.get('tempo_map', []))
    song = Song(**data)
    if not isinstance(song.title, str) or not song.title.strip() or len(song.title) > 500 or type(song.difficulty) is not int or not 0 <= song.difficulty <= 5:
        raise ValueError('Некорректное название или сложность')
    if any(not isinstance(getattr(song,k), str) or len(getattr(song,k)) > 100000 for k in ('composer','work','description','source')):
        raise ValueError('Некорректные метаданные')
    if not song.track_names or any(not isinstance(t,str) or len(t)>1000 for t in song.track_names):
        raise ValueError('Некорректные дорожки')
    if not isinstance(song.bpm, (int, float)) or not math.isfinite(song.bpm) or song.bpm <= 0:
        raise ValueError('Некорректный темп')
    if len(song.time_signature) != 2 or any(type(v) is not int or v <= 0 for v in song.time_signature):
        raise ValueError('Некорректный размер')
    for t, bpm in song.tempo_map:
        if not math.isfinite(t) or not math.isfinite(bpm) or t < 0 or bpm <= 0:
            raise ValueError('Некорректная карта темпа')
    return song


def validate_preference(key, value):
    def numeric(v, low, high):
        return type(v) in (int,float) and math.isfinite(v) and low <= v <= high
    valid = True
    if key == 'hints': valid = type(value) is int and value in range(3)
    elif key == 'seeded' or key.startswith('passed:'): valid = type(value) is bool
    elif key.startswith('loop:'):
        valid = isinstance(value,dict) and numeric(value.get('start'),0,1e9) and numeric(value.get('end'),0,1e9) and value['start']<value['end'] and type(value.get('enabled')) is bool
    elif key == 'practice':
        valid = isinstance(value,dict) and numeric(value.get('tolerance'),.03,1) and numeric(value.get('latency'),-.5,.5) and type(value.get('metronome')) is bool and type(value.get('accompany')) is bool
    elif key == 'audio':
        valid = isinstance(value,dict) and value.get('engine') in ('preview','fluid') and isinstance(value.get('soundfont'),str) and numeric(value.get('volume'),0,1)
    elif key == 'achievements':
        valid = isinstance(value,dict) and all(isinstance(k,str) and numeric(v,1,32503680000) for k,v in value.items())
    elif key == 'app_settings': valid = isinstance(value,dict)
    if not valid: raise ValueError('Некорректные настройки: '+key)


def validate_report(report):
    if not isinstance(report,dict): raise ValueError('Некорректный результат')
    for key in ('accuracy','speed','start','end','tolerance','latency'):
        if key in report and (type(report[key]) not in (int,float) or not math.isfinite(report[key])): raise ValueError('Некорректное число в результате')
    if not 0<=report.get('accuracy',0)<=100: raise ValueError('Некорректная точность')
    for key in ('hit','total'):
        if key in report and (type(report[key]) is not int or report[key]<0): raise ValueError('Некорректное количество нот')
    for key in ('complete','full_song','both_hands'):
        if key in report and type(report[key]) is not bool: raise ValueError('Некорректный статус результата')
    if 'difficulty' in report and (type(report['difficulty']) is not int or report['difficulty'] not in range(6)): raise ValueError('Некорректная сложность результата')
    if any(not isinstance(report.get(key,''),str) for key in ('title','mode')): raise ValueError('Некорректное описание результата')
    mistakes=report.get('mistakes',[])
    if not isinstance(mistakes,list): raise ValueError('Некорректный список ошибок')
    for m in mistakes:
        if not isinstance(m,dict) or type(m.get('pitch')) is not int or not 0<=m['pitch']<=127 or not isinstance(m.get('kind'),str): raise ValueError('Некорректная ошибка')
        for key in ('position','delta'):
            v=m.get(key,0)
            if type(v) not in (int,float) or not math.isfinite(v): raise ValueError('Некорректное время ошибки')


class Library:
    def __init__(self, path):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(self.path)
        self.db.row_factory = sqlite3.Row
        version = self.db.execute("PRAGMA user_version").fetchone()[0]
        if version > 1:
            self.db.close()
            raise ValueError("Эта библиотека создана более новой версией Clavunora")
        self.db.executescript('''
        CREATE TABLE IF NOT EXISTS songs(id TEXT PRIMARY KEY, score TEXT NOT NULL,
            favorite INTEGER NOT NULL DEFAULT 0, created REAL NOT NULL, digest TEXT);
        CREATE TABLE IF NOT EXISTS attempts(id TEXT PRIMARY KEY, song_id TEXT,
            created REAL, report TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS preferences(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS recordings(id TEXT PRIMARY KEY, name TEXT, created REAL, data BLOB);
        PRAGMA user_version=1;
        ''')

    def add(self, song, identifier=None):
        raw = encode(song)
        decode(raw)
        digest = hashlib.sha256(raw.encode()).hexdigest()
        existing = self.db.execute('SELECT id FROM songs WHERE digest=?', (digest,)).fetchone()
        if existing:
            return existing['id']
        identifier = identifier or uuid.uuid4().hex
        with self.db:
            self.db.execute('INSERT OR IGNORE INTO songs VALUES(?,?,0,?,?)', (identifier, raw, time.time(), digest))
        return identifier

    def update(self, identifier, song):
        raw = encode(song)
        decode(raw)
        with self.db:
            self.db.execute('UPDATE songs SET score=?,digest=? WHERE id=?', (raw, hashlib.sha256(raw.encode()).hexdigest(), identifier))

    def songs(self):
        return [(r['id'], decode(r['score']), bool(r['favorite'])) for r in self.db.execute('SELECT * FROM songs ORDER BY created')]

    def get(self, identifier):
        row = self.db.execute('SELECT score FROM songs WHERE id=?', (identifier,)).fetchone()
        return decode(row['score']) if row else None

    def favorite(self, identifier, value):
        with self.db:
            self.db.execute('UPDATE songs SET favorite=? WHERE id=?', (int(value), identifier))

    def delete(self, identifier):
        with self.db:
            self.db.execute('DELETE FROM songs WHERE id=?', (identifier,))

    def preference(self, key, default=None):
        r = self.db.execute('SELECT value FROM preferences WHERE key=?', (key,)).fetchone()
        return json.loads(r[0]) if r else default

    def set_preference(self, key, value):
        with self.db:
            self.db.execute('INSERT OR REPLACE INTO preferences VALUES(?,?)', (key, json.dumps(value, ensure_ascii=False)))

    def save_attempt(self, identifier, report):
        validate_report(report)
        with self.db:
            self.db.execute('INSERT INTO attempts VALUES(?,?,?,?)', (uuid.uuid4().hex, identifier, time.time(), json.dumps(report, ensure_ascii=False)))

    def attempts(self):
        return [dict(r) | {'report': json.loads(r['report'])} for r in self.db.execute('SELECT * FROM attempts ORDER BY created DESC')]

    def save_recording(self, name, data):
        with self.db:
            self.db.execute('INSERT INTO recordings VALUES(?,?,?,?)', (uuid.uuid4().hex, name, time.time(), data))

    def recordings(self):
        return list(self.db.execute('SELECT * FROM recordings ORDER BY created DESC'))

    def backup(self, path):
        data = {'version': 1}
        for table in ('songs', 'attempts', 'preferences', 'recordings'):
            data[table] = [dict(r) for r in self.db.execute('SELECT * FROM ' + table)]
        for row in data['recordings']:
            row['data'] = base64.b64encode(row['data']).decode()
        destination = Path(path)
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix='.pianoflow-', delete=False) as temp:
            temporary = Path(temp.name)
        try:
            with zipfile.ZipFile(temporary, 'w', zipfile.ZIP_DEFLATED) as archive:
                archive.writestr('pianoflow.json', json.dumps(data, ensure_ascii=False, allow_nan=False))
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)

    def read_backup(self, path):
        with zipfile.ZipFile(path) as archive:
            if archive.namelist() != ['pianoflow.json'] or archive.getinfo('pianoflow.json').file_size > 128 * 1024 * 1024:
                raise ValueError('Неподдерживаемый архив или размер больше 128 МБ')
            data = json.loads(archive.read('pianoflow.json'))
        if data.get('version') != 1:
            raise ValueError('Неподдерживаемая версия архива')
        for key in ('songs', 'attempts', 'preferences', 'recordings'):
            if not isinstance(data.get(key), list):
                raise ValueError('Неполный архив')
        for table in ('songs','attempts','recordings'):
            for row in data[table]:
                if not isinstance(row.get('id'),str) or not row['id'] or len(row['id'])>200: raise ValueError('Некорректный ID')
                if type(row.get('created')) not in (int,float) or not math.isfinite(row['created']) or not 0<=row['created']<=32503680000: raise ValueError('Некорректная дата')
        for row in data['songs']:
            decode(row['score'])
        for row in data['attempts']:
            validate_report(json.loads(row['report']))
        for row in data['preferences']:
            validate_preference(row['key'],json.loads(row['value']))
        for row in data['recordings']:
            row['data'] = base64.b64decode(row['data'], validate=True)
            import io
            import mido
            mido.MidiFile(file=io.BytesIO(row['data']))
        return data

    def restore(self, data, overwrite=False):
        # User-selected conflict policy; the whole restore rolls back on failure.
        fields = {'songs': ('id','score','favorite','created','digest'),
                  'attempts': ('id','song_id','created','report'),
                  'preferences': ('key','value'), 'recordings': ('id','name','created','data')}
        with self.db:
            for table, columns in fields.items():
                for row in data[table]:
                    if table == 'songs':
                        decode(row['score'])
                    policy = "REPLACE" if overwrite else "IGNORE"
                    self.db.execute(f"INSERT OR {policy} INTO {table} VALUES({','.join('?' for _ in columns)})", tuple(row[c] for c in columns))

    def close(self):
        self.db.close()
