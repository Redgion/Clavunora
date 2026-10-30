// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Educational notation derived from pitch/time data, not an original engraved score.
use crate::{
    playback::BeatGrid,
    score::{Note, Song},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Russian,
    #[default]
    English,
}
impl Language {
    pub fn name(self, pitch: u8) -> String {
        let classes = match self {
            Self::Russian => [
                "До",
                "До#",
                "Ре",
                "Ре#",
                "Ми",
                "Фа",
                "Фа#",
                "Соль",
                "Соль#",
                "Ля",
                "Ля#",
                "Си",
            ],
            Self::English => [
                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
            ],
        };
        format!(
            "{}{}",
            classes[usize::from(pitch % 12)],
            i16::from(pitch) / 12 - 1
        )
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum View {
    #[default]
    Falling,
    Sheet,
    Both,
}
impl View {
    pub fn sheet(self) -> bool {
        matches!(self, Self::Sheet | Self::Both)
    }
    pub fn falling(self) -> bool {
        matches!(self, Self::Falling | Self::Both)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub view: View,
    pub language: Language,
}

pub fn diatonic(pitch: u8) -> i32 {
    i32::from(pitch / 12) * 7 + [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6][usize::from(pitch % 12)]
}
pub fn treble(note: &Note) -> bool {
    note.hand == "R" || (note.hand.is_empty() && note.pitch >= 60)
}

#[derive(Clone, Copy, Debug)]
pub struct Page {
    pub index: usize,
    pub count: usize,
    pub start: f64,
    pub end: f64,
    pub beats_per_bar: f64,
}
impl Page {
    pub fn new(
        song: &Song,
        grid: &BeatGrid,
        width: f32,
        position: f64,
        manual: Option<usize>,
    ) -> Self {
        let bars = ((width - 120.0) / 260.0).floor().clamp(1.0, 4.0) as usize;
        let beats_per_bar = f64::from(song.time_signature[0]);
        let span = bars as f64 * beats_per_bar;
        let total = grid.beat_position(song.duration()).max(0.0);
        let count = ((total / span).ceil() as usize).max(1);
        let index = manual
            .unwrap_or_else(|| {
                (grid.beat_position(position.max(0.0)) / span)
                    .floor()
                    .max(0.0) as usize
            })
            .min(count - 1);
        Self {
            index,
            count,
            start: index as f64 * span,
            end: ((index + 1) as f64 * span)
                .min((total / beats_per_bar).ceil().max(1.0) * beats_per_bar),
            beats_per_bar,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rhythm {
    pub hollow: bool,
    pub stem: bool,
    pub flags: u8,
    pub dotted: bool,
}
pub fn rhythm(quarters: f64) -> Rhythm {
    let values = [
        0.125, 0.1875, 0.25, 0.375, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0,
    ];
    let value = values
        .into_iter()
        .min_by(|a, b| {
            (quarters / a)
                .ln()
                .abs()
                .total_cmp(&(quarters / b).ln().abs())
        })
        .unwrap();
    let dotted = [0.1875, 0.375, 0.75, 1.5, 3.0, 6.0].contains(&value);
    let base = if dotted { value / 1.5 } else { value };
    Rhythm {
        hollow: base >= 2.0,
        stem: base < 4.0,
        flags: if base < 0.2 {
            3
        } else if base < 0.4 {
            2
        } else if base < 0.8 {
            1
        } else {
            0
        },
        dotted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_octaves_accidentals_and_positions_agree_in_both_languages() {
        assert_eq!(Language::Russian.name(60), "До4");
        assert_eq!(Language::English.name(60), "C4");
        assert_eq!(Language::Russian.name(70), "Ля#4");
        assert_eq!(Language::English.name(71), "B4");
        assert_eq!(Language::Russian.name(0), "До-1");
        assert_eq!(Language::English.name(127), "G9");
        assert_eq!(diatonic(60), diatonic(61));
        assert_eq!(diatonic(64) - diatonic(60), 2);
        assert_eq!(diatonic(72) - diatonic(60), 7);
    }
    #[test]
    fn tempo_grid_pages_and_note_values_include_changes_and_boundaries() {
        let song=Song::from_json(r#"{"title":"page","bpm":120,"time_signature":[4,4],"tempo_map":[[2,60]],"notes":[{"pitch":60,"start":0,"duration":10}]}"#).unwrap();
        let grid = BeatGrid::new(&song);
        for t in [-1.0, 0.0, 1.0, 2.0, 5.0, 10.0] {
            assert!((grid.time_at_beat(grid.beat_position(t)) - t).abs() < 1e-9);
        }
        let p = Page::new(&song, &grid, 500.0, 2.0, None);
        assert_eq!(p.index, 1);
        assert_eq!(p.start, 4.0);
        assert_eq!(
            Page::new(&song, &grid, 500.0, 0.0, Some(999)).index,
            p.count - 1
        );
        assert!(!rhythm(1.0).hollow);
        assert!(rhythm(2.0).hollow);
        assert!(!rhythm(4.0).stem);
        assert_eq!(rhythm(0.5).flags, 1);
        assert!(rhythm(0.75).dotted);
    }
}
