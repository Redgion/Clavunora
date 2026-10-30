// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use clavunora_core::{
    score::{Song, note_name},
    store::Record,
};
use eframe::egui::{self, Id};

pub enum Action {
    Keep,
    Save,
    Cancel,
}

/// The draft is isolated from the library and active practice until explicitly saved.
pub struct Editor {
    pub id: String,
    pub version: String,
    pub original: Song,
    pub song: Song,
    pub error: String,
    pub discard: bool,
    track: Option<usize>,
    selected: usize,
    hand: String,
    finger: u8,
    track_hand: String,
    undo: Vec<Vec<(usize, String, u8)>>,
}
impl Editor {
    pub fn new(record: &Record) -> Self {
        let note = &record.song.notes[0];
        Self {
            id: record.id.clone(),
            version: record.score_version.clone(),
            original: record.song.clone(),
            song: record.song.clone(),
            error: String::new(),
            discard: false,
            track: None,
            selected: 0,
            hand: note.hand.clone(),
            finger: note.finger,
            track_hand: String::new(),
            undo: vec![],
        }
    }
    pub fn dirty(&self) -> bool {
        self.song != self.original
    }
    pub fn assign(&mut self, indices: &[usize], hand: &str, finger: Option<u8>) {
        if !["", "L", "R"].contains(&hand) || finger.is_some_and(|f| f > 5) {
            return;
        }
        let mut changes = vec![];
        for &index in indices {
            if let Some(n) = self.song.notes.get_mut(index)
                && (n.hand != hand || finger.is_some_and(|f| n.finger != f))
            {
                changes.push((index, n.hand.clone(), n.finger));
                n.hand = hand.into();
                if let Some(f) = finger {
                    n.finger = f;
                }
            }
        }
        if !changes.is_empty() {
            // Cap both operations and retained note entries for large imported scores.
            self.undo.push(changes);
            while self.undo.len() > 50
                || (self.undo.len() > 1 && self.undo.iter().map(Vec::len).sum::<usize>() > 200_000)
            {
                self.undo.remove(0);
            }
        }
        self.select(self.selected);
    }
    pub fn undo(&mut self) {
        if let Some(changes) = self.undo.pop() {
            for (i, hand, finger) in changes {
                self.song.notes[i].hand = hand;
                self.song.notes[i].finger = finger;
            }
            self.select(self.selected);
        }
    }
    fn select(&mut self, index: usize) {
        self.selected = index;
        self.hand = self.song.notes[index].hand.clone();
        self.finger = self.song.notes[index].finger;
    }
    pub fn show(&mut self, ctx: &egui::Context) -> Action {
        let mut action = Action::Keep;
        let width = (ctx.content_rect().width() - 70.0).clamp(250.0, 820.0);
        let height = (ctx.content_rect().height() - 100.0).max(180.0);
        let area = egui::Modal::default_area(Id::new("score-editor"))
            .default_size(egui::vec2(width + 16.0, height + 16.0));
        egui::Modal::new(Id::new("score-editor"))
            .area(area)
            .show(ctx, |ui| {
                ui.set_width(width);
                egui::ScrollArea::vertical()
                    .id_salt("editor-body")
                    .max_height((ctx.content_rect().height() - 100.0).max(180.0))
                    .show(ui, |ui| {
                        ui.heading("Руки и аппликатура");
                        ui.add(egui::Label::new(&self.song.title).wrap());
                        ui.label("Выберите ноту, назначьте руку и палец. 0 — палец не указан.");
                        ui.small(
                            "Меняется только разметка; высота, время и звучание нот сохраняются.",
                        );
                        if !self.error.is_empty() {
                            ui.colored_label(egui::Color32::from_rgb(190, 55, 40), &self.error);
                        }
                        let tracks: std::collections::BTreeSet<usize> =
                            self.song.notes.iter().map(|n| n.track).collect();
                        ui.horizontal_wrapped(|ui| {
                            egui::ComboBox::from_id_salt("editor-track")
                                .selected_text(
                                    self.track.map_or_else(
                                        || "Все дорожки".into(),
                                        |t| self.track_name(t),
                                    ),
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.track, None, "Все дорожки");
                                    for t in tracks {
                                        let name = self.track_name(t);
                                        ui.selectable_value(&mut self.track, Some(t), name);
                                    }
                                });
                            hand_picker(ui, "batch-hand", &mut self.track_hand);
                            if ui
                                .add_enabled(
                                    self.track.is_some(),
                                    egui::Button::new("Назначить руку дорожке"),
                                )
                                .clicked()
                            {
                                let indices: Vec<_> = self
                                    .song
                                    .notes
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, n)| Some(n.track) == self.track)
                                    .map(|(i, _)| i)
                                    .collect();
                                self.assign(&indices, &self.track_hand.clone(), None);
                            }
                        });
                        ui.small("Назначение дорожке сохраняет уже указанные пальцы.");
                        let visible: Vec<usize> = self
                            .song
                            .notes
                            .iter()
                            .enumerate()
                            .filter(|(_, n)| self.track.is_none_or(|t| n.track == t))
                            .map(|(i, _)| i)
                            .collect();
                        if !visible.contains(&self.selected)
                            && let Some(&i) = visible.first()
                        {
                            self.select(i);
                        }
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .id_salt("editor-notes")
                            .max_height(230.0)
                            .show_rows(ui, 32.0, visible.len(), |ui, range| {
                                for row in range {
                                    let index = visible[row];
                                    let n = &self.song.notes[index];
                                    let label = format!(
                                        "{:>4} · {:>7.2} с · {:<4} · {} · {} · палец {}",
                                        index + 1,
                                        n.start,
                                        note_name(n.pitch),
                                        self.track_name(n.track),
                                        hand_name(&n.hand),
                                        n.finger
                                    );
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 32.0],
                                            egui::Button::selectable(self.selected == index, label)
                                                .truncate(),
                                        )
                                        .clicked()
                                    {
                                        self.select(index);
                                    }
                                }
                            });
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!(
                                "Нота {}: {}",
                                self.selected + 1,
                                note_name(self.song.notes[self.selected].pitch)
                            ));
                            hand_picker(ui, "note-hand", &mut self.hand);
                            ui.label("Палец");
                            ui.add(egui::DragValue::new(&mut self.finger).range(0..=5));
                            if ui.button("Применить к ноте").clicked() {
                                self.assign(
                                    &[self.selected],
                                    &self.hand.clone(),
                                    Some(self.finger),
                                );
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    !self.undo.is_empty(),
                                    egui::Button::new("Отменить изменение"),
                                )
                                .clicked()
                            {
                                self.undo();
                            }
                            ui.label(if self.dirty() {
                                "Есть несохранённые изменения"
                            } else {
                                "Изменений нет"
                            });
                        });
                        ui.separator();
                        if self.discard {
                            ui.label("Отменить все несохранённые изменения?");
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Продолжить редактирование").clicked()
                                {
                                    self.discard = false;
                                }
                                if ui.button("Не сохранять и закрыть").clicked()
                                {
                                    action = Action::Cancel;
                                }
                            });
                        } else {
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .add_enabled(
                                        self.dirty(),
                                        egui::Button::new("Сохранить разметку"),
                                    )
                                    .clicked()
                                {
                                    action = Action::Save;
                                }
                                if ui.button("Закрыть").clicked() {
                                    if self.dirty() {
                                        self.discard = true;
                                    } else {
                                        action = Action::Cancel;
                                    }
                                }
                            });
                        }
                    });
            });
        action
    }
    fn track_name(&self, t: usize) -> String {
        self.song
            .track_names
            .get(t)
            .cloned()
            .unwrap_or_else(|| format!("Дорожка {}", t + 1))
    }
}
fn hand_name(hand: &str) -> &'static str {
    match hand {
        "L" => "Левая",
        "R" => "Правая",
        _ => "Не указана",
    }
}
fn hand_picker(ui: &mut egui::Ui, id: &str, hand: &mut String) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(hand_name(hand))
        .show_ui(ui, |ui| {
            for (key, label) in [("", "Не указана"), ("L", "Левая"), ("R", "Правая")]
            {
                ui.selectable_value(hand, key.into(), label);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_are_isolated_and_batch_preserves_fingers_and_undo() {
        let song = clavunora_core::catalog::course().remove(0).1;
        let record = Record {
            id: "test".into(),
            song: song.clone(),
            favorite: true,
            score_version: "v".into(),
        };
        let mut e = Editor::new(&record);
        let fingers: Vec<_> = song.notes.iter().map(|n| n.finger).collect();
        let indices: Vec<_> = (0..song.notes.len()).collect();
        e.assign(&indices, "L", None);
        assert!(e.dirty());
        assert_eq!(record.song, song);
        assert_eq!(
            e.song.notes.iter().map(|n| n.finger).collect::<Vec<_>>(),
            fingers
        );
        e.assign(&[0], "R", Some(4));
        assert_eq!(e.song.notes[0].finger, 4);
        e.undo();
        assert_eq!(e.song.notes[0].hand, "L");
        e.undo();
        assert!(!e.dirty());
        e.assign(&[0], "invalid", Some(1));
        e.assign(&[0], "L", Some(6));
        assert!(!e.dirty());
    }
}
