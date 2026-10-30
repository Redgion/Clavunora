// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Physical-key scoring in musical seconds. Matching radius and tolerances use wall time.
use crate::{
    midi::{Message, MidiEvent},
    score::Note,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MistakeKind {
    #[serde(rename = "Лишняя нота")]
    Extra,
    #[serde(rename = "Поздно")]
    Late,
    #[serde(rename = "Рано")]
    Early,
    #[serde(rename = "Долгое удержание")]
    LongHold,
    #[serde(rename = "Короткое удержание")]
    ShortHold,
    #[serde(rename = "Пропущена")]
    Missed,
    #[serde(rename = "Не отпущена")]
    Unreleased,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Mistake {
    pub position: f64,
    pub pitch: u8,
    pub kind: MistakeKind,
    pub delta: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Outcome {
    pub complete: bool,
    pub total: usize,
    pub hit: usize,
    pub accuracy: f64,
    pub mistakes: Vec<Mistake>,
}
impl Outcome {
    pub fn new(complete: bool, total: usize, hit: usize, mistakes: Vec<Mistake>) -> Self {
        let accuracy = format!("{:.1}", 100.0 * hit as f64 / total.max(1) as f64)
            .parse()
            .expect("Finite ratio");
        Self {
            complete,
            total,
            hit,
            accuracy,
            mistakes,
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.total > 0 && self.hit <= self.total,
            "Некорректное количество нот результата"
        );
        ensure!(
            self.accuracy.is_finite() && (0.0..=100.0).contains(&self.accuracy),
            "Некорректная точность"
        );
        for m in &self.mistakes {
            ensure!(
                m.pitch <= 127 && m.position.is_finite() && m.delta.is_finite(),
                "Некорректная ошибка исполнения"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Options {
    pub speed: f64,
    pub tolerance: f64,
    pub latency: f64,
    pub start: f64,
    pub end: Option<f64>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            speed: 1.0,
            tolerance: 0.15,
            latency: 0.0,
            start: 0.0,
            end: None,
        }
    }
}
type Key = (String, u8, u8);
#[derive(Debug)]
pub struct Performance {
    notes: Vec<Note>,
    pitch_index: [Vec<(f64, usize)>; 128],
    ignore_release: BTreeSet<usize>,
    hit: BTreeSet<usize>,
    active: Vec<(Key, VecDeque<usize>)>,
    active_index: BTreeMap<Key, usize>,
    mistakes: Vec<Mistake>,
    options: Options,
    finished: Option<Outcome>,
}
impl Performance {
    pub fn new(notes: &[Note], options: Options) -> Result<Self> {
        ensure!(
            options.speed.is_finite() && (0.25..=2.0).contains(&options.speed),
            "Некорректная скорость"
        );
        ensure!(
            options.tolerance.is_finite() && (0.03..=1.0).contains(&options.tolerance),
            "Некорректный допуск"
        );
        ensure!(
            options.latency.is_finite() && (-0.5..=0.5).contains(&options.latency),
            "Некорректная задержка"
        );
        ensure!(
            options.start.is_finite() && options.start >= 0.0,
            "Некорректное начало"
        );
        if let Some(end) = options.end {
            ensure!(
                end.is_finite() && end > options.start,
                "Некорректная граница"
            );
        }
        let mut physical: BTreeMap<(u8, String), Note> = BTreeMap::new();
        for note in notes {
            ensure!(
                note.pitch <= 127
                    && note.start.is_finite()
                    && note.start >= 0.0
                    && note.duration.is_finite()
                    && note.duration > 0.0
                    && note.end().is_finite(),
                "Некорректная нота"
            );
            if note.start < options.start || options.end.is_some_and(|end| note.start >= end) {
                continue;
            }
            // Decimal rounding, like Python round(start, 7), including binary-float tie behavior.
            let key = (note.pitch, format!("{:.7}", note.start));
            if physical
                .get(&key)
                .is_none_or(|existing| note.duration > existing.duration)
            {
                physical.insert(key, note.clone());
            }
        }
        let mut notes: Vec<Note> = physical.into_values().collect();
        notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
        let mut pitch_index = std::array::from_fn(|_| Vec::new());
        let mut ignore_release = BTreeSet::new();
        for (i, n) in notes.iter().enumerate() {
            pitch_index[n.pitch as usize].push((n.start, i));
            if options.end.is_some_and(|end| n.end() >= end) {
                ignore_release.insert(i);
            }
        }
        Ok(Self {
            notes,
            pitch_index,
            ignore_release,
            hit: BTreeSet::new(),
            active: vec![],
            active_index: BTreeMap::new(),
            mistakes: vec![],
            options,
            finished: None,
        })
    }
    pub fn feed(&mut self, event: &MidiEvent, position: f64) -> Result<()> {
        if self.finished.is_some() {
            return Ok(());
        }
        ensure!(position.is_finite(), "Некорректное время ввода");
        let position = position - self.options.latency * self.options.speed;
        let tolerance = self.options.tolerance * self.options.speed;
        let (pitch, on) = match event.message {
            Message::NoteOn { pitch, velocity } => (pitch, velocity > 0),
            Message::NoteOff { pitch } => (pitch, false),
            Message::Control { .. } => return Ok(()),
        };
        ensure!(
            pitch <= 127 && event.channel <= 15,
            "Некорректное MIDI-событие"
        );
        let key = (event.source.clone(), event.channel, pitch);
        if on {
            let index = &self.pitch_index[pitch as usize];
            let radius = (0.5 * self.options.speed).max(tolerance);
            let left = index.partition_point(|(time, _)| *time < position - radius);
            let right = index.partition_point(|(time, _)| *time <= position + radius);
            let best = index[left..right]
                .iter()
                .filter(|(_, i)| !self.hit.contains(i))
                .min_by(|(ta, ia), (tb, ib)| {
                    (*ta - position)
                        .abs()
                        .total_cmp(&(*tb - position).abs())
                        .then(ia.cmp(ib))
                })
                .map(|(_, i)| *i);
            let Some(i) = best else {
                self.mistakes.push(Mistake {
                    position: position.max(0.0),
                    pitch,
                    kind: MistakeKind::Extra,
                    delta: 0.0,
                });
                return Ok(());
            };
            self.hit.insert(i);
            let lane = *self.active_index.entry(key.clone()).or_insert_with(|| {
                self.active.push((key, VecDeque::new()));
                self.active.len() - 1
            });
            self.active[lane].1.push_back(i);
            let n = &self.notes[i];
            let delta = position - n.start;
            if delta.abs() > tolerance {
                self.mistakes.push(Mistake {
                    position: n.start,
                    pitch,
                    kind: if delta > 0.0 {
                        MistakeKind::Late
                    } else {
                        MistakeKind::Early
                    },
                    delta: delta / self.options.speed,
                });
            }
        } else if let Some(&lane) = self.active_index.get(&key)
            && let Some(i) = self.active[lane].1.pop_front()
        {
            let n = &self.notes[i];
            let delta = position - n.end();
            if !self.ignore_release.contains(&i)
                && delta.abs() > tolerance.max(0.2 * self.options.speed)
            {
                self.mistakes.push(Mistake {
                    position: n.start,
                    pitch,
                    kind: if delta > 0.0 {
                        MistakeKind::LongHold
                    } else {
                        MistakeKind::ShortHold
                    },
                    delta: delta / self.options.speed,
                });
            }
        }
        Ok(())
    }
    pub fn finish(&mut self, complete: bool) -> Outcome {
        if let Some(report) = &self.finished {
            return report.clone();
        }
        if complete {
            for (i, n) in self.notes.iter().enumerate() {
                if !self.hit.contains(&i) {
                    self.mistakes.push(Mistake {
                        position: n.start,
                        pitch: n.pitch,
                        kind: MistakeKind::Missed,
                        delta: 0.0,
                    });
                }
            }
            for (_, indices) in &self.active {
                for i in indices {
                    if !self.ignore_release.contains(i) {
                        let n = &self.notes[*i];
                        self.mistakes.push(Mistake {
                            position: n.start,
                            pitch: n.pitch,
                            kind: MistakeKind::Unreleased,
                            delta: 0.0,
                        });
                    }
                }
            }
        }
        let report = Outcome::new(
            complete,
            self.notes.len(),
            self.hit.len(),
            self.mistakes.clone(),
        );
        self.finished = Some(report.clone());
        report
    }
    pub fn total(&self) -> usize {
        self.notes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[derive(Deserialize)]
    struct Input {
        bytes: Vec<u8>,
        source: String,
        position: f64,
    }
    #[derive(Deserialize)]
    struct Case {
        name: String,
        notes: Vec<Note>,
        options: Options,
        events: Vec<Input>,
        complete: bool,
        expected: Value,
    }
    fn compare(a: &Value, b: &Value, path: &str) {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-9,
                "{path}: {a} != {b}"
            ),
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    compare(a, b, &format!("{path}/{i}"));
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (key, a) in a {
                    compare(a, &b[key], &format!("{path}/{key}"));
                }
            }
            _ => assert_eq!(a, b, "{path}"),
        }
    }
    #[test]
    fn matches_python_reference_scenarios() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../tests/fixtures/python-performance.json"))
                .unwrap();
        assert_eq!(cases.len(), 170);
        for c in cases {
            let mut p = Performance::new(&c.notes, c.options).unwrap();
            for event in c.events {
                p.feed(
                    &MidiEvent::parse(&event.bytes, &event.source).unwrap(),
                    event.position,
                )
                .unwrap();
            }
            let report = p.finish(c.complete);
            compare(
                &serde_json::to_value(&report).unwrap(),
                &c.expected,
                &c.name,
            );
            assert_eq!(p.finish(c.complete), report);
        }
    }
    #[test]
    fn finalization_is_immutable_and_late_input_is_ignored() {
        let notes = crate::catalog::course()[0].1.notes.clone();
        let mut p = Performance::new(&notes, Options::default()).unwrap();
        let result = p.finish(false);
        p.feed(&MidiEvent::parse(&[0x90, 60, 90], "test").unwrap(), 0.0)
            .unwrap();
        assert_eq!(p.finish(true), result);
    }
    #[test]
    fn long_score_is_matched_once_per_physical_key() {
        let mut note = crate::catalog::course()[0].1.notes[0].clone();
        note.duration = 0.07;
        let notes: Vec<_> = (0..3000)
            .map(|i| {
                let mut n = note.clone();
                n.pitch = 60 + (i % 12) as u8;
                n.start = i as f64 * 0.1;
                n
            })
            .collect();
        let mut p = Performance::new(
            &notes,
            Options {
                tolerance: 0.03,
                ..Default::default()
            },
        )
        .unwrap();
        for n in &notes {
            p.feed(
                &MidiEvent::parse(&[0x90, n.pitch, 90], "midi").unwrap(),
                n.start,
            )
            .unwrap();
            p.feed(
                &MidiEvent::parse(&[0x80, n.pitch, 0], "midi").unwrap(),
                n.end(),
            )
            .unwrap();
        }
        let result = p.finish(true);
        assert_eq!(result.hit, 3000);
        assert_eq!(result.accuracy, 100.0);
        assert!(result.mistakes.is_empty());
    }
}
