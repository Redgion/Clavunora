// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! A clipped, paged educational grand staff from the available pitch/time model.
use clavunora_core::{
    notation::{self, Language, Page},
    playback::BeatGrid,
    score::{Note, Song},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

struct Staff {
    region: Rect,
    center: f32,
    step: f32,
    reference: i32,
}
impl Staff {
    fn new(region: Rect, treble: bool, notes: &[&Note]) -> Self {
        let reference = notation::diatonic(if treble { 64 } else { 43 });
        let low = notes
            .iter()
            .map(|n| notation::diatonic(n.pitch) - 3)
            .min()
            .unwrap_or(reference - 5)
            .min(reference - 5);
        let high = notes
            .iter()
            .map(|n| notation::diatonic(n.pitch) + 8)
            .max()
            .unwrap_or(reference + 14)
            .max(reference + 14);
        Self {
            region,
            center: (low + high) as f32 / 2.0,
            step: (region.height() / (high - low) as f32).clamp(0.5, 4.5),
            reference,
        }
    }
    fn y(&self, step: i32) -> f32 {
        self.region.center().y + (self.center - step as f32) * self.step
    }
    fn bottom(&self) -> f32 {
        self.y(self.reference)
    }
    fn top(&self) -> f32 {
        self.y(self.reference + 8)
    }
}

pub struct Options {
    pub language: Language,
    pub dark: bool,
    pub fingers: bool,
    pub height: f32,
    pub page: Option<usize>,
}

pub fn show(
    ui: &mut egui::Ui,
    song: &Song,
    grid: &BeatGrid,
    position: f64,
    options: Options,
) -> Rect {
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, options.height.max(60.0)), Sense::hover());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let bg = if options.dark {
        Color32::from_rgb(26, 36, 48)
    } else {
        Color32::from_rgb(250, 248, 239)
    };
    let ink = if options.dark {
        Color32::from_rgb(222, 233, 240)
    } else {
        Color32::from_rgb(37, 53, 66)
    };
    let line = if options.dark {
        Color32::from_rgb(121, 145, 162)
    } else {
        Color32::from_rgb(134, 150, 158)
    };
    painter.rect_filled(rect, 5, bg);
    let page = Page::new(song, grid, width, position, options.page);
    let first_bar = (page.start / page.beats_per_bar) as usize + 1;
    let last_bar = (page.end / page.beats_per_bar) as usize;
    painter.text(
        rect.left_top() + Vec2::new(12.0, 7.0),
        Align2::LEFT_TOP,
        format!(
            "Нотный лист · такты {first_bar}–{last_bar} · {}/{}",
            page.index + 1,
            page.count
        ),
        FontId::proportional(12.0),
        ink,
    );
    let visible: Vec<_> = song
        .notes
        .iter()
        .filter(|n| {
            grid.beat_position(n.end()) > page.start && grid.beat_position(n.start) < page.end
        })
        .collect();
    let mid = rect.center().y;
    let treble_region = Rect::from_min_max(
        rect.left_top() + Vec2::new(0.0, 25.0),
        Pos2::new(rect.right(), mid - 2.0),
    );
    let bass_region = Rect::from_min_max(
        Pos2::new(rect.left(), mid + 3.0),
        rect.right_bottom() - Vec2::new(0.0, 24.0),
    );
    let treble_notes: Vec<_> = visible
        .iter()
        .copied()
        .filter(|n| notation::treble(n))
        .collect();
    let bass_notes: Vec<_> = visible
        .iter()
        .copied()
        .filter(|n| !notation::treble(n))
        .collect();
    let treble = Staff::new(treble_region, true, &treble_notes);
    let bass = Staff::new(bass_region, false, &bass_notes);
    let left = rect.left() + 90.0;
    let right = rect.right() - 18.0;
    let x =
        |beat: f64| left + ((beat - page.start) / (page.end - page.start)) as f32 * (right - left);
    for (staff, is_treble) in [(&treble, true), (&bass, false)] {
        for i in 0..5 {
            let y = staff.y(staff.reference + i * 2);
            painter.line_segment(
                [Pos2::new(rect.left() + 12.0, y), Pos2::new(right, y)],
                Stroke::new(0.8_f32, line),
            );
        }
        clef(
            &painter,
            Pos2::new(rect.left() + 35.0, staff.top()),
            (staff.bottom() - staff.top()).max(10.0),
            is_treble,
            ink,
        );
        painter.text(
            Pos2::new(rect.left() + 68.0, (staff.top() + staff.bottom()) / 2.0),
            Align2::CENTER_CENTER,
            format!("{}\n{}", song.time_signature[0], song.time_signature[1]),
            FontId::proportional(13.0),
            ink,
        );
    }
    let mut beat = page.start;
    while beat <= page.end + 1e-8 {
        let xx = x(beat);
        painter.line_segment(
            [Pos2::new(xx, treble.top()), Pos2::new(xx, bass.bottom())],
            Stroke::new(0.8_f32, line),
        );
        beat += page.beats_per_bar;
    }
    let mut labels: Vec<Rect> = vec![];
    let mut placed: Vec<(f64, bool, i32, bool)> = vec![];
    for (index, n) in visible.iter().enumerate() {
        let is_treble = notation::treble(n);
        let staff = if is_treble { &treble } else { &bass };
        let step = notation::diatonic(n.pitch);
        let start = grid.beat_position(n.start);
        let end = grid.beat_position(n.end());
        let carried = start < page.start;
        let mut xx = x(start.max(page.start)) + if carried { 8.0 } else { 0.0 };
        let shifted = placed
            .iter()
            .rev()
            .find(|(s, t, d, _)| *s == n.start && *t == is_treble && (*d - step).abs() == 1)
            .is_some_and(|(_, _, _, shift)| !*shift);
        if shifted {
            xx += 9.0;
        }
        placed.push((n.start, is_treble, step, shifted));
        let yy = staff.y(step);
        let center = Pos2::new(xx, yy);
        let radius = Vec2::new(
            (staff.step * 1.4).clamp(2.3, 6.0),
            (staff.step * 0.95).clamp(1.8, 4.2),
        );
        let active = n.start <= position && position < n.end();
        let color = if active {
            Color32::from_rgb(23, 177, 183)
        } else {
            ink
        };
        let quarters = (end - start) * 4.0 / f64::from(song.time_signature[1]);
        let rhythm = notation::rhythm(quarters);
        let mut ledger = staff.reference - 2;
        while ledger >= step {
            let y = staff.y(ledger);
            painter.line_segment(
                [
                    Pos2::new(xx - radius.x - 3.0, y),
                    Pos2::new(xx + radius.x + 3.0, y),
                ],
                Stroke::new(0.9_f32, line),
            );
            ledger -= 2;
        }
        let mut ledger = staff.reference + 10;
        while ledger <= step {
            let y = staff.y(ledger);
            painter.line_segment(
                [
                    Pos2::new(xx - radius.x - 3.0, y),
                    Pos2::new(xx + radius.x + 3.0, y),
                ],
                Stroke::new(0.9_f32, line),
            );
            ledger += 2;
        }
        // The thin duration guide also makes MIDI articulation and carried notes explicit.
        painter.line_segment(
            [
                center + Vec2::new(radius.x + 1.0, 0.0),
                Pos2::new(x(end.min(page.end)).max(xx + radius.x), yy),
            ],
            Stroke::new(
                1.0_f32,
                if active {
                    color
                } else {
                    line.gamma_multiply(0.55)
                },
            ),
        );
        if rhythm.hollow {
            painter.add(Shape::ellipse_stroke(
                center,
                radius,
                Stroke::new(1.5_f32, color),
            ));
        } else {
            painter.add(Shape::ellipse_filled(center, radius, color));
        }
        if rhythm.stem && !carried {
            let down = step >= staff.reference + 4;
            let stem_x = xx + if down { -radius.x } else { radius.x };
            let stem_y = yy
                + if down {
                    staff.step * 6.0
                } else {
                    -staff.step * 6.0
                };
            painter.line_segment(
                [Pos2::new(stem_x, yy), Pos2::new(stem_x, stem_y)],
                Stroke::new(1.2_f32, color),
            );
            for flag in 0..rhythm.flags {
                let dy = f32::from(flag) * staff.step * 1.5 * if down { -1.0 } else { 1.0 };
                painter.line_segment(
                    [
                        Pos2::new(stem_x, stem_y + dy),
                        Pos2::new(stem_x + 7.0, stem_y + dy + if down { -7.0 } else { 7.0 }),
                    ],
                    Stroke::new(1.5_f32, color),
                );
            }
        }
        if rhythm.dotted {
            painter.circle_filled(center + Vec2::new(radius.x + 4.0, -2.0), 1.5, color);
        }
        // Explicit accidentals on every attack avoid implying a key signature we do not have.
        if !carried {
            accidental(
                &painter,
                center - Vec2::new(radius.x + 9.0, 0.0),
                [1, 3, 6, 8, 10].contains(&(n.pitch % 12)),
                color,
            );
        }
        let name = options.language.name(n.pitch);
        let galley = painter.layout_no_wrap(name.clone(), FontId::proportional(12.0), color);
        let text_rect = Rect::from_min_size(
            center + Vec2::new(radius.x + 5.0, -galley.size().y - 4.0),
            galley.size(),
        );
        if staff.region.contains_rect(text_rect)
            && text_rect.right() < right
            && labels.iter().all(|r| !r.intersects(text_rect.expand(1.0)))
        {
            painter.galley(text_rect.min, galley, color);
            labels.push(text_rect);
        }
        if options.fingers && n.finger > 0 {
            painter.text(
                Pos2::new(xx, yy - staff.step * 7.0),
                Align2::CENTER_BOTTOM,
                n.finger,
                FontId::proportional(10.0),
                color,
            );
        }
        ui.interact(
            Rect::from_center_size(center, Vec2::splat(16.0)),
            ui.id().with(("sheet-note", index)),
            Sense::hover(),
        )
        .on_hover_text(format!(
            "{name} · {:.2}–{:.2} с{}",
            n.start,
            n.end(),
            if carried {
                " · продолжение ноты"
            } else {
                ""
            }
        ));
    }
    let current = grid.beat_position(position.max(0.0));
    if current >= page.start && current <= page.end {
        painter.line_segment(
            [
                Pos2::new(x(current), rect.top() + 25.0),
                Pos2::new(x(current), rect.bottom() - 23.0),
            ],
            Stroke::new(1.8_f32, Color32::from_rgb(23, 177, 183)),
        );
    }
    let names: Vec<_> = song
        .notes
        .iter()
        .filter(|n| n.start <= position && n.end() > position)
        .map(|n| options.language.name(n.pitch))
        .collect();
    let footer = if names.is_empty() {
        "Учебный лист по MIDI · длительность — тонкая линия".into()
    } else {
        format!("Сейчас: {}", names.join(" · "))
    };
    let max_chars = ((width - 24.0) / 7.0) as usize;
    let short = if footer.chars().count() > max_chars {
        format!(
            "{}…",
            footer
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        footer.clone()
    };
    painter.text(
        rect.left_bottom() + Vec2::new(12.0, -6.0),
        Align2::LEFT_BOTTOM,
        short,
        FontId::proportional(11.0),
        ink,
    );
    response.on_hover_text(format!("{footer}\nЛист построен из MIDI-высот и времени. Длительности округлены для учебного изображения; тонкая линия показывает фактическую длину. Исходные тональность, паузы и распределение голосов не восстанавливаются. Все знаки альтерации указаны явно."));
    rect
}

fn accidental(p: &egui::Painter, center: Pos2, sharp: bool, color: Color32) {
    let lines = if sharp {
        [
            [[-2.0, -8.0], [-4.0, 8.0]],
            [[4.0, -8.0], [2.0, 8.0]],
            [[-7.0, -1.0], [7.0, -3.0]],
            [[-7.0, 5.0], [7.0, 3.0]],
        ]
    } else {
        [
            [[-2.0, -8.0], [-2.0, 4.0]],
            [[2.0, -3.0], [2.0, 9.0]],
            [[-2.0, -1.0], [2.0, -3.0]],
            [[-2.0, 4.0], [2.0, 2.0]],
        ]
    };
    for [[x1, y1], [x2, y2]] in lines {
        p.line_segment(
            [center + Vec2::new(x1, y1), center + Vec2::new(x2, y2)],
            Stroke::new(1.1_f32, color),
        );
    }
}
fn smooth(points: &[Pos2]) -> Vec<Pos2> {
    let mut out = Vec::with_capacity(points.len() * 8);
    for i in 0..points.len() - 1 {
        let a = points[i.saturating_sub(1)].to_vec2();
        let b = points[i].to_vec2();
        let c = points[i + 1].to_vec2();
        let d = points[(i + 2).min(points.len() - 1)].to_vec2();
        for step in 0..8 {
            let t = step as f32 / 8.0;
            let v = (2.0 * b
                + (-a + c) * t
                + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                + (-a + 3.0 * b - 3.0 * c + d) * t * t * t)
                * 0.5;
            out.push(Pos2::new(v.x, v.y));
        }
    }
    out.push(*points.last().unwrap());
    out
}

fn clef(p: &egui::Painter, origin: Pos2, height: f32, treble: bool, color: Color32) {
    let scale = height / 32.0;
    let point = |x: f32, y: f32| origin + Vec2::new(x * scale, y * scale);
    if treble {
        let points = [
            (2., 42.),
            (-3., 39.),
            (-2., 34.),
            (4., 35.),
            (6., 39.),
            (4., 43.),
            (0., 43.),
            (-2., 39.),
            (0., 28.),
            (4., 15.),
            (5., 2.),
            (1., -7.),
            (-3., -3.),
            (-4., 4.),
            (-1., 11.),
            (5., 17.),
            (9., 23.),
            (8., 29.),
            (3., 33.),
            (-4., 31.),
            (-8., 26.),
            (-8., 20.),
            (-4., 16.),
            (2., 15.),
            (6., 19.),
            (5., 24.),
            (1., 26.),
            (-2., 24.),
            (-2., 21.),
        ];
        p.add(Shape::line(
            smooth(
                &points
                    .into_iter()
                    .map(|(x, y)| point(x, y))
                    .collect::<Vec<_>>(),
            ),
            Stroke::new(1.8_f32, color),
        ));
    } else {
        p.circle_filled(point(-5., 8.), 2.8 * scale, color);
        let points = [
            (-7., 6.),
            (-5., 2.),
            (0., 0.),
            (5., 3.),
            (7., 8.),
            (6., 14.),
            (2., 20.),
            (-5., 26.),
        ];
        p.add(Shape::line(
            smooth(
                &points
                    .into_iter()
                    .map(|(x, y)| point(x, y))
                    .collect::<Vec<_>>(),
            ),
            Stroke::new(2.0_f32, color),
        ));
        for y in [4., 12.] {
            p.circle_filled(point(12., y), 1.7 * scale, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_midi_pitch_stays_inside_its_staff_region_and_reference_lines_are_correct() {
        let song=Song::from_json(r#"{"title":"range","notes":[{"pitch":0,"start":0,"duration":1},{"pitch":127,"start":0,"duration":1}]}"#).unwrap();
        let notes: Vec<_> = song.notes.iter().collect();
        let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 130.0));
        for treble in [false, true] {
            let staff = Staff::new(region, treble, &notes);
            for pitch in 0..=127 {
                assert!(
                    region
                        .y_range()
                        .contains(staff.y(notation::diatonic(pitch)))
                );
            }
            assert!(staff.top() < staff.bottom());
            assert!(
                (staff.bottom() - staff.y(staff.reference + 2) - staff.step * 2.0).abs() < 1e-4
            );
        }
    }
    #[test]
    fn dense_chromatic_chords_render_in_both_languages_and_themes_without_panicking() {
        let mut song = clavunora_core::catalog::course()[0].1.clone();
        let template = song.notes[0].clone();
        song.notes = (0..=127)
            .map(|pitch| Note {
                pitch,
                start: 0.0,
                duration: 8.0,
                ..template.clone()
            })
            .collect();
        let grid = BeatGrid::new(&song);
        let ctx = egui::Context::default();
        for language in [Language::Russian, Language::English] {
            for dark in [false, true] {
                let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let rect = show(
                        ui,
                        &song,
                        &grid,
                        1.0,
                        Options {
                            language,
                            dark,
                            fingers: true,
                            height: 260.0,
                            page: None,
                        },
                    );
                    assert!(rect.is_positive());
                });
            }
        }
    }
}
