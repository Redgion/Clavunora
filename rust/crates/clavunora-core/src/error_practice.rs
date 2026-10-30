// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Prepare a repeat around one saved mistake, without editing the score or old report.
use crate::{
    lesson::Plan,
    session::Mode,
    store::{Attempt, Record},
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;
pub struct Repair {
    pub plan: Plan,
    pub speed: f64,
    pub mode: Mode,
    pub position: f64,
    pub kind: String,
    pub pitch: u8,
}
pub fn check_version(record: &Record, attempt: &Attempt) -> Result<()> {
    ensure!(
        attempt.song_id.as_deref() == Some(record.id.as_str()),
        "Пьеса этой попытки недоступна"
    );
    let version = attempt.report["score_version"]
        .as_str()
        .filter(|v| !v.is_empty())
        .context("В старом результате нет версии нот. Автоматический переход недоступен")?;
    ensure!(
        version == record.score_version,
        "Ноты или разметка пьесы изменились после этого занятия. Автоматический переход отключён"
    );
    Ok(())
}
pub fn from_attempt(record: &Record, attempt: &Attempt, index: usize) -> Result<Repair> {
    check_version(record, attempt)?;
    let r = &attempt.report;
    let error = r["mistakes"]
        .as_array()
        .and_then(|v| v.get(index))
        .context("Ошибка больше не найдена в результате")?;
    let position = error["position"]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0 && *v <= record.song.duration() + 1.0)
        .context("Некорректное время ошибки")?;
    let pitch = error["pitch"]
        .as_u64()
        .filter(|p| *p <= 127)
        .context("Некорректная нота ошибки")? as u8;
    let kind = error["kind"]
        .as_str()
        .context("Не указан вид ошибки")?
        .to_owned();
    let hand = r["hand"].as_str().unwrap_or("").to_owned();
    let tracks = if r["tracks"].is_null() {
        None
    } else {
        let array = r["tracks"]
            .as_array()
            .context("Некорректные дорожки результата")?;
        Some(
            array
                .iter()
                .map(|v| {
                    v.as_u64()
                        .filter(|t| *t <= 10_000)
                        .map(|t| t as usize)
                        .context("Некорректная дорожка результата")
                })
                .collect::<Result<BTreeSet<_>>>()?,
        )
    };
    let start = r["start"]
        .as_f64()
        .filter(|v| v.is_finite())
        .context("Нет границ исходного занятия")?;
    let end = r["end"]
        .as_f64()
        .filter(|v| v.is_finite())
        .context("Нет границ исходного занятия")?
        .min(record.song.duration());
    let original = Plan {
        hand,
        tracks,
        start,
        end: Some(end),
        ..Default::default()
    };
    original.validate(&record.song)?;
    let at = position.clamp(start, end);
    let bpm = record
        .song
        .tempo_map
        .iter()
        .filter(|t| t[0] <= at)
        .max_by(|a, b| a[0].total_cmp(&b[0]))
        .map_or(record.song.bpm, |t| t[1]);
    let padding = (120.0 / bpm).clamp(0.75, 3.0);
    let selected = record
        .song
        .notes
        .iter()
        .filter(|n| original.includes(n) && n.start >= start && n.start < end);
    // Prefer the reported pitch only if it occurs near this time. Wrong extra
    // pitches must not jump to a distant occurrence elsewhere in the piece.
    let anchor = selected
        .clone()
        .filter(|n| {
            n.pitch == pitch
                && ((n.start - at).abs() <= padding || (n.start <= at && n.end() >= at))
        })
        .min_by(|a, b| (a.start - at).abs().total_cmp(&(b.start - at).abs()))
        .or_else(|| selected.min_by(|a, b| (a.start - at).abs().total_cmp(&(b.start - at).abs())))
        .context("В этой партии нет нот для разучивания")?;
    let plan = Plan {
        start: (at - padding).max(start).min(anchor.start),
        end: Some((at + padding).min(end).max(anchor.end().min(end))),
        repeat: true,
        smart: false,
        ..original
    };
    plan.validate(&record.song)?;
    let speed = r["speed"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= 2.0)
        .context("Некорректная скорость исходного занятия")?;
    let mode = match r["mode"].as_str() {
        Some("Ожидание") => Mode::Waiting,
        Some("Ритм") => Mode::Rhythm,
        _ => anyhow::bail!("Неизвестный режим исходного занятия"),
    };
    Ok(Repair {
        plan,
        speed: (speed * 0.75).clamp(0.25, 1.0),
        mode,
        position,
        kind,
        pitch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Record, Attempt) {
        let song = crate::catalog::course()
            .into_iter()
            .find(|(id, _)| id == "course-5")
            .unwrap()
            .1;
        let version = crate::version::python_hash(&serde_json::to_string(&song).unwrap()).unwrap();
        let record = Record {
            id: "course-5".into(),
            song,
            favorite: false,
            score_version: version.clone(),
        };
        let attempt = Attempt {
            id: "a".into(),
            song_id: Some(record.id.clone()),
            created: 1.0,
            date: String::new(),
            report: serde_json::json!({"score_version":version,"start":1.0,"end":3.8,"hand":"L","tracks":[0],"speed":0.8,"mode":"Ритм","mistakes":[{"position":2.0,"pitch":48,"kind":"Поздно"}]}),
        };
        (record, attempt)
    }
    #[test]
    fn preserves_selection_and_bounds_and_slows_without_changing_source() {
        let (record, attempt) = fixture();
        let original = record.song.clone();
        let raw = attempt.report.clone();
        let repair = from_attempt(&record, &attempt, 0).unwrap();
        assert_eq!(repair.plan.hand, "L");
        assert_eq!(repair.plan.tracks, Some(BTreeSet::from([0])));
        assert!(repair.plan.start >= 1.0 && repair.plan.end(&record.song) <= 3.8);
        assert!((repair.speed - 0.6).abs() < 1e-9);
        assert_eq!(repair.mode, Mode::Rhythm);
        assert!(repair.plan.repeat && !repair.plan.smart);
        assert!(
            repair
                .plan
                .selected(&record.song)
                .unwrap()
                .notes
                .iter()
                .all(|n| n.hand == "L" && n.track == 0)
        );
        assert_eq!(record.song, original);
        assert_eq!(attempt.report, raw);
    }
    #[test]
    fn rejects_stale_unknown_versions_deleted_identity_and_invalid_selection() {
        let (record, attempt) = fixture();
        for n in 0..5 {
            let mut a = attempt.clone();
            match n {
                0 => a.report["score_version"] = "old".into(),
                1 => {
                    a.report.as_object_mut().unwrap().remove("score_version");
                }
                2 => a.song_id = None,
                3 => a.report["tracks"] = serde_json::json!([99]),
                _ => a.report["hand"] = "bad".into(),
            }
            assert!(from_attempt(&record, &a, 0).is_err());
        }
        assert!(from_attempt(&record, &attempt, 99).is_err());
    }
    #[test]
    fn first_last_and_extra_notes_produce_nonempty_clamped_fragments() {
        let (record, mut a) = fixture();
        a.report["hand"] = "".into();
        a.report["tracks"] = serde_json::Value::Null;
        a.report["start"] = 0.0.into();
        a.report["speed"] = 0.25.into();
        for position in [0.0, 3.8, 4.2] {
            a.report["mistakes"][0]["position"] = position.into();
            a.report["mistakes"][0]["pitch"] = 100.into();
            let p = from_attempt(&record, &a, 0).unwrap();
            assert_eq!(p.speed, 0.25);
            assert!(!p.plan.selected(&record.song).unwrap().notes.is_empty());
        }
    }
}
