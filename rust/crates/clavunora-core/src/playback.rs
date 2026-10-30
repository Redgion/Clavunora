// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Audio-only accompaniment and a tempo-aware beat grid. Never feeds scoring/MIDI state.
use crate::{lesson::Plan, score::Song};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub metronome: bool,
    pub accompaniment: bool,
    pub volume: f32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            metronome: false,
            accompaniment: false,
            volume: 0.65,
        }
    }
}
impl Config {
    pub fn valid(&self) -> bool {
        self.volume.is_finite() && (0.0..=1.0).contains(&self.volume)
    }
}

#[derive(Clone, Debug)]
struct Tempo {
    time: f64,
    beats: f64,
    per_second: f64,
}
#[derive(Clone, Debug)]
pub struct BeatGrid {
    segments: Vec<Tempo>,
    numerator: u32,
}
impl BeatGrid {
    /// BPM is quarter notes per minute; clicks follow the denominator of the meter.
    pub fn new(song: &Song) -> Self {
        let unit = f64::from(song.time_signature[1]) / 4.0;
        let mut changes = song.tempo_map.clone();
        // Stable sorting means the last entry at a duplicate time wins.
        changes.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let mut segments = vec![Tempo {
            time: 0.0,
            beats: 0.0,
            per_second: song.bpm / 60.0 * unit,
        }];
        for [time, bpm] in changes {
            let last = segments.last().unwrap();
            let beats = last.beats + (time - last.time) * last.per_second;
            let next = Tempo {
                time,
                beats,
                per_second: bpm / 60.0 * unit,
            };
            if last.time == time {
                *segments.last_mut().unwrap() = next;
            } else {
                segments.push(next);
            }
        }
        Self {
            segments,
            numerator: song.time_signature[0],
        }
    }
    pub fn standalone(bpm: f64, beats: u32) -> Self {
        Self {
            segments: vec![Tempo {
                time: 0.0,
                beats: 0.0,
                per_second: bpm / 60.0,
            }],
            numerator: beats.max(1),
        }
    }
    pub fn beat(&self, position: f64) -> (i64, bool, f64) {
        let beat = self.beat_position(position);
        let number = (beat + 1e-9).floor() as i64;
        (
            number,
            number.rem_euclid(i64::from(self.numerator)) == 0,
            (beat - number as f64).max(0.0),
        )
    }
    pub fn beat_position(&self, position: f64) -> f64 {
        let index = self
            .segments
            .partition_point(|s| s.time <= position)
            .saturating_sub(1);
        let s = &self.segments[index];
        s.beats + (position - s.time) * s.per_second
    }
    pub fn time_at_beat(&self, beat: f64) -> f64 {
        let index = self
            .segments
            .partition_point(|s| s.beats <= beat)
            .saturating_sub(1);
        let s = &self.segments[index];
        s.time + (beat - s.beats) / s.per_second
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Voice {
    pub velocity: u8,
    pub generation: u64,
}
#[derive(Clone, Debug)]
struct Event {
    time: f64,
    index: usize,
    pitch: u8,
    velocity: u8,
    voice: Voice,
}
#[derive(Clone, Debug)]
pub struct Sequence {
    pub grid: BeatGrid,
    events: Vec<Event>,
    pub note_count: usize,
}
impl Sequence {
    pub fn accompaniment(song: &Song, plan: &Plan) -> Self {
        Self::build(song, plan, |n| match plan.hand.as_str() {
            "L" => n.hand == "R",
            "R" => n.hand == "L",
            _ => plan
                .tracks
                .as_ref()
                .is_some_and(|tracks| !tracks.contains(&n.track)),
        })
    }
    pub fn listening(song: &Song, plan: &Plan) -> Self {
        Self::build(song, plan, |n| plan.includes(n) && n.start >= plan.start)
    }
    fn build(song: &Song, plan: &Plan, includes: impl Fn(&crate::score::Note) -> bool) -> Self {
        let mut events = Vec::new();
        let mut count = 0;
        for (index, n) in song.notes.iter().enumerate() {
            // A sustained accompaniment note may overlap the beginning of a fragment.
            if !includes(n) || n.end() <= plan.start || n.start >= plan.end(song) {
                continue;
            }
            count += 1;
            events.push(Event {
                time: n.start.max(plan.start),
                index,
                pitch: n.pitch,
                velocity: n.velocity,
                voice: Voice::default(),
            });
            events.push(Event {
                time: n.end().min(plan.end(song)),
                index,
                pitch: n.pitch,
                velocity: 0,
                voice: Voice::default(),
            });
        }
        events.sort_by(|a, b| {
            a.time
                .total_cmp(&b.time)
                .then(a.velocity.cmp(&b.velocity))
                .then(a.index.cmp(&b.index))
        });
        // Resolve overlapping equal pitches before playback; the audio cursor allocates nothing.
        let mut active: [BTreeSet<(u64, usize)>; 128] = std::array::from_fn(|_| BTreeSet::new());
        for event in &mut events {
            let voices = &mut active[usize::from(event.pitch)];
            let key = (song.notes[event.index].start.abs().to_bits(), event.index);
            if event.velocity == 0 {
                voices.remove(&key);
            } else {
                voices.insert(key);
            }
            event.voice = voices.last().map_or(Voice::default(), |&(_, index)| Voice {
                velocity: song.notes[index].velocity,
                generation: index as u64 + 1,
            });
        }
        Self {
            grid: BeatGrid::new(song),
            events,
            note_count: count,
        }
    }
    pub fn standalone(bpm: f64, beats: u32) -> Self {
        Self {
            grid: BeatGrid::standalone(bpm, beats),
            events: vec![],
            note_count: 0,
        }
    }
}

pub struct Cursor {
    event: usize,
    position: Option<f64>,
    voices: [Voice; 128],
    beat: Option<i64>,
}
impl Default for Cursor {
    fn default() -> Self {
        Self {
            event: 0,
            position: None,
            voices: [Voice::default(); 128],
            beat: None,
        }
    }
}
impl Cursor {
    /// Monotonic audio positions are cheap; a seek rebuilds only once. No missed-beat bursts.
    pub fn advance(&mut self, sequence: &Sequence, position: f64) -> (&[Voice; 128], Option<bool>) {
        if self.position.is_some_and(|p| position < p) {
            *self = Self::default();
        }
        while let Some(e) = sequence
            .events
            .get(self.event)
            .filter(|e| e.time <= position + 1e-9)
        {
            self.voices[usize::from(e.pitch)] = e.voice;
            self.event += 1;
        }
        let (beat, accent, phase) = sequence.grid.beat(position);
        // Starting in the middle of a beat must not invent an off-beat click.
        let click = if self.beat != Some(beat) && (self.beat.is_some() || phase < 0.02) {
            Some(accent)
        } else {
            None
        };
        self.beat = Some(beat);
        self.position = Some(position);
        (&self.voices, click)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn song() -> Song {
        Song::from_json(r#"{"title":"duet","bpm":120,"time_signature":[3,8],"tempo_map":[[1,60]],"notes":[{"pitch":60,"hand":"R","track":0,"start":0,"duration":2},{"pitch":48,"hand":"L","track":1,"start":0,"duration":1},{"pitch":48,"hand":"L","track":1,"start":1,"duration":1}]}"#).unwrap()
    }
    #[test]
    fn listening_contains_the_whole_score_or_only_selected_attacks_and_clips_the_fragment() {
        let song = song();
        let whole = Sequence::listening(&song, &Plan::default());
        assert_eq!(whole.note_count, 3);
        let mut cursor = Cursor::default();
        let (voices, _) = cursor.advance(&whole, 0.0);
        assert!(voices[60].velocity > 0 && voices[48].velocity > 0);
        let plan = Plan {
            hand: "L".into(),
            start: 0.5,
            end: Some(1.5),
            ..Default::default()
        };
        let selected = Sequence::listening(&song, &plan);
        assert_eq!(selected.note_count, 1);
        let mut cursor = Cursor::default();
        assert_eq!(cursor.advance(&selected, 0.75).0[48].velocity, 0);
        assert!(cursor.advance(&selected, 1.0).0[48].velocity > 0);
        assert_eq!(cursor.advance(&selected, 1.0).0[60].velocity, 0);
        assert_eq!(cursor.advance(&selected, 1.5).0[48].velocity, 0);
    }
    #[test]
    fn meter_tempo_changes_count_in_and_accents_follow_musical_time() {
        let g = BeatGrid::new(&song());
        assert_eq!(g.beat(-0.25).0, -1);
        assert_eq!(g.beat(0.0), (0, true, 0.0));
        assert_eq!(g.beat(0.75), (3, true, 0.0));
        assert_eq!(g.beat(1.0), (4, false, 0.0));
        assert_eq!(g.beat(1.5), (5, false, 0.0));
        assert_eq!(g.beat(2.0), (6, true, 0.0));
        let mut s = song();
        s.tempo_map = vec![[1.0, 90.0], [0.0, 60.0], [1.0, 120.0]];
        let g = BeatGrid::new(&s);
        assert_eq!(g.beat(1.5).0, 4);
    }
    #[test]
    fn opposite_hand_is_clipped_retriggers_and_never_contains_player_or_unknown_notes() {
        let s = song();
        let plan = Plan {
            hand: "R".into(),
            start: 0.5,
            end: Some(1.5),
            ..Default::default()
        };
        let seq = Sequence::accompaniment(&s, &plan);
        assert_eq!(seq.note_count, 2);
        let mut cursor = Cursor::default();
        assert_eq!(cursor.advance(&seq, 0.49).0[48].velocity, 0);
        let first = cursor.advance(&seq, 0.5).0[48];
        assert!(first.velocity > 0);
        assert_eq!(cursor.advance(&seq, 0.5).0[60].velocity, 0);
        let next = cursor.advance(&seq, 1.0).0[48];
        assert_ne!(first.generation, next.generation);
        assert_eq!(cursor.advance(&seq, 1.5).0[48].velocity, 0);
        assert_eq!(cursor.advance(&seq, 0.75).0[48], first);
        assert_eq!(Sequence::accompaniment(&s, &Plan::default()).note_count, 0);
    }
    #[test]
    fn track_selection_overlaps_and_seeks_do_not_stick_or_replay_old_clicks() {
        let mut s = song();
        s.notes[2].start = 0.5;
        s.notes[2].duration = 1.0;
        s.notes[2].velocity = 40;
        let seq = Sequence::accompaniment(
            &s,
            &Plan {
                tracks: Some([0].into()),
                ..Default::default()
            },
        );
        let mut c = Cursor::default();
        assert_eq!(c.advance(&seq, 0.0).1, Some(true));
        assert_eq!(c.advance(&seq, 0.0).1, None);
        assert_eq!(c.advance(&seq, 0.5).0[48].velocity, 40);
        assert!(c.advance(&seq, 1.1).0[48].velocity > 0);
        assert_eq!(c.advance(&seq, 1.5).0[48].velocity, 0);
        assert_eq!(c.advance(&seq, 0.1).1, None);
        assert!(c.advance(&seq, 0.25).1.is_some());
    }
}
