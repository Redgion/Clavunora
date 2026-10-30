// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub pitch: u8,
    pub start: f64,
    pub duration: f64,
    #[serde(default = "default_velocity")]
    pub velocity: u8,
    #[serde(default)]
    pub track: usize,
    #[serde(default)]
    pub channel: u8,
    #[serde(default)]
    pub hand: String,
    #[serde(default)]
    pub finger: u8,
}
fn default_velocity() -> u8 {
    88
}
impl Note {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Song {
    pub title: String,
    pub notes: Vec<Note>,
    #[serde(default = "default_bpm")]
    pub bpm: f64,
    #[serde(default = "default_meter")]
    pub time_signature: [u32; 2],
    #[serde(default = "default_tracks")]
    pub track_names: Vec<String>,
    #[serde(default)]
    pub difficulty: u8,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub work: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub tempo_map: Vec<[f64; 2]>,
}
fn default_bpm() -> f64 {
    96.0
}
fn default_meter() -> [u32; 2] {
    [3, 4]
}
fn default_tracks() -> Vec<String> {
    vec!["Мелодия".into()]
}
impl Song {
    pub fn from_json(raw: &str) -> Result<Self> {
        let song: Self = serde_json::from_str(raw)?;
        song.validate()?;
        Ok(song)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.title.trim().is_empty() && self.title.chars().count() <= 500,
            "Некорректное название"
        );
        ensure!(
            !self.notes.is_empty() && self.notes.len() <= 100_000,
            "Некорректное количество нот"
        );
        ensure!(
            self.difficulty <= 5 && self.bpm.is_finite() && self.bpm > 0.0,
            "Некорректная сложность или темп"
        );
        ensure!(
            self.time_signature.iter().all(|v| *v > 0),
            "Некорректный размер"
        );
        ensure!(
            !self.track_names.is_empty()
                && self.track_names.iter().all(|s| s.chars().count() <= 1000),
            "Некорректные дорожки"
        );
        for text in [&self.composer, &self.work, &self.description, &self.source] {
            ensure!(
                text.chars().count() <= 100_000,
                "Слишком длинные метаданные"
            );
        }
        for n in &self.notes {
            ensure!(
                n.pitch <= 127 && n.velocity <= 127 && n.channel <= 15 && n.track <= 10_000,
                "Некорректная MIDI-нота"
            );
            ensure!(
                n.start.is_finite()
                    && n.start >= 0.0
                    && n.duration.is_finite()
                    && n.duration > 0.0
                    && n.end().is_finite(),
                "Некорректное время ноты"
            );
            ensure!(
                ["", "L", "R"].contains(&n.hand.as_str()) && n.finger <= 5,
                "Некорректная аппликатура"
            );
        }
        for [time, bpm] in &self.tempo_map {
            ensure!(
                time.is_finite() && *time >= 0.0 && bpm.is_finite() && *bpm > 0.0,
                "Некорректная карта темпа"
            );
        }
        Ok(())
    }
    pub fn duration(&self) -> f64 {
        self.notes.iter().map(Note::end).fold(0.0, f64::max)
    }
}

pub fn note_name(pitch: u8) -> String {
    let names = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "B♭", "B",
    ];
    format!(
        "{}{}",
        names[pitch as usize % 12],
        i16::from(pitch) / 12 - 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_validation() {
        let song =
            Song::from_json(r#"{"title":"Пьеса","notes":[{"pitch":60,"start":0,"duration":0.5}]}"#)
                .unwrap();
        assert_eq!(song.duration(), 0.5);
        assert_eq!(song.notes[0].velocity, 88);
        assert!(
            Song::from_json(r#"{"title":"x","notes":[{"pitch":128,"start":0,"duration":1}]}"#)
                .is_err()
        );
        assert!(
            Song::from_json(r#"{"title":"x","notes":[{"pitch":60,"start":-1,"duration":1}]}"#)
                .is_err()
        );
        assert!(Song::from_json(r#"{"title":"x","notes":[]}"#).is_err());
    }
    #[test]
    fn names_match_python() {
        assert_eq!(note_name(21), "A0");
        assert_eq!(note_name(108), "C8");
    }
}
