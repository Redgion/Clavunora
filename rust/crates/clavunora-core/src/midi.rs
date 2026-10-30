// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    NoteOn { pitch: u8, velocity: u8 },
    NoteOff { pitch: u8 },
    Control { control: u8, value: u8 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    pub source: String,
    pub channel: u8,
    pub message: Message,
}
impl MidiEvent {
    pub fn parse(bytes: &[u8], source: &str) -> Option<Self> {
        if bytes.len() != 3 || bytes[1] > 127 || bytes[2] > 127 {
            return None;
        }
        let message = match bytes[0] & 0xf0 {
            0x90 if bytes[2] > 0 => Message::NoteOn {
                pitch: bytes[1],
                velocity: bytes[2],
            },
            0x80 | 0x90 => Message::NoteOff { pitch: bytes[1] },
            0xb0 => Message::Control {
                control: bytes[1],
                value: bytes[2],
            },
            _ => return None,
        };
        Some(Self {
            source: source.into(),
            channel: bytes[0] & 15,
            message,
        })
    }
}
type Key = (String, u8, u8);
#[derive(Default, Debug)]
pub struct PianoState {
    held: BTreeMap<Key, u8>,
    latched: BTreeMap<Key, u8>,
    pedals: BTreeSet<(String, u8)>,
    pub last_velocity: u8,
    pub note_count: u64,
}
impl PianoState {
    pub fn pressed(&self) -> BTreeSet<u8> {
        self.held.keys().map(|k| k.2).collect()
    }
    pub fn sounding(&self) -> BTreeSet<u8> {
        self.held
            .keys()
            .chain(self.latched.keys())
            .map(|k| k.2)
            .collect()
    }
    /// The loudest still-sounding source wins when physical MIDI pitches overlap.
    pub fn sounding_velocities(&self) -> BTreeMap<u8, u8> {
        let mut result = BTreeMap::new();
        for (key, velocity) in self.held.iter().chain(self.latched.iter()) {
            let entry = result.entry(key.2).or_insert(0u8);
            *entry = (*entry).max(*velocity);
        }
        result
    }
    pub fn pedal_active(&self) -> bool {
        !self.pedals.is_empty()
    }
    pub fn apply(&mut self, event: &MidiEvent) {
        let lane = (event.source.clone(), event.channel);
        match event.message {
            Message::NoteOn { pitch, velocity } if velocity > 0 => {
                let key = (event.source.clone(), event.channel, pitch);
                self.held.insert(key.clone(), velocity);
                self.latched.remove(&key);
                self.last_velocity = velocity;
                self.note_count += 1;
            }
            Message::NoteOff { pitch } | Message::NoteOn { pitch, .. } => {
                let key = (event.source.clone(), event.channel, pitch);
                if let Some(v) = self.held.remove(&key)
                    && self.pedals.contains(&lane)
                {
                    self.latched.insert(key, v);
                }
            }
            Message::Control { control, value } => match control {
                64 if value >= 64 => {
                    self.pedals.insert(lane);
                }
                64 | 121 => {
                    self.pedals.remove(&lane);
                    self.latched.retain(|k, _| (k.0.clone(), k.1) != lane);
                }
                120 | 123 => {
                    self.held.retain(|k, _| (k.0.clone(), k.1) != lane);
                    self.latched.retain(|k, _| (k.0.clone(), k.1) != lane);
                }
                _ => {}
            },
        }
    }
    pub fn clear_source(&mut self, source: &str) {
        self.held.retain(|k, _| k.0 != source);
        self.latched.retain(|k, _| k.0 != source);
        self.pedals.retain(|k| k.0 != source);
    }
    pub fn panic(&mut self) {
        self.held.clear();
        self.latched.clear();
        self.pedals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(s: &mut PianoState, data: &[u8], source: &str) {
        s.apply(&MidiEvent::parse(data, source).unwrap());
    }
    #[test]
    fn zero_velocity_and_bad_messages() {
        let mut s = PianoState::default();
        send(&mut s, &[0x90, 60, 90], "midi");
        send(&mut s, &[0x90, 60, 0], "midi");
        assert!(s.pressed().is_empty());
        assert_eq!(s.note_count, 1);
        assert!(MidiEvent::parse(&[0x90, 60], "midi").is_none());
        assert!(MidiEvent::parse(&[0x90, 255, 80], "midi").is_none());
        assert!(MidiEvent::parse(&[0xf8], "midi").is_none());
    }
    #[test]
    fn sustain_isolates_channels_and_sources() {
        let mut s = PianoState::default();
        send(&mut s, &[0xb0, 64, 127], "midi");
        send(&mut s, &[0x90, 60, 90], "midi");
        send(&mut s, &[0x91, 60, 90], "midi");
        send(&mut s, &[0x80, 60, 0], "midi");
        send(&mut s, &[0x81, 60, 0], "midi");
        assert!(s.pressed().is_empty());
        assert_eq!(s.sounding(), BTreeSet::from([60]));
        send(&mut s, &[0xb1, 64, 0], "midi");
        assert!(!s.sounding().is_empty());
        send(&mut s, &[0xb0, 64, 0], "midi");
        assert!(s.sounding().is_empty());
        send(&mut s, &[0x90, 60, 90], "midi");
        send(&mut s, &[0x90, 60, 90], "computer");
        s.clear_source("midi");
        assert_eq!(s.pressed(), BTreeSet::from([60]));
        s.panic();
        assert!(s.sounding().is_empty());
    }
    #[test]
    fn controller_reset_preserves_other_lane() {
        let mut s = PianoState::default();
        send(&mut s, &[0x90, 60, 90], "midi");
        send(&mut s, &[0x91, 64, 90], "midi");
        send(&mut s, &[0xb0, 123, 0], "midi");
        assert_eq!(s.pressed(), BTreeSet::from([64]));
    }
}
