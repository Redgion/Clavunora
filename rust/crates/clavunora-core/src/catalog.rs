// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use crate::score::{Note, Song};
use anyhow::{Context, Result, ensure};
include!(concat!(env!("OUT_DIR"), "/catalog.rs"));

fn note(pitch: u8, start: f64, duration: f64, hand: &str, finger: u8) -> Note {
    Note {
        pitch,
        start,
        duration,
        velocity: 88,
        track: 0,
        channel: 0,
        hand: hand.into(),
        finger,
    }
}
fn song(title: &str, notes: Vec<Note>, bpm: f64, level: u8, description: &str) -> Song {
    Song {
        title: title.into(),
        notes,
        bpm,
        time_signature: [4, 4],
        track_names: vec!["Мелодия".into()],
        difficulty: level,
        composer: String::new(),
        work: "Курс с нуля".into(),
        description: description.into(),
        source: "Собственные упражнения Clavunora".into(),
        tempo_map: vec![],
    }
}

pub fn course() -> Vec<(String, Song)> {
    let definitions = [
        (
            "Найдите до",
            vec![60, 60, 60, 60],
            vec![1, 1, 1, 1],
            "R",
            "Найдите до первой октавы (C4). Сыграйте большим пальцем правой руки. Отпускайте клавишу перед повтором.",
        ),
        (
            "Пять пальцев правой руки",
            vec![60, 62, 64, 65, 67, 65, 64, 62, 60],
            vec![1, 2, 3, 4, 5, 4, 3, 2, 1],
            "R",
            "Поставьте пальцы на до–ре–ми–фа–соль. 1 — большой, 5 — мизинец. Играйте по порядку без перемещения руки.",
        ),
        (
            "Пять пальцев левой руки",
            vec![48, 50, 52, 53, 55, 53, 52, 50, 48],
            vec![5, 4, 3, 2, 1, 2, 3, 4, 5],
            "L",
            "Левая рука: мизинец на до, большой на соль. Сначала играйте с ожиданием, затем попробуйте ровный ритм.",
        ),
        (
            "Ритм и паузы",
            vec![60, 62, 64, 60, 64, 62, 60],
            vec![1, 2, 3, 1, 3, 2, 1],
            "R",
            "Считайте 1–2–3–4. После каждой ноты оставляйте паузу. В непрерывном режиме следите за моментом нажатия и отпускания.",
        ),
    ];
    let mut output = vec![];
    for (i, (title, pitches, fingers, hand, description)) in definitions.into_iter().enumerate() {
        let notes = pitches
            .into_iter()
            .zip(fingers)
            .enumerate()
            .map(|(j, (p, f))| {
                note(
                    p,
                    j as f64 * 0.75,
                    if i == 3 { 0.35 } else { 0.65 },
                    hand,
                    f,
                )
            })
            .collect();
        output.push((
            format!("course-{}", i + 1),
            song(title, notes, 80.0, 1, description),
        ));
    }
    let mut notes = vec![];
    for j in 0..4 {
        notes.push(note(48, j as f64, 0.8, "L", 5));
        let mut n = note(60 + (j % 3) * 2, j as f64, 0.8, "R", 1 + j % 3);
        n.track = 1;
        notes.push(n);
    }
    let mut both = song(
        "Две руки вместе",
        notes,
        60.0,
        2,
        "Левая рука повторяет до, правая играет до–ре–ми–до. Нажимайте обе ноты одновременно.",
    );
    both.track_names = vec!["Левая".into(), "Правая".into()];
    both.source = "Собственное упражнение Clavunora".into();
    output.push(("course-5".into(), both));
    let notes = (0..3)
        .flat_map(|j| {
            [(60, 1), (64, 3), (67, 5)]
                .into_iter()
                .map(move |(p, f)| note(p, j as f64 * 1.5, 1.2, "R", f))
        })
        .collect();
    let mut chord = song(
        "Первый аккорд",
        notes,
        80.0,
        2,
        "Сыграйте до–ми–соль пальцами 1–3–5 правой руки. Все три клавиши должны быть нажаты вместе.",
    );
    chord.source = "Собственное упражнение Clavunora".into();
    output.push(("course-6".into(), chord));
    output.extend(crate::course_extension::lessons());
    output
}

pub fn birthday_variants() -> Vec<(String, Song)> {
    let phrases: Vec<Vec<(u8, f64)>> = vec![
        vec![
            (60, 0.75),
            (60, 0.25),
            (62, 1.0),
            (60, 1.0),
            (65, 1.0),
            (64, 2.0),
        ],
        vec![
            (60, 0.75),
            (60, 0.25),
            (62, 1.0),
            (60, 1.0),
            (67, 1.0),
            (65, 2.0),
        ],
        vec![
            (60, 0.75),
            (60, 0.25),
            (72, 1.0),
            (69, 1.0),
            (65, 1.0),
            (64, 1.0),
            (62, 1.0),
        ],
        vec![
            (70, 0.75),
            (70, 0.25),
            (69, 1.0),
            (65, 1.0),
            (67, 1.0),
            (65, 2.0),
        ],
    ];
    let fingers = [
        1, 1, 2, 1, 4, 3, 1, 1, 2, 1, 5, 4, 1, 1, 5, 4, 2, 1, 2, 5, 5, 4, 2, 3, 2,
    ];
    let mut beat = 0.0;
    let notes = phrases
        .into_iter()
        .flatten()
        .zip(fingers)
        .map(|((p, length), finger)| {
            let n = note(p, beat * 0.625, length * 0.625 * 0.9, "R", finger);
            beat += length;
            n
        })
        .collect();
    let mut base = song(
        "С днём рождения",
        notes,
        96.0,
        1,
        "Подсказки пальцев — один из вариантов аппликатуры. Разучивайте короткими фразами.",
    );
    base.time_signature = [3, 4];
    base.work = "С днём рождения".into();
    base.source = "Собственная учебная аранжировка Clavunora".into();
    let mut bass = base.clone();
    bass.title = "С днём рождения · с басом".into();
    bass.difficulty = 3;
    bass.track_names = vec!["Правая".into(), "Левая".into()];
    for (p, t, f) in [(41, 0.0, 5), (48, 3.75, 1), (41, 7.5, 5), (46, 11.25, 2)] {
        let mut n = note(p, t, 1.5, "L", f);
        n.track = 1;
        bass.notes.push(n);
    }
    bass.notes.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut chords = base.clone();
    chords.title = "С днём рождения · с аккордами".into();
    chords.difficulty = 3;
    chords.track_names = bass.track_names.clone();
    for (t, pitches) in [
        (0.0, [41, 45, 48]),
        (3.75, [48, 52, 55]),
        (7.5, [41, 45, 48]),
        (11.25, [46, 50, 53]),
    ] {
        for (p, f) in pitches.into_iter().zip([5, 3, 1]) {
            let mut n = note(p, t, 1.4, "L", f);
            n.track = 1;
            chords.notes.push(n);
        }
    }
    chords.notes.sort_by(|a, b| a.start.total_cmp(&b.start));
    vec![
        ("birthday-melody".into(), base),
        ("birthday-bass".into(), bass),
        ("birthday-chords".into(), chords),
    ]
}

/// Scores are independently licensed data shipped beside the executable.
/// Do not embed third-party editions in the GPL program binary.
#[cfg(target_os = "android")]
pub static ANDROID_CATALOG: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub fn score_raw(relative: &str) -> Result<String> {
    #[cfg(target_os = "android")]
    let root = ANDROID_CATALOG
        .get()
        .context("Android catalog not initialized")?
        .canonicalize()?;
    #[cfg(not(target_os = "android"))]
    let root = {
        let executable = std::env::current_exe()?;
        let directory = executable.parent().context("Нет каталога программы")?;
        let installed = directory.join("resources/catalog");
        let mac_bundle = directory.join("../Resources/catalog");
        let candidate = if installed.is_dir() {
            installed
        } else if mac_bundle.is_dir() {
            mac_bundle
        } else {
            // Development runs may use a checkout relative to their working directory.
            // Never embed the developer's absolute checkout path in distributed binaries.
            let cwd = std::env::current_dir()?;
            cwd.ancestors()
                .take(4)
                .map(|ancestor| ancestor.join("pianoflow/resources/catalog"))
                .find(|path| path.is_dir())
                .unwrap_or(installed)
        }
        .canonicalize()
        .context("Не найдена библиотека нот. Распакуйте приложение вместе с папкой resources")?;
        candidate
    };
    let path = root
        .join(relative)
        .canonicalize()
        .context("Не найден файл нот в библиотеке")?;
    ensure!(
        path.starts_with(&root),
        "Путь нот выходит за пределы библиотеки"
    );
    std::fs::read_to_string(path).context("Не удалось прочитать файл нот")
}

pub fn bundled() -> Result<Vec<(String, Song)>> {
    let mut songs = course();
    songs.extend(birthday_variants());
    for (id, raw) in BUNDLED_SCORES {
        songs.push(((*id).into(), Song::from_json(&score_raw(raw)?)?));
    }
    Ok(songs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exercises_match_python_field_for_field() {
        let expected: std::collections::BTreeMap<String, Song> =
            serde_json::from_str(include_str!("../tests/fixtures/python-exercises.json")).unwrap();
        for (id, song) in course().into_iter().take(6).chain(birthday_variants()) {
            assert_eq!(song, expected[&id], "Python contract: {id}");
        }
    }
    #[test]
    fn all_existing_scores_are_valid_and_unavailable_scores_excluded() {
        let songs = bundled().unwrap();
        assert_eq!(songs.len(), 124);
        assert_eq!(BUNDLED_SCORES.len(), 97);
        for (id, s) in &songs {
            s.validate().unwrap();
            assert_ne!(id, "repertoire-048");
            assert_ne!(id, "repertoire-076");
        }
        assert_eq!(birthday_variants()[0].1.notes.len(), 25);
        assert_eq!(course().len(), 24);
    }
}
