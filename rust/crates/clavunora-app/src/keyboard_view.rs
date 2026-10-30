// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Display-only piano geometry. It never transposes or filters MIDI/scoring input.
use eframe::egui::{Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};
pub const COUNTS: [u8; 8] = [13, 25, 32, 37, 49, 61, 76, 88];
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub count: u8,
    pub first: u8,
    pub zoom: f32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            count: 13,
            first: 60,
            zoom: 1.0,
        }
    }
}
impl Config {
    pub fn valid(&self) -> bool {
        COUNTS.contains(&self.count)
            && u16::from(self.first) + u16::from(self.count) <= 128
            && self.zoom.is_finite()
            && (0.25..=1.0).contains(&self.zoom)
    }
    pub fn preset(&mut self, count: u8) {
        if !COUNTS.contains(&count) {
            return;
        }
        self.count = count;
        self.first = match count {
            13 => 60,
            25 | 37 => 48,
            32 => 41,
            49 | 61 => 36,
            76 => 28,
            88 => 21,
            _ => 60,
        };
    }
    pub fn end(&self) -> u8 {
        self.first + self.count - 1
    }
    pub fn keys(&self, rect: Rect) -> Vec<Key> {
        let whites = (self.first..=self.end()).filter(|p| !black(*p)).count();
        let width = rect.width() / whites as f32;
        let mut before = 0;
        let mut keys = vec![];
        for pitch in self.first..=self.end() {
            let is_black = black(pitch);
            let key = if is_black {
                let x = rect.left() + before as f32 * width;
                Rect::from_min_max(
                    Pos2::new(x - width * 0.29, rect.top()),
                    Pos2::new(x + width * 0.29, rect.top() + rect.height() * 0.63),
                )
                .intersect(rect)
            } else {
                let x = rect.left() + before as f32 * width;
                before += 1;
                Rect::from_min_max(
                    Pos2::new(x, rect.top()),
                    Pos2::new(x + width, rect.bottom()),
                )
                .intersect(rect)
            };
            keys.push(Key {
                pitch,
                rect: key,
                black: is_black,
            });
        }
        keys
    }
    pub fn bounds(&self, rect: Rect) -> Rect {
        Rect::from_center_size(
            rect.center(),
            Vec2::new(rect.width() * self.zoom, rect.height()),
        )
    }
}
pub fn black(pitch: u8) -> bool {
    [1, 3, 6, 8, 10].contains(&(pitch % 12))
}
pub struct Key {
    pub pitch: u8,
    pub rect: Rect,
    pub black: bool,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_presets_have_exact_count_and_valid_black_key_overlap() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 150.0));
        for count in COUNTS {
            let mut config = Config::default();
            config.preset(count);
            assert!(config.valid());
            let keys = config.keys(rect);
            assert_eq!(keys.len(), usize::from(count));
            assert!(
                keys.iter()
                    .all(|k| k.rect.is_positive() && rect.contains_rect(k.rect))
            );
            assert_eq!(keys.first().unwrap().pitch, config.first);
            assert_eq!(keys.last().unwrap().pitch, config.end());
        }
        let mut full = Config::default();
        full.preset(88);
        assert_eq!((full.first, full.end()), (21, 108));
    }
    #[test]
    fn invalid_values_rejected_zoom_preserves_pitches_and_both_range_edges_work() {
        assert!(
            !Config {
                zoom: f32::NAN,
                ..Config::default()
            }
            .valid()
        );
        assert!(
            !Config {
                first: 127,
                ..Config::default()
            }
            .valid()
        );
        let mut c = Config::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 120.0));
        c.zoom = 0.5;
        assert_eq!(c.bounds(rect).width(), 500.0);
        assert_eq!(c.keys(c.bounds(rect)).len(), 13);
        for first in [0, 1, 115] {
            c.first = first;
            assert!(c.valid());
            assert_eq!(c.keys(rect).len(), 13);
        }
    }
}
