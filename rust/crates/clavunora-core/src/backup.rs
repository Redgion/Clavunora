// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Consistent SQLite snapshots. Preview uses a private immutable staging copy;
//! only validated app schemas are ever restored, never SQL supplied by an archive.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

pub const MAX_BYTES: u64 = 512 * 1024 * 1024;
const APP_ID: i32 = 0x434c4255;
const TABLES: [(&str, &[&str]); 4] = [
    ("songs", &["id", "score", "favorite", "created", "digest"]),
    ("attempts", &["id", "song_id", "created", "report"]),
    ("preferences", &["key", "value"]),
    ("recordings", &["id", "name", "created", "data"]),
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub songs: usize,
    pub attempts: usize,
    pub preferences: usize,
    pub recordings: usize,
}
pub struct Preview {
    file: NamedTempFile,
    pub summary: Summary,
    pub name: String,
}
fn read_only(path: &Path) -> Result<Connection> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(Duration::from_secs(3))?;
    db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")?;
    Ok(db)
}
fn bounded_text(value: &str, max: usize) -> Result<()> {
    ensure!(value.len() <= max, "Слишком длинная запись резервной копии");
    Ok(())
}
fn date(value: f64) -> Result<()> {
    ensure!(
        value.is_finite() && (0.0..=32503680000.0).contains(&value),
        "Некорректная дата в копии"
    );
    Ok(())
}
fn validate(db: &Connection, marked: bool) -> Result<Summary> {
    ensure!(
        db.pragma_query_value::<i32, _>(None, "user_version", |r| r.get(0))? == 1,
        "Неподдерживаемая версия копии"
    );
    if marked {
        ensure!(
            db.pragma_query_value::<i32, _>(None, "application_id", |r| r.get(0))? == APP_ID,
            "Это не резервная копия Clavunora"
        );
    }
    let check: String = db.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    ensure!(check == "ok", "Повреждённая резервная копия: {check}");
    let mut stmt =
        db.prepare("SELECT type,name,sql FROM sqlite_master WHERE NOT(type='index' AND name LIKE 'sqlite_autoindex_%' AND sql IS NULL)")?;
    let objects = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(objects.len() == 4, "Неожиданная структура копии");
    for (kind, name, sql) in objects {
        ensure!(
            kind == "table" && TABLES.iter().any(|(t, _)| *t == name),
            "Неподдерживаемый объект в копии"
        );
        // A strict schema whitelist excludes virtual tables, triggers, generated
        // expressions and executable defaults, rather than trusting SQLite text.
        let expected = match name.as_str() {
            "songs" => {
                "CREATE TABLE songs(id TEXT PRIMARY KEY,score TEXT NOT NULL,favorite INTEGER NOT NULL DEFAULT 0,created REAL NOT NULL,digest TEXT)"
            }
            "attempts" => {
                "CREATE TABLE attempts(id TEXT PRIMARY KEY,song_id TEXT,created REAL,report TEXT NOT NULL)"
            }
            "preferences" => "CREATE TABLE preferences(key TEXT PRIMARY KEY,value TEXT NOT NULL)",
            "recordings" => {
                "CREATE TABLE recordings(id TEXT PRIMARY KEY,name TEXT,created REAL,data BLOB)"
            }
            _ => unreachable!(),
        };
        let normalize = |s: &str| {
            s.chars()
                .filter(|c| !c.is_whitespace())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        };
        ensure!(
            normalize(sql.as_deref().unwrap_or("")) == normalize(expected),
            "Неподдерживаемая схема таблицы {name}"
        );
    }
    let mut counts = vec![];
    for (table, columns) in TABLES {
        let count: i64 =
            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
        ensure!(
            (0..=500_000).contains(&count),
            "Слишком много записей в копии"
        );
        counts.push(count as usize);
        let mut stmt = db.prepare(&format!("SELECT {} FROM {table}", columns.join(",")))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            bounded_text(&id, 1000)?;
            ensure!(!id.is_empty(), "Пустой идентификатор в копии");
            match table {
                "songs" => {
                    let raw: String = row.get(1)?;
                    bounded_text(&raw, 32 * 1024 * 1024)?;
                    crate::score::Song::from_json(&raw)?;
                    ensure!(
                        [0, 1].contains(&row.get::<_, i32>(2)?),
                        "Некорректное избранное"
                    );
                    date(row.get(3)?)?;
                    if let Some(digest) = row.get::<_, Option<String>>(4)? {
                        bounded_text(&digest, 1000)?;
                    }
                }
                "attempts" => {
                    if let Some(id) = row.get::<_, Option<String>>(1)? {
                        bounded_text(&id, 1000)?;
                    }
                    date(row.get(2)?)?;
                    let raw: String = row.get(3)?;
                    bounded_text(&raw, 32 * 1024 * 1024)?;
                    let report: serde_json::Value = serde_json::from_str(&raw)?;
                    ensure!(report.is_object(), "Некорректный результат занятия");
                    // Legacy reports may omit fields added by the Rust engine.
                    for key in ["accuracy", "speed", "start", "end", "tolerance", "latency"] {
                        if let Some(v) = report.get(key) {
                            ensure!(
                                v.as_f64().is_some_and(f64::is_finite),
                                "Некорректное число в результате"
                            );
                        }
                    }
                    ensure!(
                        report
                            .get("accuracy")
                            .is_none_or(|v| v.as_f64().is_some_and(|n| (0.0..=100.0).contains(&n))),
                        "Некорректная точность"
                    );
                    for key in ["hit", "total"] {
                        if let Some(v) = report.get(key) {
                            ensure!(v.as_u64().is_some(), "Некорректное количество нот");
                        }
                    }
                    for key in ["complete", "full_song", "both_hands"] {
                        if let Some(v) = report.get(key) {
                            ensure!(v.is_boolean(), "Некорректный статус результата");
                        }
                    }
                    if let Some(v) = report.get("difficulty") {
                        ensure!(
                            v.as_u64().is_some_and(|d| d <= 5),
                            "Некорректная сложность результата"
                        );
                    }
                    for key in ["title", "mode"] {
                        if let Some(v) = report.get(key) {
                            ensure!(v.is_string(), "Некорректное описание результата");
                        }
                    }
                    if let Some(v) = report.get("mistakes") {
                        for m in v.as_array().context("Некорректный список ошибок")?
                        {
                            ensure!(
                                m.get("pitch")
                                    .and_then(|v| v.as_u64())
                                    .is_some_and(|p| p <= 127)
                                    && m.get("kind").is_some_and(|v| v.is_string()),
                                "Некорректная ошибка занятия"
                            );
                            for key in ["position", "delta"] {
                                if let Some(v) = m.get(key) {
                                    ensure!(
                                        v.as_f64().is_some_and(f64::is_finite),
                                        "Некорректное время ошибки"
                                    );
                                }
                            }
                        }
                    }
                }
                "preferences" => {
                    let raw: String = row.get(1)?;
                    bounded_text(&raw, 4 * 1024 * 1024)?;
                    let value: serde_json::Value = serde_json::from_str(&raw)?;
                    if id == "achievements" {
                        let map = value.as_object().context("Некорректные достижения")?;
                        for value in map.values() {
                            date(value.as_f64().context("Некорректная дата достижения")?)?;
                        }
                    }
                }
                "recordings" => {
                    if let Some(name) = row.get::<_, Option<String>>(1)? {
                        bounded_text(&name, 100_000)?;
                    }
                    date(row.get(2)?)?;
                    let _: Vec<u8> = row.get(3)?;
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(Summary {
        songs: counts[0],
        attempts: counts[1],
        preferences: counts[2],
        recordings: counts[3],
    })
}
/// Creates a new file atomically and never overwrites an existing path.
pub fn create(source: &Path, destination: &Path) -> Result<Summary> {
    let db = read_only(source)?;
    create_snapshot(&db, destination)
}
fn create_snapshot(db: &Connection, destination: &Path) -> Result<Summary> {
    ensure!(
        !destination.exists(),
        "Файл уже существует. Укажите новое имя копии"
    );
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = NamedTempFile::new_in(parent)?;
    let mut target = Connection::open(temp.path())?;
    {
        let backup = rusqlite::backup::Backup::new(db, &mut target)?;
        let start = Instant::now();
        loop {
            ensure!(
                start.elapsed() < Duration::from_secs(60),
                "Создание копии заняло слишком долго; повторите позже"
            );
            match backup.step(128)? {
                rusqlite::backup::StepResult::Done => break,
                rusqlite::backup::StepResult::More => {}
                _ => anyhow::bail!("Библиотека занята. Повторите создание копии"),
            }
            ensure!(
                temp.as_file().metadata()?.len() <= MAX_BYTES,
                "Копия больше 512 МБ"
            );
        }
    }
    target.execute_batch("PRAGMA journal_mode=DELETE;")?;
    target.pragma_update(None, "application_id", APP_ID)?;
    let summary = validate(&target, true)?;
    drop(target);
    ensure!(
        temp.as_file().metadata()?.len() <= MAX_BYTES,
        "Копия больше 512 МБ"
    );
    temp.as_file().sync_all()?;
    temp.persist_noclobber(destination).map_err(|e| e.error)?;
    // The data file was flushed above before the atomic no-clobber rename.
    // Unix additionally supports directory fsync; Windows File::open on a
    // directory returns AccessDenied after an otherwise successful save.
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(summary)
}
/// Freeze the chosen file before previewing it; restore never reopens a changed user path.
pub fn preview(path: &Path) -> Result<Preview> {
    let mut source = File::open(path).context("Не удалось открыть резервную копию")?;
    ensure!(source.metadata()?.is_file(), "Выберите обычный файл копии");
    ensure!(source.metadata()?.len() <= MAX_BYTES, "Копия больше 512 МБ");
    let mut staged = NamedTempFile::new()?;
    let bytes = std::io::copy(
        &mut Read::by_ref(&mut source).take(MAX_BYTES + 1),
        &mut staged,
    )?;
    ensure!(bytes <= MAX_BYTES, "Копия больше 512 МБ");
    staged.flush()?;
    let mut header = [0u8; 20];
    File::open(staged.path())?.read_exact(&mut header)?;
    ensure!(
        &header[..16] == b"SQLite format 3\0" && header[18..20] == [1, 1],
        "Неподдерживаемая или незавершённая копия"
    );
    let summary = validate(&read_only(staged.path())?, true)?;
    Ok(Preview {
        file: staged,
        summary,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
    })
}
/// A failed restore leaves the target unchanged; SQLite commits the snapshot as a unit.
/// The safety snapshot is committed before the restore starts and is never deleted here.
pub fn restore(target: &Path, preview: Preview, safety_dir: &Path) -> Result<PathBuf> {
    restore_with(target, preview, safety_dir, || Ok(()))
}
fn restore_with(
    target: &Path,
    preview: Preview,
    safety_dir: &Path,
    after_safety: impl FnOnce() -> Result<()>,
) -> Result<PathBuf> {
    validate(&read_only(preview.file.path())?, true)?;
    fs::create_dir_all(safety_dir)?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let safety = safety_dir.join(format!("before-restore-{stamp}.clavunora-backup"));
    let mut db = Connection::open_with_flags(target, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    db.busy_timeout(Duration::from_secs(3))?;
    let source = read_only(preview.file.path())?;
    let tx = db.transaction()?;
    // Pin a read snapshot before creating the safety file. An intervening writer
    // makes the upgrade fail (BUSY/BUSY_SNAPSHOT), so no unbacked edits are lost.
    let _: i64 = tx.query_row("SELECT COUNT(*) FROM songs", [], |r| r.get(0))?;
    create_snapshot(&tx, &safety)
        .context("Не удалось сохранить текущую библиотеку. Восстановление отменено")?;
    after_safety()?;
    let result = (|| -> Result<()> {
        for (table, columns) in TABLES {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
            let mut stmt = source.prepare(&format!("SELECT {} FROM {table}", columns.join(",")))?;
            let mut rows = stmt.query([])?;
            let sql = format!(
                "INSERT INTO {table} ({}) VALUES({})",
                columns.join(","),
                vec!["?"; columns.len()].join(",")
            );
            let mut insert = tx.prepare(&sql)?;
            while let Some(row) = rows.next()? {
                let values = (0..columns.len())
                    .map(|i| row.get::<_, rusqlite::types::Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                insert.execute(rusqlite::params_from_iter(values))?;
            }
        }
        Ok(())
    })();
    result.with_context(|| {
        format!(
            "Восстановление не выполнено. Текущая копия сохранена: {}",
            safety.display()
        )
    })?;
    tx.commit()?;
    Ok(safety)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Library;
    #[test]
    fn wal_snapshot_restores_every_table_and_safety_copy_can_undo_restore() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        let mut library = Library::open(&db_path).unwrap();
        library.seed().unwrap();
        let writer = Connection::open(&db_path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        library.favorite("course-1", true).unwrap();
        library
            .set_preference("rust:plain-theme", &serde_json::json!(true))
            .unwrap();
        writer
            .execute(
                "INSERT INTO attempts VALUES('history','course-1',123,'{\"accuracy\":50}')",
                [],
            )
            .unwrap();
        writer
            .execute(
                "INSERT INTO recordings VALUES('old','Preserve',123,x'010203')",
                [],
            )
            .unwrap();
        let file = dir.path().join("copy.clavunora-backup");
        let summary = create(&db_path, &file).unwrap();
        assert_eq!(summary.songs, 124);
        assert_eq!(summary.attempts, 1);
        assert_eq!(summary.recordings, 1);
        assert!(create(&db_path, &file).is_err());
        let selected = preview(&file).unwrap();
        // Selection is frozen, not silently replaced by later edits to this path.
        fs::write(&file, b"changed after preview").unwrap();
        library.favorite("course-1", false).unwrap();
        library
            .set_preference("rust:plain-theme", &serde_json::json!(false))
            .unwrap();
        writer.execute("DELETE FROM recordings", []).unwrap();
        let safety = restore(&db_path, selected, &dir.path().join("backups")).unwrap();
        assert!(
            library
                .songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .favorite
        );
        assert_eq!(
            library.preference("rust:plain-theme").unwrap(),
            Some(serde_json::json!(true))
        );
        assert_eq!(library.recording_count().unwrap(), 1);
        assert_eq!(library.attempts().unwrap().len(), 1);
        restore(
            &db_path,
            preview(&safety).unwrap(),
            &dir.path().join("backups"),
        )
        .unwrap();
        assert!(
            !library
                .songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .favorite
        );
        assert_eq!(library.recording_count().unwrap(), 0);
    }
    #[test]
    fn intervening_wal_writer_is_not_overwritten_by_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut library = Library::open(&path).unwrap();
        library.seed().unwrap();
        let writer = Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        let copy = dir.path().join("copy.clavunora-backup");
        create(&path, &copy).unwrap();
        let result = restore_with(
            &path,
            preview(&copy).unwrap(),
            &dir.path().join("backups"),
            || {
                writer.execute("UPDATE songs SET favorite=1 WHERE id='course-1'", [])?;
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(
            library
                .songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .favorite
        );
    }
    #[test]
    fn rejects_corrupt_schema_invalid_notes_and_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut library = Library::open(&path).unwrap();
        library.seed().unwrap();
        let copy = dir.path().join("copy.clavunora-backup");
        create(&path, &copy).unwrap();
        let original = fs::read(&copy).unwrap();
        for mutation in 0..4 {
            fs::write(&copy, &original).unwrap();
            let db = Connection::open(&copy).unwrap();
            match mutation {
                0=>db.execute_batch("CREATE TRIGGER evil AFTER INSERT ON songs BEGIN DELETE FROM attempts; END;").unwrap(),
                1=>db.execute_batch("UPDATE songs SET score='{}' WHERE id='course-1';").unwrap(),
                2=>db.execute_batch("PRAGMA user_version=2;").unwrap(),
                _=>db.execute_batch("PRAGMA application_id=0;").unwrap(),
            }
            drop(db);
            assert!(preview(&copy).is_err());
        }
        fs::write(&copy, &original[..200]).unwrap();
        assert!(preview(&copy).is_err());
        assert_eq!(library.songs().unwrap().len(), 124);
    }
    #[test]
    fn refuses_restore_when_safety_cannot_be_saved_and_rolls_back_busy_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut library = Library::open(&path).unwrap();
        library.seed().unwrap();
        let copy = dir.path().join("copy.clavunora-backup");
        create(&path, &copy).unwrap();
        library.favorite("course-1", true).unwrap();
        let not_folder = dir.path().join("not-folder");
        fs::write(&not_folder, b"x").unwrap();
        assert!(restore(&path, preview(&copy).unwrap(), &not_folder).is_err());
        let lock = Connection::open(&path).unwrap();
        lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
        let safety_dir = dir.path().join("backups");
        assert!(restore(&path, preview(&copy).unwrap(), &safety_dir).is_err());
        lock.execute_batch("ROLLBACK;").unwrap();
        assert!(
            library
                .songs()
                .unwrap()
                .into_iter()
                .find(|r| r.id == "course-1")
                .unwrap()
                .favorite
        );
        assert_eq!(fs::read_dir(safety_dir).unwrap().count(), 1);
    }
}
