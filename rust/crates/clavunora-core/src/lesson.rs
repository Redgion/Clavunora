// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Lesson selections and smart repetitions, independent of the user interface.
use crate::score::Song;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Plan {
    pub hand: String,
    pub tracks: Option<BTreeSet<usize>>,
    pub start: f64,
    pub end: Option<f64>,
    pub repeat: bool,
    pub smart: bool,
}
impl Plan {
    pub fn end(&self, song: &Song) -> f64 {
        self.end.unwrap_or(song.duration())
    }
    pub fn validate(&self, song: &Song) -> Result<()> {
        song.validate()?;
        let end = self.end(song);
        ensure!(
            ["", "L", "R"].contains(&self.hand.as_str()),
            "Неизвестная партия руки"
        );
        ensure!(
            self.start.is_finite()
                && end.is_finite()
                && self.start >= 0.0
                && end > self.start
                && end <= song.duration(),
            "Фрагмент должен находиться внутри пьесы и иметь положительную длину"
        );
        if let Some(tracks) = &self.tracks {
            ensure!(!tracks.is_empty(), "Выберите хотя бы одну дорожку");
            ensure!(
                tracks
                    .iter()
                    .all(|track| song.notes.iter().any(|n| n.track == *track)),
                "В пьесе нет выбранной дорожки"
            );
        }
        ensure!(
            song.notes
                .iter()
                .any(|n| self.includes(n) && n.start >= self.start && n.start < end),
            "В выбранной партии и фрагменте нет нот"
        );
        Ok(())
    }
    pub fn includes(&self, n: &crate::score::Note) -> bool {
        (self.hand.is_empty() || n.hand == self.hand)
            && self
                .tracks
                .as_ref()
                .is_none_or(|tracks| tracks.contains(&n.track))
    }
    pub fn selected(&self, song: &Song) -> Result<Song> {
        self.validate(song)?;
        let end = self.end(song);
        let mut selected = song.clone();
        selected
            .notes
            .retain(|n| self.includes(n) && n.start >= self.start && n.start < end);
        Ok(selected)
    }
    pub fn whole(&self, song: &Song) -> bool {
        self.hand.is_empty()
            && self.tracks.is_none()
            && self.start == 0.0
            && self.end(song) == song.duration()
            && !self.repeat
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SmartConfig {
    pub threshold: f64,
    pub repetitions: u32,
    pub step: f64,
    pub target: f64,
}
impl Default for SmartConfig {
    fn default() -> Self {
        Self {
            threshold: 90.0,
            repetitions: 3,
            step: 0.05,
            target: 1.0,
        }
    }
}
impl SmartConfig {
    pub fn valid(&self) -> bool {
        self.threshold.is_finite()
            && (0.0..=100.0).contains(&self.threshold)
            && (1..=20).contains(&self.repetitions)
            && self.step.is_finite()
            && (0.01..=0.25).contains(&self.step)
            && self.target.is_finite()
            && (0.25..=2.0).contains(&self.target)
    }
}
#[derive(Default)]
pub struct RepeatProgress {
    pub streak: u32,
    pub completed: u32,
}
impl RepeatProgress {
    /// Matches Python LoopProgress; mistakes reduce effective accuracy before this call.
    pub fn next_speed(&mut self, speed: f64, accuracy: f64, config: &SmartConfig) -> f64 {
        self.completed = self.completed.saturating_add(1);
        self.streak = if accuracy >= config.threshold {
            self.streak + 1
        } else {
            0
        };
        if self.streak >= config.repetitions {
            self.streak = 0;
            if speed < config.target {
                (speed + config.step).min(config.target)
            } else {
                speed
            }
        } else {
            speed
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_rejects_empty_and_invalid_ranges_and_preserves_original_score() {
        let song = crate::catalog::course()
            .into_iter()
            .find(|(id, _)| id == "course-5")
            .unwrap()
            .1;
        let p = Plan {
            hand: "L".into(),
            ..Default::default()
        };
        let selected = p.selected(&song).unwrap();
        assert!(selected.notes.iter().all(|n| n.hand == "L"));
        assert!(song.notes.iter().any(|n| n.hand == "R"));
        assert!(!p.whole(&song));
        assert!(
            Plan {
                tracks: Some(BTreeSet::new()),
                ..Default::default()
            }
            .validate(&song)
            .is_err()
        );
        assert!(
            Plan {
                tracks: Some(BTreeSet::from([999])),
                ..Default::default()
            }
            .validate(&song)
            .is_err()
        );
        assert!(
            Plan {
                start: 1.0,
                end: Some(1.0),
                ..Default::default()
            }
            .validate(&song)
            .is_err()
        );
        assert!(
            Plan {
                start: f64::NAN,
                ..Default::default()
            }
            .validate(&song)
            .is_err()
        );
        let p = Plan {
            start: 1.0,
            end: Some(2.0),
            ..Default::default()
        };
        assert!(
            p.selected(&song)
                .unwrap()
                .notes
                .iter()
                .all(|n| n.start >= 1.0 && n.start < 2.0)
        );
    }
    #[test]
    fn smart_repeat_requires_consecutive_success_caps_target_and_never_slows_existing_speed() {
        let c = SmartConfig::default();
        let mut p = RepeatProgress::default();
        assert_eq!(p.next_speed(0.5, 100.0, &c), 0.5);
        assert_eq!(p.next_speed(0.5, 80.0, &c), 0.5);
        assert_eq!(p.streak, 0);
        for _ in 0..2 {
            assert_eq!(p.next_speed(0.5, 95.0, &c), 0.5);
        }
        assert_eq!(p.next_speed(0.5, 95.0, &c), 0.55);
        for _ in 0..2 {
            p.next_speed(0.98, 95.0, &c);
        }
        assert_eq!(p.next_speed(0.98, 95.0, &c), 1.0);
        for _ in 0..2 {
            p.next_speed(1.5, 95.0, &c);
        }
        assert_eq!(p.next_speed(1.5, 95.0, &c), 1.5);
    }
}
