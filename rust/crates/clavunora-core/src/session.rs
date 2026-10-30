// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! One immutable attempt configuration. Pause/mode/speed changes finish the old attempt.
use crate::{
    midi::{Message, MidiEvent},
    performance::{Mistake, MistakeKind, Options, Outcome, Performance},
    practice::{Group, WaitPractice},
    score::Song,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "Ожидание")]
    Waiting,
    #[serde(rename = "Ритм")]
    Rhythm,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Report {
    #[serde(flatten)]
    pub outcome: Outcome,
    pub title: String,
    pub mode: Mode,
    pub speed: f64,
    pub hand: String,
    pub start: f64,
    pub end: f64,
    pub tolerance: f64,
    pub latency: f64,
    pub score_version: String,
    pub full_song: bool,
    pub both_hands: bool,
    pub difficulty: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<usize>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeated: bool,
}
impl Report {
    pub fn validate(&self) -> Result<()> {
        self.outcome.validate()?;
        ensure!(
            self.tracks
                .as_ref()
                .is_none_or(|s| !s.is_empty() && s.iter().all(|t| *t <= 10000)),
            "Некорректные дорожки результата"
        );
        ensure!(
            !self.title.trim().is_empty() && self.title.chars().count() <= 500,
            "Некорректное название результата"
        );
        ensure!(
            self.speed.is_finite()
                && (0.25..=2.0).contains(&self.speed)
                && self.start.is_finite()
                && self.start >= 0.0
                && self.end.is_finite()
                && self.end > self.start,
            "Некорректные границы результата"
        );
        ensure!(
            self.tolerance.is_finite()
                && (0.03..=1.0).contains(&self.tolerance)
                && self.latency.is_finite()
                && (-0.5..=0.5).contains(&self.latency),
            "Некорректные допуски результата"
        );
        ensure!(
            self.difficulty <= 5 && ["", "L", "R"].contains(&self.hand.as_str()),
            "Некорректная партия результата"
        );
        ensure!(
            !self.full_song
                || (self.outcome.complete
                    && self.start == 0.0
                    && self.hand.is_empty()
                    && self.tracks.is_none()
                    && !self.repeated),
            "Фрагмент нельзя считать целой пьесой"
        );
        ensure!(
            self.score_version.len() == 64
                && self.score_version.bytes().all(|b| b.is_ascii_hexdigit()),
            "Некорректная версия партитуры"
        );
        let expected = Outcome::new(
            self.outcome.complete,
            self.outcome.total,
            self.outcome.hit,
            vec![],
        )
        .accuracy;
        ensure!(
            (self.outcome.accuracy - expected).abs() < 1e-8,
            "Точность не соответствует нотам"
        );
        Ok(())
    }
    pub fn passed(&self) -> bool {
        self.full_song
            && self.outcome.complete
            && self.outcome.accuracy >= 90.0
            && self.outcome.mistakes.is_empty()
    }
    pub fn worth_saving(&self) -> bool {
        self.outcome.hit > 0 || !self.outcome.mistakes.is_empty() || self.outcome.complete
    }
}

enum Engine {
    Rhythm(Box<Performance>),
    Waiting {
        wait: WaitPractice,
        mistakes: Vec<Mistake>,
    },
}
pub struct Session {
    engine: Engine,
    report: Report,
    finished: bool,
    full_range: bool,
}
impl Session {
    pub fn new(song: &Song, mode: Mode, mut options: Options, version: String) -> Result<Self> {
        song.validate()?;
        let end = options.end.unwrap_or(song.duration());
        options.end = Some(end);
        let performance = Performance::new(&song.notes, options)?;
        ensure!(performance.total() > 0, "После выбранной позиции нет нот");
        let notes: Vec<_> = song
            .notes
            .iter()
            .filter(|n| n.start >= options.start && n.start < end)
            .cloned()
            .collect();
        let engine = if mode == Mode::Rhythm {
            Engine::Rhythm(Box::new(performance))
        } else {
            Engine::Waiting {
                wait: WaitPractice::new(&notes),
                mistakes: vec![],
            }
        };
        let report = Report {
            outcome: Outcome::new(false, 0, 0, vec![]),
            title: song.title.clone(),
            mode,
            speed: options.speed,
            hand: String::new(),
            start: options.start,
            end,
            tolerance: options.tolerance,
            latency: options.latency,
            score_version: version,
            full_song: false,
            both_hands: notes.iter().any(|n| n.hand == "L") && notes.iter().any(|n| n.hand == "R"),
            difficulty: song.difficulty,
            tracks: None,
            repeated: false,
        };
        Ok(Self {
            engine,
            report,
            finished: false,
            full_range: options.start == 0.0 && end == song.duration(),
        })
    }
    pub fn selected(
        song: &Song,
        mode: Mode,
        options: Options,
        version: String,
        plan: &crate::lesson::Plan,
    ) -> Result<Self> {
        let filtered = plan.selected(song)?;
        ensure!(
            options.start >= plan.start && options.end.unwrap_or(plan.end(song)) <= plan.end(song),
            "Попытка выходит за границы выбранного фрагмента"
        );
        let mut session = Self::new(
            &filtered,
            mode,
            Options {
                end: Some(options.end.unwrap_or(plan.end(song))),
                ..options
            },
            version,
        )?;
        session.report.hand = plan.hand.clone();
        session.report.tracks = plan.tracks.as_ref().map(|s| s.iter().copied().collect());
        session.report.repeated = plan.repeat;
        session.full_range = plan.whole(song)
            && options.start == 0.0
            && options.end.unwrap_or(plan.end(song)) == song.duration();
        Ok(session)
    }
    /// Returns true only when a waiting chord was completed. Rhythm never controls transport.
    pub fn feed(
        &mut self,
        event: &MidiEvent,
        pressed: &BTreeSet<u8>,
        position: f64,
        waiting: bool,
    ) -> Result<bool> {
        self.feed_timed(
            event,
            pressed,
            position,
            waiting,
            position / self.report.speed,
        )
    }
    pub fn feed_timed(
        &mut self,
        event: &MidiEvent,
        pressed: &BTreeSet<u8>,
        position: f64,
        waiting: bool,
        wall_time: f64,
    ) -> Result<bool> {
        if self.finished {
            return Ok(false);
        }
        ensure!(
            position.is_finite() && wall_time.is_finite(),
            "Некорректное время попытки"
        );
        match &mut self.engine {
            Engine::Rhythm(p) => {
                p.feed(event, position)?;
                Ok(false)
            }
            Engine::Waiting { wait, mistakes } => {
                let window = self.report.tolerance.max(0.25) * self.report.speed;
                if !waiting
                    && !wait
                        .current()
                        .is_some_and(|g| position >= g.start - window && position <= g.start)
                {
                    return Ok(false);
                }
                if let Message::NoteOn { pitch, velocity } = event.message
                    && velocity > 0
                    && wait.current().is_some_and(|g| !g.pitches.contains(&pitch))
                {
                    mistakes.push(Mistake {
                        position: position.max(0.0),
                        pitch,
                        kind: MistakeKind::Extra,
                        delta: 0.0,
                    });
                }
                Ok(wait.accept_lenient(event, pressed, wall_time, window / self.report.speed))
            }
        }
    }
    pub fn skip_waiting(&mut self) -> bool {
        if self.finished {
            return false;
        }
        if let Engine::Waiting { wait, mistakes } = &mut self.engine
            && let Some(group) = wait.skip()
        {
            for pitch in group.pitches {
                mistakes.push(Mistake {
                    position: group.start,
                    pitch,
                    kind: MistakeKind::Missed,
                    delta: 0.0,
                });
            }
            return true;
        }
        false
    }
    pub fn expected(&self) -> Option<&Group> {
        match &self.engine {
            Engine::Waiting { wait, .. } => wait.current(),
            _ => None,
        }
    }
    pub fn completed_groups(&self) -> usize {
        match &self.engine {
            Engine::Waiting { wait, .. } => wait.completed,
            _ => 0,
        }
    }
    pub fn finish(&mut self, complete: bool) -> Report {
        if !self.finished {
            self.report.outcome = match &mut self.engine {
                Engine::Rhythm(p) => p.finish(complete),
                Engine::Waiting { wait, mistakes } => Outcome::new(
                    complete && wait.current().is_none(),
                    wait.groups.len(),
                    wait.completed,
                    mistakes.clone(),
                ),
            };
            self.report.full_song = self.report.outcome.complete && self.full_range;
            self.finished = true;
        }
        self.report.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{catalog, midi::PianoState};
    fn session(mode: Mode, options: Options) -> Session {
        let song = &catalog::course()[0].1;
        let version = crate::version::python_hash(&serde_json::to_string(song).unwrap()).unwrap();
        Session::new(song, mode, options, version).unwrap()
    }
    fn finish_wait(session: &mut Session, start: f64, end: f64) {
        let mut state = PianoState::default();
        let notes = catalog::course()[0].1.notes.clone();
        for n in notes.iter().filter(|n| n.start >= start && n.start < end) {
            let event = MidiEvent::parse(&[0x90, n.pitch, 90], "midi").unwrap();
            state.apply(&event);
            assert!(
                session
                    .feed(&event, &state.pressed(), n.start, true)
                    .unwrap()
            );
            let event = MidiEvent::parse(&[0x80, n.pitch, 0], "midi").unwrap();
            state.apply(&event);
            session
                .feed(&event, &state.pressed(), n.end(), false)
                .unwrap();
        }
    }
    #[test]
    fn waiting_accepts_early_and_briefly_rolled_chords_but_not_stale_input() {
        let song=Song::from_json(r#"{"title":"chords","notes":[{"pitch":60,"start":1,"duration":1},{"pitch":64,"start":1,"duration":1},{"pitch":60,"start":3,"duration":1}]}"#).unwrap();
        let mut s = Session::new(&song, Mode::Waiting, Options::default(), "a".repeat(64)).unwrap();
        let on = |p| MidiEvent::parse(&[0x90, p, 90], "midi").unwrap();
        assert!(
            !s.feed_timed(&on(60), &BTreeSet::new(), 0.5, false, 10.0)
                .unwrap()
        );
        assert!(
            !s.feed_timed(&on(60), &BTreeSet::new(), 0.85, false, 10.4)
                .unwrap()
        );
        assert!(
            s.feed_timed(&on(64), &BTreeSet::from([64]), 1.0, true, 10.55)
                .unwrap()
        );
        assert_eq!(s.expected().unwrap().start, 3.0);
        assert!(s.skip_waiting());
        let report = s.finish(true);
        assert!(report.outcome.complete);
        assert_eq!(report.outcome.hit, 1);
        assert!(!report.passed());
        let mut s = Session::new(&song, Mode::Waiting, Options::default(), "a".repeat(64)).unwrap();
        s.feed_timed(&on(60), &BTreeSet::new(), 1.0, true, 0.0)
            .unwrap();
        assert!(
            !s.feed_timed(&on(64), &BTreeSet::new(), 1.0, true, 1.0)
                .unwrap()
        );
        assert_eq!(s.completed_groups(), 0);
    }
    #[test]
    fn selected_hands_tracks_fragments_and_repeats_never_count_as_whole_piece() {
        use crate::lesson::Plan;
        let song = crate::catalog::course()
            .into_iter()
            .find(|(id, _)| id == "course-5")
            .unwrap()
            .1;
        let version = "a".repeat(64);
        for plan in [
            Plan {
                hand: "L".into(),
                ..Default::default()
            },
            Plan {
                tracks: Some(BTreeSet::from([0, 1])),
                ..Default::default()
            },
            Plan {
                start: 1.0,
                end: Some(2.0),
                ..Default::default()
            },
            Plan {
                repeat: true,
                ..Default::default()
            },
        ] {
            let notes = plan.selected(&song).unwrap().notes;
            let mut session = Session::selected(
                &song,
                Mode::Waiting,
                Options {
                    start: plan.start,
                    end: Some(plan.end(&song)),
                    ..Default::default()
                },
                version.clone(),
                &plan,
            )
            .unwrap();
            let mut state = crate::midi::PianoState::default();
            for note in &notes {
                let event = MidiEvent::parse(&[0x90, note.pitch, 90], "test").unwrap();
                state.apply(&event);
                session
                    .feed(&event, &state.pressed(), note.start, true)
                    .unwrap();
            }
            let report = session.finish(true);
            report.validate().unwrap();
            assert!(report.outcome.complete);
            assert!(!report.full_song);
            assert!(!report.passed());
            assert_eq!(report.score_version, version);
            assert_eq!(report.hand, plan.hand);
        }
    }
    #[test]
    fn full_waiting_report_and_finalization_are_consistent() {
        let mut s = session(Mode::Waiting, Options::default());
        finish_wait(&mut s, 0.0, 10.0);
        let report = s.finish(true);
        report.validate().unwrap();
        assert!(report.full_song && report.passed());
        assert_eq!(report.outcome.hit, 4);
        assert_eq!(s.finish(false), report);
    }
    #[test]
    fn fragments_and_mid_piece_resumes_never_count_as_whole_piece() {
        let mut s = session(
            Mode::Waiting,
            Options {
                start: 1.0,
                ..Default::default()
            },
        );
        finish_wait(&mut s, 1.0, 10.0);
        let report = s.finish(true);
        assert_eq!(report.outcome.accuracy, 100.0);
        assert!(!report.full_song && !report.passed());
        let mut s = session(
            Mode::Waiting,
            Options {
                end: Some(0.7),
                ..Default::default()
            },
        );
        finish_wait(&mut s, 0.0, 0.7);
        let report = s.finish(true);
        assert!(report.outcome.complete);
        assert!(!report.full_song && !report.passed());
    }
    #[test]
    fn premature_finish_and_blank_pause_do_not_fabricate_progress() {
        let mut s = session(Mode::Waiting, Options::default());
        let result = s.finish(true);
        assert!(!result.outcome.complete && !result.full_song && !result.worth_saving());
        let mut s = session(Mode::Rhythm, Options::default());
        let result = s.finish(false);
        assert!(!result.worth_saving());
        assert!(result.outcome.mistakes.is_empty());
    }
    #[test]
    fn rhythm_records_missing_notes_and_keeps_session_settings() {
        let mut s = session(
            Mode::Rhythm,
            Options {
                speed: 0.5,
                tolerance: 0.1,
                latency: 0.1,
                ..Default::default()
            },
        );
        s.feed(
            &MidiEvent::parse(&[0x90, 60, 90], "midi").unwrap(),
            &BTreeSet::from([60]),
            0.05,
            false,
        )
        .unwrap();
        s.feed(
            &MidiEvent::parse(&[0x80, 60, 0], "midi").unwrap(),
            &BTreeSet::new(),
            0.7,
            false,
        )
        .unwrap();
        let report = s.finish(true);
        report.validate().unwrap();
        assert_eq!(report.mode, Mode::Rhythm);
        assert_eq!(report.speed, 0.5);
        assert_eq!(report.outcome.hit, 1);
        assert_eq!(
            report
                .outcome
                .mistakes
                .iter()
                .filter(|m| m.kind == MistakeKind::Missed)
                .count(),
            3
        );
    }
}
