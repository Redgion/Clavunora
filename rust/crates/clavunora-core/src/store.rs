// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use crate::{catalog, score::Song, session::Report, version::python_hash};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Library {
    db: Connection,
}
#[derive(Clone, Debug)]
pub struct Record {
    pub id: String,
    pub song: Song,
    pub favorite: bool,
    pub score_version: String,
}
#[derive(Clone, Debug)]
pub struct Attempt {
    pub id: String,
    pub date: String,
    pub song_id: Option<String>,
    pub created: f64,
    pub report: Value,
}
/// A stable ID and timestamp survive retries, so one finished attempt is stored once.
#[derive(Clone, Debug)]
pub struct PendingAttempt {
    pub id: String,
    pub song_id: String,
    pub created: f64,
    pub report: Report,
}
impl PendingAttempt {
    pub fn new(song_id: String, report: Report) -> Result<Self> {
        report.validate()?;
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let id = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Ok(Self {
            id,
            song_id,
            created: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64(),
            report,
        })
    }
}
impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(3))?;
        let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(
            version <= 1,
            "Библиотека создана более новой версией приложения"
        );
        db.execute_batch("CREATE TABLE IF NOT EXISTS songs(id TEXT PRIMARY KEY,score TEXT NOT NULL,favorite INTEGER NOT NULL DEFAULT 0,created REAL NOT NULL,digest TEXT);CREATE TABLE IF NOT EXISTS attempts(id TEXT PRIMARY KEY,song_id TEXT,created REAL,report TEXT NOT NULL);CREATE TABLE IF NOT EXISTS preferences(key TEXT PRIMARY KEY,value TEXT NOT NULL);CREATE TABLE IF NOT EXISTS recordings(id TEXT PRIMARY KEY,name TEXT,created REAL,data BLOB);PRAGMA user_version=1;")?;
        Ok(Self { db })
    }
    /// The caller must provide the preview's separate data path. Never call this on the legacy database.
    pub fn seed(&mut self) -> Result<()> {
        let songs = catalog::bundled()?;
        let tx = self.db.transaction()?;
        let created = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
        let seeded = tx
            .query_row(
                "SELECT value FROM preferences WHERE key='seeded'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .is_some_and(|raw| raw == "true");
        for (id, song) in songs {
            let exercise = id.starts_with("birthday-")
                || id
                    .strip_prefix("course-")
                    .and_then(|n| n.parse::<u32>().ok())
                    .is_some_and(|n| n <= 6);
            if exercise && seeded {
                continue;
            }
            let marker = format!("catalog-installed:{id}");
            if !exercise
                && tx
                    .query_row(
                        "SELECT value FROM preferences WHERE key=?1",
                        [&marker],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                    .is_some_and(|raw| raw == "true")
            {
                continue;
            }
            let raw = serde_json::to_string(&song)?;
            let digest = format!("{:x}", Sha256::digest(raw.as_bytes()));
            tx.execute(
                "INSERT OR IGNORE INTO songs VALUES(?1,?2,0,?3,?4)",
                params![id, raw, created, digest],
            )?;
            if !exercise {
                tx.execute(
                    "INSERT OR REPLACE INTO preferences VALUES(?1,'true')",
                    [marker],
                )?;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO preferences VALUES('seeded','true')",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn database_path(&self) -> Result<std::path::PathBuf> {
        Ok(std::path::PathBuf::from(
            self.db.path().context("База не сохранена на диске")?,
        ))
    }
    pub fn songs(&self) -> Result<Vec<Record>> {
        let mut stmt = self
            .db
            .prepare("SELECT id,score,favorite FROM songs ORDER BY created,id")?;
        let mut rows = stmt.query([])?;
        let mut result = vec![];
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let raw: String = row.get(1)?;
            let song = Song::from_json(&raw)
                .with_context(|| format!("Не удалось прочитать пьесу {id}"))?;
            result.push(Record {
                id,
                song,
                favorite: row.get::<_, i32>(2)? != 0,
                score_version: python_hash(&raw)?,
            });
        }
        Ok(result)
    }
    pub fn add_midi(&self, song: &Song, bytes: &[u8]) -> Result<(String, bool)> {
        self.add_score_file(song, bytes, "midi")
    }
    pub fn add_score_file(&self, song: &Song, bytes: &[u8], kind: &str) -> Result<(String, bool)> {
        ensure!(
            ["midi", "musicxml"].contains(&kind),
            "Неизвестный формат нот"
        );
        song.validate()?;
        ensure!(
            bytes.len()
                <= if kind == "musicxml" {
                    crate::musicxml::MAX_BYTES
                } else {
                    crate::midi_file::MAX_BYTES
                },
            "Файл превышает допустимый размер: MIDI — 32 МБ, MusicXML — 16 МБ"
        );
        let hash = format!("{:x}", Sha256::digest(bytes));
        let id = format!("imported-{kind}:{hash}");
        let raw = serde_json::to_string(song)?;
        let created = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
        let added = self.db.execute(
            "INSERT OR IGNORE INTO songs VALUES(?1,?2,0,?3,?4)",
            params![id, raw, created, format!("{kind}:{hash}")],
        )?;
        Ok((id, added > 0))
    }
    /// Atomically changes only annotations, retaining the import identity and all history.
    pub fn save_annotations(&self, id: &str, expected_version: &str, song: &Song) -> Result<()> {
        song.validate()?;
        let raw: String = self
            .db
            .query_row("SELECT score FROM songs WHERE id=?1", [id], |r| r.get(0))
            .context("Пьеса больше не найдена в библиотеке")?;
        ensure!(
            python_hash(&raw)? == expected_version,
            "Пьеса уже изменена. Закройте редактор и откройте её заново"
        );
        let original = Song::from_json(&raw)?;
        let mut annotations_only = original.clone();
        ensure!(
            song.notes.len() == original.notes.len(),
            "Редактор меняет только руки и пальцы"
        );
        for (target, edited) in annotations_only.notes.iter_mut().zip(&song.notes) {
            target.hand.clone_from(&edited.hand);
            target.finger = edited.finger;
        }
        ensure!(
            annotations_only == *song,
            "Редактор меняет только руки и пальцы"
        );
        let count = self.db.execute(
            "UPDATE songs SET score=?1 WHERE id=?2 AND score=?3",
            params![serde_json::to_string(song)?, id, raw],
        )?;
        ensure!(
            count == 1,
            "Пьеса изменилась во время сохранения. Откройте редактор заново"
        );
        Ok(())
    }
    pub fn favorite(&self, id: &str, value: bool) -> Result<()> {
        self.db.execute(
            "UPDATE songs SET favorite=?1 WHERE id=?2",
            params![i32::from(value), id],
        )?;
        Ok(())
    }
    pub fn preference(&self, key: &str) -> Result<Option<Value>> {
        let raw: Option<String> = self
            .db
            .query_row("SELECT value FROM preferences WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        raw.map(|r| Ok(serde_json::from_str(&r)?)).transpose()
    }
    pub fn set_preference(&self, key: &str, value: &Value) -> Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO preferences VALUES(?1,?2)",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
    pub fn save_attempt(&mut self, attempt: &PendingAttempt) -> Result<()> {
        attempt.report.validate()?;
        ensure!(
            !attempt.id.is_empty() && attempt.id.len() <= 200 && !attempt.song_id.is_empty(),
            "Некорректный идентификатор попытки"
        );
        ensure!(
            attempt.created.is_finite() && (0.0..=32503680000.0).contains(&attempt.created),
            "Некорректная дата попытки"
        );
        let raw = serde_json::to_string(&attempt.report)?;
        let tx = self.db.transaction()?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO attempts(id,song_id,created,report) VALUES(?1,?2,?3,?4)",
            params![attempt.id, attempt.song_id, attempt.created, raw],
        )?;
        if inserted == 0 {
            let existing: (Option<String>, f64, String) = tx.query_row(
                "SELECT song_id,created,report FROM attempts WHERE id=?1",
                [&attempt.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            ensure!(
                existing == (Some(attempt.song_id.clone()), attempt.created, raw),
                "Идентификатор уже принадлежит другому результату"
            );
            tx.commit()?;
            return Ok(());
        }
        if attempt.report.passed() {
            let current: Option<String> = tx
                .query_row(
                    "SELECT score FROM songs WHERE id=?1",
                    [&attempt.song_id],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(score) = current
                && python_hash(&score)? == attempt.report.score_version
            {
                tx.execute(
                    "INSERT OR REPLACE INTO preferences VALUES(?1,'true')",
                    [format!("passed:{}", attempt.song_id)],
                )?;
            }
        }
        Self::refresh_awards(&tx)?;
        tx.commit()?;
        Ok(())
    }
    fn refresh_awards(db: &Connection) -> Result<crate::achievements::State> {
        let previous: Option<String> = db
            .query_row(
                "SELECT value FROM preferences WHERE key='achievements'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let earned = previous
            .as_ref()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        let mut stmt=db.prepare("SELECT a.id,a.song_id,a.created,a.report,CAST(julianday(date(a.created,'unixepoch','localtime')) AS INTEGER),s.score FROM attempts a LEFT JOIN songs s ON s.id=a.song_id ORDER BY a.created,a.id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        let mut inputs = vec![];
        for row in rows {
            let (id, song_id, created, raw, day, score) = row?;
            let Some(day) = day else {
                continue;
            };
            let report: Value = serde_json::from_str(&raw)?;
            let level = score
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .and_then(|s| s["difficulty"].as_u64())
                .unwrap_or(0)
                .min(5) as u8;
            inputs.push((
                Attempt {
                    id,
                    song_id,
                    created,
                    report,
                    date: String::new(),
                },
                day,
                level,
            ));
        }
        let state = crate::achievements::evaluate(&inputs, earned.clone());
        // Preserve malformed imported preference values for diagnosis rather than destroying them.
        let valid = previous.as_ref().is_none_or(|s| {
            serde_json::from_str::<Value>(s)
                .ok()
                .is_some_and(|v| v.is_object())
        });
        if valid && state.earned != earned {
            db.execute(
                "INSERT OR REPLACE INTO preferences VALUES('achievements',?1)",
                [serde_json::to_string(&state.earned)?],
            )?;
        }
        Ok(state)
    }
    pub fn achievement_state(&mut self) -> Result<crate::achievements::State> {
        let tx = self.db.transaction()?;
        let state = Self::refresh_awards(&tx)?;
        tx.commit()?;
        Ok(state)
    }
    pub fn attempts(&self) -> Result<Vec<Attempt>> {
        let mut stmt = self
            .db
            .prepare("SELECT song_id,created,report,id,strftime('%d.%m.%Y %H:%M',created,'unixepoch','localtime') FROM attempts ORDER BY created DESC,id DESC")?;
        let mut rows = stmt.query([])?;
        let mut result = vec![];
        while let Some(row) = rows.next()? {
            let raw: String = row.get(2)?;
            let created: f64 = row.get(1)?;
            ensure!(created.is_finite(), "Некорректная дата результата");
            result.push(Attempt {
                id: row.get(3)?,
                date: row
                    .get::<_, Option<String>>(4)?
                    .unwrap_or_else(|| "Дата недоступна".into()),
                song_id: row.get(0)?,
                created,
                report: serde_json::from_str(&raw)?,
            });
        }
        Ok(result)
    }
    pub fn recording_count(&self) -> Result<u32> {
        Ok(self
            .db
            .query_row("SELECT COUNT(*) FROM recordings", [], |r| r.get(0))?)
    }
}

/// Consistent SQLite snapshot, even with an active Python session. Source is opened read-only.
/// Existing destination files are never overwritten; all work belongs to the preview copy.
pub fn import_legacy(source: &Path, destination: &Path) -> Result<()> {
    ensure!(source.is_file(), "Исходная библиотека не найдена");
    let db = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(Duration::from_secs(3))?;
    let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    ensure!(
        version == 1,
        "Поддерживается библиотека Python версии схемы 1"
    );
    for table in ["songs", "attempts", "preferences", "recordings"] {
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |r| r.get(0),
        )?;
        ensure!(exists, "В библиотеке нет таблицы {table}");
    }
    if let Some(parent) = destination.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .context("Копия уже существует или недоступна; перезапись запрещена")?;
    drop(file);
    let result = (|| -> Result<()> {
        db.backup("main", destination, rusqlite::backup::NO_PROGRESS)?;
        let check = Connection::open_with_flags(destination, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let integrity: String = check.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        ensure!(integrity == "ok", "Проверка копии SQLite: {integrity}");
        let library = Library { db: check };
        library.songs()?;
        library.attempts()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn annotations_survive_restart_preserve_identity_and_reject_stale_or_structural_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut l = Library::open(&path).unwrap();
        l.seed().unwrap();
        let record = l
            .songs()
            .unwrap()
            .into_iter()
            .find(|r| r.id == "course-1")
            .unwrap();
        l.favorite(&record.id, true).unwrap();
        l.db.execute(
            "INSERT INTO attempts VALUES('history','course-1',1,'{}')",
            [],
        )
        .unwrap();
        let digest: String =
            l.db.query_row("SELECT digest FROM songs WHERE id='course-1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let mut edited = record.song.clone();
        edited.notes[0].hand = "L".into();
        edited.notes[0].finger = 4;
        l.save_annotations(&record.id, &record.score_version, &edited)
            .unwrap();
        assert!(
            l.save_annotations(&record.id, &record.score_version, &record.song)
                .is_err()
        );
        drop(l);
        let mut l = Library::open(&path).unwrap();
        l.seed().unwrap();
        let saved = l
            .songs()
            .unwrap()
            .into_iter()
            .find(|r| r.id == record.id)
            .unwrap();
        assert_eq!(saved.song, edited);
        assert!(saved.favorite);
        assert_ne!(saved.score_version, record.score_version);
        assert_eq!(l.attempts().unwrap().len(), 1);
        let current_digest: String =
            l.db.query_row("SELECT digest FROM songs WHERE id='course-1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(digest, current_digest);
        for field in 0..3 {
            let mut invalid = edited.clone();
            match field {
                0 => invalid.notes[0].pitch += 1,
                1 => invalid.title = "Changed".into(),
                _ => invalid.notes[0].finger = 6,
            }
            assert!(
                l.save_annotations(&record.id, &saved.score_version, &invalid)
                    .is_err()
            );
        }
        assert_eq!(
            l.songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == record.id)
                .unwrap()
                .song,
            edited
        );
    }

    #[test]
    fn seeds_once_and_preserves_user_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Library::open(&dir.path().join("db")).unwrap();
        l.seed().unwrap();
        l.seed().unwrap();
        assert_eq!(l.songs().unwrap().len(), 124);
        l.favorite("course-1", true).unwrap();
        l.seed().unwrap();
        assert!(
            l.songs()
                .unwrap()
                .into_iter()
                .find(|s| s.id == "course-1")
                .unwrap()
                .favorite
        );
    }
    #[test]
    fn snapshot_preserves_records_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("python.db");
        let target = dir.path().join("preview.db");
        {
            let mut l = Library::open(&source).unwrap();
            l.seed().unwrap();
            l.favorite("course-1", true).unwrap();
            l.set_preference("achievements", &serde_json::json!({"first-notes":12345}))
                .unwrap();
            l.db.execute(
                "INSERT INTO attempts VALUES('attempt','course-1',12345,'{\"accuracy\":91}')",
                [],
            )
            .unwrap();
            l.db.execute(
                "INSERT INTO recordings VALUES('record','Test',12345,x'010203')",
                [],
            )
            .unwrap();
        }
        let before = fs::read(&source).unwrap();
        import_legacy(&source, &target).unwrap();
        assert_eq!(before, fs::read(&source).unwrap());
        let l = Library::open(&target).unwrap();
        assert_eq!(l.attempts().unwrap()[0].report["accuracy"], 91);
        assert_eq!(l.recording_count().unwrap(), 1);
        assert_eq!(
            l.preference("achievements").unwrap().unwrap()["first-notes"],
            12345
        );
        l.favorite("course-1", false).unwrap();
        assert_eq!(before, fs::read(&source).unwrap());
        assert!(import_legacy(&source, &target).is_err());
    }
    #[test]
    fn deleted_builtins_are_not_resurrected() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Library::open(&dir.path().join("db")).unwrap();
        l.seed().unwrap();
        l.db.execute(
            "DELETE FROM songs WHERE id IN ('course-1','birthday-melody','repertoire-002')",
            [],
        )
        .unwrap();
        l.seed().unwrap();
        assert_eq!(l.songs().unwrap().len(), 121);
    }
    #[test]
    fn older_profiles_receive_new_lessons_once_without_replacing_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Library::open(&dir.path().join("db")).unwrap();
        l.seed().unwrap();
        for n in 7..=24 {
            let id = format!("course-{n}");
            l.db.execute("DELETE FROM songs WHERE id=?1", [&id])
                .unwrap();
            l.db.execute(
                "DELETE FROM preferences WHERE key=?1",
                [format!("catalog-installed:{id}")],
            )
            .unwrap();
        }
        assert_eq!(l.songs().unwrap().len(), 106);
        l.favorite("course-1", true).unwrap();
        l.seed().unwrap();
        assert_eq!(l.songs().unwrap().len(), 124);
        l.db.execute("DELETE FROM songs WHERE id='course-7'", [])
            .unwrap();
        l.seed().unwrap();
        assert_eq!(l.songs().unwrap().len(), 123);
        assert!(
            l.songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .favorite
        );
    }
    #[test]
    fn future_schema_rejected_without_schema_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("future.db");
        let db = Connection::open(&path).unwrap();
        db.pragma_update(None, "user_version", 2).unwrap();
        drop(db);
        let before = fs::read(&path).unwrap();
        assert!(Library::open(&path).is_err());
        assert_eq!(before, fs::read(path).unwrap());
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;
    use crate::{
        midi::{MidiEvent, PianoState},
        performance::Options,
        session::{Mode, Session},
    };
    fn pending(library: &Library) -> PendingAttempt {
        let record = library
            .songs()
            .unwrap()
            .into_iter()
            .find(|r| r.id == "course-1")
            .unwrap();
        let mut session = Session::new(
            &record.song,
            Mode::Waiting,
            Options::default(),
            record.score_version,
        )
        .unwrap();
        let mut state = PianoState::default();
        for n in &record.song.notes {
            let event = MidiEvent::parse(&[0x90, n.pitch, 90], "midi").unwrap();
            state.apply(&event);
            session
                .feed(&event, &state.pressed(), n.start, true)
                .unwrap();
            let event = MidiEvent::parse(&[0x80, n.pitch, 0], "midi").unwrap();
            state.apply(&event);
        }
        PendingAttempt::new(record.id, session.finish(true)).unwrap()
    }
    #[test]
    fn one_attempt_one_row_and_passed_flag_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut l = Library::open(&path).unwrap();
        l.seed().unwrap();
        let p = pending(&l);
        l.save_attempt(&p).unwrap();
        l.save_attempt(&p).unwrap();
        assert_eq!(l.attempts().unwrap().len(), 1);
        assert_eq!(
            l.preference("passed:course-1").unwrap(),
            Some(Value::Bool(true))
        );
        let mut collision = p.clone();
        collision.report.title = "Другое занятие".into();
        assert!(l.save_attempt(&collision).is_err());
        drop(l);
        let l = Library::open(&path).unwrap();
        assert_eq!(l.attempts().unwrap().len(), 1);
        assert_eq!(
            l.attempts().unwrap()[0].report,
            serde_json::to_value(p.report).unwrap()
        );
    }
    #[test]
    fn failed_passed_update_rolls_back_attempt_and_retry_is_safe() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Library::open(&dir.path().join("db")).unwrap();
        l.seed().unwrap();
        let p = pending(&l);
        l.db.execute_batch("CREATE TRIGGER fail_progress BEFORE INSERT ON preferences WHEN NEW.key LIKE 'passed:%' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(l.save_attempt(&p).is_err());
        assert!(l.attempts().unwrap().is_empty());
        assert_eq!(l.preference("passed:course-1").unwrap(), None);
        l.db.execute_batch("DROP TRIGGER fail_progress").unwrap();
        l.save_attempt(&p).unwrap();
        l.save_attempt(&p).unwrap();
        assert_eq!(l.attempts().unwrap().len(), 1);
    }
    #[test]
    fn invalid_report_never_reaches_database() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Library::open(&dir.path().join("db")).unwrap();
        l.seed().unwrap();
        let mut p = pending(&l);
        p.report.outcome.accuracy = 99.0;
        assert!(l.save_attempt(&p).is_err());
        assert!(l.attempts().unwrap().is_empty());
    }
}

#[cfg(test)]
mod achievement_tests {
    use super::*;
    use crate::{
        midi::{MidiEvent, PianoState},
        performance::Options,
        session::{Mode, Session},
    };
    #[test]
    fn new_awards_are_atomic_and_recovered_from_legacy_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut l = Library::open(&path).unwrap();
        // A legacy result is legal even after its song is deleted: its own level is retained.
        l.db.execute("INSERT INTO attempts VALUES('legacy','deleted',1704110400,?1)",[serde_json::json!({"hit":2,"total":2,"complete":true,"full_song":true,"accuracy":95,"both_hands":true,"difficulty":5}).to_string()]).unwrap();
        let state = l.achievement_state().unwrap();
        assert_eq!(&state.progress[..6], &[1, 1, 90, 1, 1, 1]);
        assert_eq!(
            crate::achievements::DEFINITIONS[..6]
                .iter()
                .filter(|d| state.earned.contains_key(d.key))
                .count(),
            4
        );
        let previous = state.earned.clone();
        drop(l);
        let mut l = Library::open(&path).unwrap();
        assert_eq!(l.achievement_state().unwrap().earned, previous);
        l.db.execute("DELETE FROM preferences WHERE key='achievements'", [])
            .unwrap();
        l.db.execute_batch("CREATE TRIGGER deny_awards BEFORE INSERT ON preferences WHEN NEW.key='achievements' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(l.achievement_state().is_err());
        assert!(l.preference("achievements").unwrap().is_none());
        l.db.execute_batch("DROP TRIGGER deny_awards").unwrap();
        assert_eq!(l.achievement_state().unwrap().earned, previous);
    }
    #[test]
    fn two_awards_are_earned_once_without_overwriting_imported_medals() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Library::open(&dir.path().join("db")).unwrap();
        l.seed().unwrap();
        l.set_preference("achievements", &serde_json::json!({"explorer":12345}))
            .unwrap();
        let r = l
            .songs()
            .unwrap()
            .into_iter()
            .find(|r| r.id == "course-1")
            .unwrap();
        let mut s =
            Session::new(&r.song, Mode::Waiting, Options::default(), r.score_version).unwrap();
        let mut state = PianoState::default();
        for n in &r.song.notes {
            let on = MidiEvent::parse(&[0x90, n.pitch, 90], "midi").unwrap();
            state.apply(&on);
            s.feed(&on, &state.pressed(), n.start, true).unwrap();
            state.apply(&MidiEvent::parse(&[0x80, n.pitch, 0], "midi").unwrap());
        }
        let p = PendingAttempt::new(r.id, s.finish(true)).unwrap();
        l.save_attempt(&p).unwrap();
        let earned = l.preference("achievements").unwrap().unwrap();
        assert_eq!(earned["explorer"], 12345);
        assert_eq!(earned["first-notes"], p.created);
        assert_eq!(earned["first-piece"], p.created);
        l.save_attempt(&p).unwrap();
        assert_eq!(l.preference("achievements").unwrap(), Some(earned));
    }
    #[test]
    fn partial_attempt_unlocks_only_first_notes_and_silent_completion_neither() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Library::open(&dir.path().join("db")).unwrap();
        l.seed().unwrap();
        let r = l
            .songs()
            .unwrap()
            .into_iter()
            .find(|r| r.id == "course-1")
            .unwrap();
        let mut silent = Session::new(
            &r.song,
            Mode::Rhythm,
            Options::default(),
            r.score_version.clone(),
        )
        .unwrap();
        l.save_attempt(&PendingAttempt::new(r.id.clone(), silent.finish(true)).unwrap())
            .unwrap();
        assert_eq!(l.preference("achievements").unwrap(), None);
        let mut partial =
            Session::new(&r.song, Mode::Waiting, Options::default(), r.score_version).unwrap();
        partial
            .feed(
                &MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(),
                &std::collections::BTreeSet::from([60]),
                0.0,
                true,
            )
            .unwrap();
        l.save_attempt(&PendingAttempt::new(r.id, partial.finish(false)).unwrap())
            .unwrap();
        let earned = l.preference("achievements").unwrap().unwrap();
        assert!(earned.get("first-notes").is_some());
        assert!(earned.get("first-piece").is_none());
    }
}
