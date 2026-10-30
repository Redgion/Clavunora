// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Awards derived from saved, real-input attempts. Dates are local calendar days from SQLite.
use crate::store::Attempt;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub struct Definition {
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub target: u32,
}
pub const DEFINITIONS: [Definition; 16] = [
    Definition {
        key: "first-notes",
        title: "Первые ноты",
        description: "Сыграйте хотя бы одну верную ноту",
        target: 1,
    },
    Definition {
        key: "first-piece",
        title: "Первое произведение",
        description: "Завершите целую пьесу с точностью от 60%",
        target: 1,
    },
    Definition {
        key: "accurate",
        title: "Точное попадание",
        description: "Завершите целую пьесу с точностью от 90%",
        target: 90,
    },
    Definition {
        key: "both-hands",
        title: "Обе руки",
        description: "Завершите пьесу обеими руками с точностью от 80%",
        target: 1,
    },
    Definition {
        key: "seven-days",
        title: "Семь дней музыки",
        description: "Занимайтесь семь дней подряд с верными нотами",
        target: 7,
    },
    Definition {
        key: "explorer",
        title: "Исследователь",
        description: "Сыграйте верные ноты в пьесах всех пяти уровней",
        target: 5,
    },
    Definition {
        key: "hundred-notes",
        title: "Сто верных нот",
        description: "Сыграйте суммарно 100 верных нот или групп в сохранённых занятиях",
        target: 100,
    },
    Definition {
        key: "thousand-notes",
        title: "Тысяча попаданий",
        description: "Сыграйте суммарно 1000 верных нот или групп в сохранённых занятиях",
        target: 1000,
    },
    Definition {
        key: "five-pieces",
        title: "Маленький репертуар",
        description: "Завершите 5 разных целых пьес с точностью от 60%",
        target: 5,
    },
    Definition {
        key: "ten-pieces",
        title: "Свой концерт",
        description: "Завершите 10 разных целых пьес с точностью от 60%",
        target: 10,
    },
    Definition {
        key: "left-hand",
        title: "Уверенная левая",
        description: "Завершите занятие выбранной левой рукой с точностью от 80%",
        target: 1,
    },
    Definition {
        key: "right-hand",
        title: "Уверенная правая",
        description: "Завершите занятие выбранной правой рукой с точностью от 80%",
        target: 1,
    },
    Definition {
        key: "rhythm-player",
        title: "В своём ритме",
        description: "Завершите целую пьесу в режиме ритма с точностью от 80%",
        target: 1,
    },
    Definition {
        key: "clean-piece",
        title: "Чистое исполнение",
        description: "Завершите целую пьесу с точностью 100% и без зарегистрированных ошибок",
        target: 1,
    },
    Definition {
        key: "course-graduate",
        title: "Первый выпускной",
        description: "Пройдите все 24 целых урока курса с точностью от 90% без ошибок",
        target: 24,
    },
    Definition {
        key: "thirty-days",
        title: "Музыкальная привычка",
        description: "Занимайтесь в 30 разных календарных дней с верными нотами",
        target: 30,
    },
];
pub struct State {
    pub progress: [u32; 16],
    pub earned: Map<String, Value>,
}
/// Rows must be sorted chronologically. The integer is a local day number; level is legacy fallback.
pub fn evaluate(rows: &[(Attempt, i64, u8)], mut earned: Map<String, Value>) -> State {
    let mut progress = [0u32; 16];
    let mut levels = BTreeSet::new();
    let mut pieces = BTreeSet::new();
    let mut lessons = BTreeSet::new();
    let mut days = BTreeSet::new();
    let mut attempts = BTreeSet::new();
    let mut last_day = None;
    let mut streak = 0;
    for (row, day, fallback_level) in rows {
        if !attempts.insert(&row.id) {
            continue;
        }
        let r = &row.report;
        if r["hit"].as_u64().unwrap_or(0) == 0 {
            continue;
        }
        progress[0] = 1;
        let hit = r["hit"].as_u64().unwrap_or(0).min(u64::from(u32::MAX)) as u32;
        progress[6] = progress[6].saturating_add(hit);
        progress[7] = progress[7].saturating_add(hit);
        days.insert(*day);
        progress[15] = days.len() as u32;
        let complete = r["complete"].as_bool() == Some(true);
        let accuracy = r["accuracy"]
            .as_f64()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);
        if complete && accuracy >= 80.0 {
            if r["hand"] == "L" {
                progress[10] = 1;
            }
            if r["hand"] == "R" {
                progress[11] = 1;
            }
        }
        if complete && r["full_song"].as_bool() == Some(true) {
            if accuracy >= 60.0
                && let Some(id) = row.song_id.as_deref()
            {
                pieces.insert(id);
            }
            progress[8] = pieces.len() as u32;
            progress[9] = pieces.len() as u32;
            if accuracy >= 80.0 && r["mode"] == "Ритм" {
                progress[12] = 1;
            }
            if accuracy == 100.0 && r["mistakes"].as_array().is_some_and(Vec::is_empty) {
                progress[13] = 1;
            }
            if accuracy >= 90.0
                && r["mistakes"].as_array().is_some_and(Vec::is_empty)
                && let Some(id) = row.song_id.as_deref()
                && let Some(number) = id
                    .strip_prefix("course-")
                    .and_then(|n| n.parse::<u32>().ok())
                && (1..=24).contains(&number)
            {
                lessons.insert(number);
            }
            progress[14] = lessons.len() as u32;
        }

        if last_day != Some(*day) {
            streak = if last_day == Some(day - 1) {
                streak + 1
            } else {
                1
            };
            last_day = Some(*day);
            progress[4] = progress[4].max(streak);
        }
        let level = r
            .get("difficulty")
            .and_then(Value::as_u64)
            .unwrap_or(u64::from(*fallback_level));
        if (1..=5).contains(&level) {
            levels.insert(level);
        }
        progress[5] = levels.len() as u32;
        if r["complete"].as_bool() == Some(true) && r["full_song"].as_bool() == Some(true) {
            let accuracy = r["accuracy"]
                .as_f64()
                .filter(|v| v.is_finite())
                .unwrap_or(0.0);
            progress[2] = progress[2].max(accuracy.clamp(0.0, 100.0) as u32);
            if accuracy >= 60.0 {
                progress[1] = 1;
            }
            if accuracy >= 80.0 && r["both_hands"].as_bool() == Some(true) {
                progress[3] = 1;
            }
        }
        for (i, def) in DEFINITIONS.iter().enumerate() {
            if progress[i] >= def.target && !earned.contains_key(def.key) {
                earned.insert(def.key.into(), serde_json::json!(row.created));
            }
        }
    }
    for (p, def) in progress.iter_mut().zip(&DEFINITIONS) {
        *p = (*p).min(def.target);
    }
    State { progress, earned }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(day: i64, level: u8, full: bool, accuracy: f64, both: bool) -> (Attempt, i64, u8) {
        (
            Attempt {
                id: format!("{day}-{level}"),
                date: String::new(),
                song_id: None,
                created: day as f64 * 86400.0,
                report: serde_json::json!({"hit":1,"complete":true,"full_song":full,"accuracy":accuracy,"both_hands":both,"difficulty":level}),
            },
            day,
            0,
        )
    }
    #[test]
    fn all_six_with_first_dates_and_duplicate_days() {
        let mut rows = vec![];
        for day in 1..=7 {
            rows.push(row(day, ((day - 1) % 5 + 1) as u8, true, 95.0, true));
            rows.push(row(day, 1, true, 100.0, true));
        }
        let s = evaluate(&rows, Map::new());
        assert_eq!(s.earned.len(), 6);
        assert_eq!(s.earned["seven-days"], 7.0 * 86400.0);
        assert_eq!(s.earned["explorer"], 5.0 * 86400.0);
        let again = evaluate(&rows, s.earned.clone());
        assert_eq!(again.earned, s.earned);
    }
    #[test]
    fn all_ten_new_awards_require_real_progress_and_dates_are_stable() {
        let mut rows = vec![];
        for day in 1..=30 {
            let mut item = row(day, ((day - 1) % 5 + 1) as u8, true, 100.0, true);
            item.0.song_id = Some(format!("course-{}", (day - 1) % 24 + 1));
            item.0.report["hit"] = 40.into();
            item.0.report["mode"] = "Ритм".into();
            item.0.report["mistakes"] = serde_json::json!([]);
            rows.push(item);
        }
        for (hand, day) in [("L", 31), ("R", 32)] {
            let mut item = row(day, 1, false, 85.0, false);
            item.0.report["hand"] = hand.into();
            rows.push(item);
        }
        let state = evaluate(&rows, Map::new());
        assert_eq!(state.earned.len(), 16);
        assert_eq!(state.earned["hundred-notes"], 3.0 * 86400.0);
        assert_eq!(state.earned["thousand-notes"], 25.0 * 86400.0);
        assert_eq!(state.earned["course-graduate"], 24.0 * 86400.0);
        assert_eq!(state.earned["thirty-days"], 30.0 * 86400.0);
        assert_eq!(evaluate(&rows, state.earned.clone()).earned, state.earned);
        let duplicate = vec![rows[0].clone(); 100];
        let repeated = evaluate(&duplicate, Map::new());
        assert_eq!(repeated.progress[6], 40);
        assert_eq!(repeated.progress[8], 1);
        assert_eq!(repeated.progress[14], 1);
        let mut silent = rows.clone();
        for (r, _, _) in &mut silent {
            r.report["hit"] = 0.into();
        }
        assert!(evaluate(&silent, Map::new()).earned.is_empty());
    }
    #[test]
    fn fragments_gaps_and_silent_days_do_not_qualify() {
        let mut rows = vec![row(1, 1, false, 100.0, true), row(3, 2, false, 100.0, true)];
        let mut silent = row(4, 3, true, 100.0, true);
        silent.0.report["hit"] = 0.into();
        rows.push(silent);
        rows.push(row(5, 4, false, 100.0, true));
        let s = evaluate(&rows, Map::new());
        assert_eq!(&s.progress[..6], &[1, 0, 0, 0, 1, 3]);
        assert_eq!(s.earned.len(), 1);
    }
}
