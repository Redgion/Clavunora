// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use crate::{
    midi::{Message, MidiEvent},
    score::Note,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Group {
    pub start: f64,
    pub pitches: BTreeSet<u8>,
}
#[derive(Clone, Debug)]
pub struct WaitPractice {
    pub groups: Vec<Group>,
    pub index: usize,
    pub completed: usize,
    accepted: BTreeSet<u8>,
    recent: BTreeMap<u8, f64>,
}
impl WaitPractice {
    pub fn new(notes: &[Note]) -> Self {
        let mut notes = notes.to_vec();
        notes.sort_by(|a, b| a.start.total_cmp(&b.start));
        let mut groups: Vec<Group> = vec![];
        for note in notes {
            if let Some(group) = groups.last_mut().filter(|g| g.start == note.start) {
                group.pitches.insert(note.pitch);
            } else {
                groups.push(Group {
                    start: note.start,
                    pitches: BTreeSet::from([note.pitch]),
                });
            }
        }
        Self {
            groups,
            index: 0,
            completed: 0,
            accepted: BTreeSet::new(),
            recent: BTreeMap::new(),
        }
    }
    /// A rolled chord may contain briefly released notes, but every group requires fresh input.
    pub fn accept_lenient(
        &mut self,
        event: &MidiEvent,
        pressed: &BTreeSet<u8>,
        time: f64,
        window: f64,
    ) -> bool {
        let Message::NoteOn { pitch, velocity } = event.message else {
            return false;
        };
        if velocity == 0 {
            return false;
        }
        let Some(group) = self.current() else {
            return false;
        };
        let pitches = group.pitches.clone();
        self.recent.retain(|_, t| time - *t <= window);
        if pitches.contains(&pitch) {
            self.recent.insert(pitch, time);
            self.accepted.insert(pitch);
        }
        if (pitches.is_subset(&self.accepted) && pitches.is_subset(pressed))
            || pitches.iter().all(|p| self.recent.contains_key(p))
        {
            self.index += 1;
            self.completed += 1;
            self.recent.clear();
            self.accepted.clear();
            true
        } else {
            false
        }
    }
    pub fn skip(&mut self) -> Option<Group> {
        let group = self.current()?.clone();
        self.index += 1;
        self.recent.clear();
        self.accepted.clear();
        Some(group)
    }
    pub fn reset(&mut self, position: f64) {
        self.index = self.groups.partition_point(|g| g.start < position);
        self.completed = 0;
        self.accepted.clear();
        self.recent.clear();
    }
    pub fn current(&self) -> Option<&Group> {
        self.groups.get(self.index)
    }
    /// Call only while waiting at the current group; held notes from the previous group never count as fresh input.
    pub fn accept(&mut self, event: &MidiEvent, pressed: &BTreeSet<u8>) -> bool {
        let Message::NoteOn { pitch, velocity } = event.message else {
            return false;
        };
        if velocity == 0 {
            return false;
        }
        let Some(group) = self.current() else {
            return false;
        };
        let required = group.pitches.clone();
        if required.contains(&pitch) {
            self.accepted.insert(pitch);
        }
        if required.is_subset(&self.accepted) && required.is_subset(pressed) {
            self.index += 1;
            self.completed += 1;
            self.accepted.clear();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::Song;
    fn event(pitch: u8) -> MidiEvent {
        MidiEvent {
            source: "test".into(),
            channel: 0,
            message: Message::NoteOn {
                pitch,
                velocity: 90,
            },
        }
    }
    #[test]
    fn repeats_require_fresh_notes_and_complete_chord() {
        let song=Song::from_json(r#"{"title":"x","notes":[{"pitch":60,"start":0,"duration":0.3},{"pitch":60,"start":0.5,"duration":0.3},{"pitch":64,"start":0.5,"duration":0.3}]}"#).unwrap();
        let mut w = WaitPractice::new(&song.notes);
        assert!(w.accept(&event(60), &BTreeSet::from([60])));
        assert!(!w.accept(&event(64), &BTreeSet::from([60, 64])));
        assert!(!w.accept(&event(60), &BTreeSet::from([60])));
        assert!(w.accept(&event(64), &BTreeSet::from([60, 64])));
        assert_eq!(w.completed, 2);
        assert!(w.current().is_none());
        w.reset(0.5);
        assert_eq!(w.index, 1);
        assert_eq!(w.completed, 0);
    }
}
