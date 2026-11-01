// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use crate::keyboard_view::Key;
use clavunora_core::midi::{Message, MidiEvent};
use eframe::egui::{Event, TouchPhase};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct TouchKeyboard {
    fingers: BTreeMap<(u64, u64), Option<u8>>,
}
impl TouchKeyboard {
    pub fn active(&self) -> bool {
        !self.fingers.is_empty()
    }
    fn event(id: (u64, u64), pitch: u8, on: bool) -> MidiEvent {
        MidiEvent {
            source: format!("touch:{}:{}", id.0, id.1),
            channel: 0,
            message: if on {
                Message::NoteOn {
                    pitch,
                    velocity: 90,
                }
            } else {
                Message::NoteOff { pitch }
            },
        }
    }
    pub fn update(&mut self, events: &[Event], keys: &[Key]) -> Vec<MidiEvent> {
        let mut output = vec![];
        for event in events {
            let Event::Touch {
                device_id,
                id,
                phase,
                pos,
                ..
            } = event
            else {
                continue;
            };
            let id = (device_id.0, id.0);
            let pitch = keys
                .iter()
                .filter(|k| k.black)
                .chain(keys.iter().filter(|k| !k.black))
                .find(|key| key.rect.contains(*pos))
                .map(|key| key.pitch);
            match phase {
                TouchPhase::Start if pitch.is_some() => {
                    if let Some(Some(previous)) = self.fingers.insert(id, pitch) {
                        output.push(Self::event(id, previous, false));
                    }
                    output.push(Self::event(id, pitch.unwrap(), true));
                }
                TouchPhase::Move if self.fingers.contains_key(&id) => {
                    let previous = self.fingers.insert(id, pitch).flatten();
                    if previous != pitch {
                        if let Some(p) = previous {
                            output.push(Self::event(id, p, false));
                        }
                        if let Some(p) = pitch {
                            output.push(Self::event(id, p, true));
                        }
                    }
                }
                TouchPhase::End | TouchPhase::Cancel => {
                    if let Some(Some(p)) = self.fingers.remove(&id) {
                        output.push(Self::event(id, p, false));
                    }
                }
                _ => {}
            }
        }
        output
    }
    pub fn clear(&mut self) -> Vec<MidiEvent> {
        std::mem::take(&mut self.fingers)
            .into_iter()
            .filter_map(|(id, p)| p.map(|p| Self::event(id, p, false)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Pos2, Rect, TouchDeviceId, TouchId, Vec2};
    fn touch(id: u64, phase: TouchPhase, x: f32) -> Event {
        Event::Touch {
            device_id: TouchDeviceId(1),
            id: TouchId(id),
            phase,
            pos: Pos2::new(x, 20.0),
            force: None,
        }
    }
    #[test]
    fn chord_owners_slide_and_cancel_release_independently() {
        let keys = crate::keyboard_view::Config::default()
            .keys(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 200.0)));
        let mut state = TouchKeyboard::default();
        let on = state.update(
            &[
                touch(1, TouchPhase::Start, 10.0),
                touch(2, TouchPhase::Start, 10.0),
            ],
            &keys,
        );
        assert_eq!(on.len(), 2);
        assert_ne!(on[0].source, on[1].source);
        assert_eq!(
            state
                .update(&[touch(1, TouchPhase::End, 10.0)], &keys)
                .len(),
            1
        );
        assert!(state.active());
        assert_eq!(
            state
                .update(&[touch(2, TouchPhase::Move, 900.0)], &keys)
                .len(),
            1
        );
        assert!(
            state
                .update(&[touch(2, TouchPhase::Cancel, 900.0)], &keys)
                .is_empty()
        );
        assert!(!state.active());
    }
    #[test]
    fn scrolling_into_keyboard_does_not_start_note_and_clear_releases_chord() {
        let keys = crate::keyboard_view::Config::default()
            .keys(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 200.0)));
        let mut state = TouchKeyboard::default();
        assert!(
            state
                .update(
                    &[
                        touch(1, TouchPhase::Start, 900.0),
                        touch(1, TouchPhase::Move, 10.0)
                    ],
                    &keys
                )
                .is_empty()
        );
        state.update(
            &[
                touch(2, TouchPhase::Start, 10.0),
                touch(3, TouchPhase::Start, 150.0),
            ],
            &keys,
        );
        assert_eq!(state.clear().len(), 2);
        assert!(!state.active());
    }
}
