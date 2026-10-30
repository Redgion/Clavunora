// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use anyhow::{Context, Result, ensure};
use clavunora_core::midi_file::{self, Import};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
};
pub struct Preview {
    pub bytes: Vec<u8>,
    pub kind: &'static str,
    pub parsed: Import,
    pub name: String,
}
pub type Reply = Result<Option<Preview>, String>;
pub fn read(path: &Path) -> Result<Preview> {
    ensure!(
        std::fs::metadata(path)
            .context("Нотный файл не найден")?
            .is_file(),
        "Выберите обычный нотный файл"
    );
    let file = std::fs::File::open(path).context("Не удалось открыть нотный файл")?;
    ensure!(file.metadata()?.is_file(), "Выберите обычный нотный файл");
    let mut bytes = vec![];
    file.take(midi_file::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= midi_file::MAX_BYTES,
        "Нотный файл больше 32 МБ"
    );
    let title = path
        .file_stem()
        .and_then(|v| v.to_str())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("Импорт нот");
    ensure!(
        !bytes.starts_with(b"PK"),
        "Сжатый MXL пока не поддерживается. Экспортируйте несжатый .musicxml"
    );
    let xml = !bytes.starts_with(b"MThd")
        && (path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("xml") || e.eq_ignore_ascii_case("musicxml"))
            || bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'<'));
    let (parsed, kind) = if xml {
        (clavunora_core::musicxml::parse(&bytes, title)?, "musicxml")
    } else {
        (midi_file::parse(&bytes, title)?, "midi")
    };
    Ok(Preview {
        bytes,
        kind,
        parsed,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    })
}
/// The optional desktop picker and parsing stay off the UI/MIDI thread. Cancel changes nothing.
pub fn start(path: Option<PathBuf>) -> Receiver<Reply> {
    let (send, recv) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<Option<Preview>> {
            let path = if let Some(path) = path {
                path
            } else {
                let Some(path) = crate::dialogs::choose(crate::dialogs::Kind::Score)? else {
                    return Ok(None);
                };
                path
            };
            Ok(Some(read(&path)?))
        })();
        let _ = send.send(result.map_err(|e| format!("{e:#}")));
    });
    recv
}
