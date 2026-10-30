// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Bounded Standard MIDI import with one global tempo map, matching the legacy loader.
use crate::score::{Note, Song};
use anyhow::{Context, Result, ensure};
use midly::{Format, MetaMessage, MidiMessage, Timing, TrackEventKind};
use std::collections::{BTreeMap, VecDeque};
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_EVENTS: usize = 500_000;
pub struct Import {
    pub song: Song,
    pub warnings: Vec<String>,
}
pub fn parse(bytes: &[u8], title: &str) -> Result<Import> {
    ensure!(bytes.len() <= MAX_BYTES, "MIDI-файл больше 32 МБ");
    let (header, tracks) = midly::parse(bytes).context("Не удалось прочитать заголовок MIDI")?;
    ensure!(
        header.format != Format::Sequential,
        "MIDI type 2 не поддерживается: независимые последовательности нужно разделить"
    );
    let Timing::Metrical(ticks) = header.timing else {
        anyhow::bail!("SMPTE timing пока не поддерживается");
    };
    let ticks = ticks.as_int();
    ensure!(ticks > 0, "Некорректное разрешение MIDI");
    let mut events = vec![];
    let mut names = vec![];
    for (track_id, track) in tracks.enumerate() {
        ensure!(track_id < 10000, "Слишком много MIDI-дорожек");
        names.push(format!("Дорожка {}", track_id + 1));
        let mut time = 0u64;
        let mut named = false;
        for (index, event) in track?.enumerate() {
            let event = event.context("Повреждённое MIDI-событие")?;
            time = time
                .checked_add(u64::from(event.delta.as_int()))
                .context("Слишком длинная MIDI-дорожка")?;
            if let TrackEventKind::Meta(MetaMessage::TrackName(name)) = event.kind
                && !named
            {
                let value = String::from_utf8_lossy(name);
                if !value.trim().is_empty() {
                    names[track_id] = value.chars().take(1000).collect();
                }
                named = true;
            }
            ensure!(
                events.len() < MAX_EVENTS,
                "Слишком много MIDI-событий (лимит 500 000)"
            );
            events.push((time, track_id, index, event.kind));
        }
    }
    ensure!(
        header.format != Format::SingleTrack || names.len() == 1,
        "MIDI type 0 должен содержать одну дорожку"
    );
    events.sort_by_key(|e| (e.0, e.1, e.2));
    let mut tempo = 500000u32;
    let mut initial = None;
    let mut tempo_map = vec![[0.0, 120.0]];
    let mut signature = [4, 4];
    let mut signature_seen = false;
    let mut last_tick = 0;
    let mut seconds = 0.0;
    let mut pending: BTreeMap<(usize, u8, u8), VecDeque<(f64, u8)>> = BTreeMap::new();
    let mut notes = vec![];
    let mut warnings = vec![];
    let mut pedal = false;
    let mut expression = false;
    let mut unmatched = 0;
    for (tick, track, _, kind) in events {
        seconds += (tick - last_tick) as f64 / f64::from(ticks) * f64::from(tempo) / 1_000_000.0;
        last_tick = tick;
        match kind {
            TrackEventKind::Meta(MetaMessage::Tempo(value)) => {
                tempo = value.as_int();
                ensure!(tempo > 0, "Некорректный темп MIDI");
                let bpm = 60_000_000.0 / f64::from(tempo);
                if tempo_map.last().is_some_and(|p| p[0] == seconds) {
                    *tempo_map.last_mut().unwrap() = [seconds, bpm];
                } else {
                    tempo_map.push([seconds, bpm]);
                }
                if tick == 0 && initial.is_none() {
                    initial = Some(bpm);
                }
            }
            TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) if !signature_seen => {
                ensure!(n > 0 && d < 31, "Некорректный размер MIDI");
                signature = [u32::from(n), 1u32 << d];
                signature_seen = true;
            }
            TrackEventKind::Midi { channel, message } => {
                let channel = channel.as_int();
                let (pitch, on, velocity) = match message {
                    MidiMessage::NoteOn { key, vel } => {
                        (key.as_int(), vel.as_int() > 0, vel.as_int())
                    }
                    MidiMessage::NoteOff { key, .. } => (key.as_int(), false, 0),
                    MidiMessage::Controller { controller, .. } => {
                        pedal |= controller.as_int() == 64;
                        expression |= controller.as_int() != 64;
                        continue;
                    }
                    _ => {
                        expression = true;
                        continue;
                    }
                };
                let queue = pending.entry((track, channel, pitch)).or_default();
                if on {
                    queue.push_back((seconds, velocity));
                } else if let Some((start, velocity)) = queue.pop_front() {
                    notes.push(Note {
                        pitch,
                        start,
                        duration: (seconds - start).max(0.01),
                        velocity,
                        track,
                        channel,
                        hand: String::new(),
                        finger: 0,
                    });
                } else {
                    unmatched += 1;
                }
                ensure!(
                    notes.len() <= 100000,
                    "В MIDI слишком много нот (лимит 100 000)"
                );
            }
            _ => {}
        }
    }
    let mut unclosed = 0;
    for ((track, channel, pitch), queue) in pending {
        for (start, velocity) in queue {
            ensure!(
                notes.len() < 100000,
                "В MIDI слишком много нот (лимит 100 000)"
            );
            unclosed += 1;
            notes.push(Note {
                pitch,
                start,
                duration: (seconds - start).max(0.1),
                velocity,
                track,
                channel,
                hand: String::new(),
                finger: 0,
            });
        }
    }
    ensure!(!notes.is_empty(), "В файле нет нот");
    notes.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.track.cmp(&b.track))
            .then(a.pitch.cmp(&b.pitch))
    });
    if unclosed > 0 {
        warnings.push(format!(
            "Незакрытых нот: {unclosed}. Их конец ограничен концом файла."
        ));
    }
    if unmatched > 0 {
        warnings.push(format!(
            "Отпусканий без соответствующего нажатия: {unmatched}. Они пропущены."
        ));
    }
    if pedal {
        warnings.push(
            "Педаль Sustain не превращается в длительности нот; сохраняются нажатия и отпускания."
                .into(),
        );
    }
    if notes.iter().any(|n| n.channel == 9) {
        warnings.push(
            "Есть ударные на MIDI-канале 10. При разучивании можно исключить их дорожку.".into(),
        );
    }
    warnings.push(
        "Руки и аппликатура в MIDI не определены. Их автоматическое назначение не выполняется."
            .into(),
    );
    if expression {
        warnings.push("Смена инструментов, pitch bend и другие MIDI-эффекты не переносятся: импорт предназначен для разучивания нот.".into());
    }
    let song = Song {
        title: title.chars().take(500).collect(),
        notes,
        bpm: initial.unwrap_or(120.0),
        time_signature: signature,
        track_names: names,
        difficulty: 1,
        composer: String::new(),
        work: String::new(),
        description: String::new(),
        source: "Пользовательский MIDI-файл".into(),
        tempo_map,
    };
    song.validate()?;
    Ok(Import { song, warnings })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn file(track: &[u8]) -> Vec<u8> {
        let mut b = b"MThd\0\0\0\x06\0\0\0\x01\x01\xe0MTrk".to_vec();
        b.extend_from_slice(&(track.len() as u32).to_be_bytes());
        b.extend_from_slice(track);
        b
    }
    #[test]
    fn global_tempo_map_and_channels_are_independent() {
        let conductor = &[
            0, 0xff, 0x51, 3, 7, 0xa1, 0x20, 0x83, 0x60, 0xff, 0x51, 3, 3, 0xd0, 0x90, 0, 0xff,
            0x2f, 0,
        ];
        let notes = &[
            0, 0x90, 60, 90, 0, 0x91, 60, 80, 0x83, 0x60, 0x80, 60, 0, 0x83, 0x60, 0x81, 60, 0, 0,
            0xff, 0x2f, 0,
        ];
        let mut data = b"MThd\0\0\0\x06\0\x01\0\x02\x01\xe0".to_vec();
        for track in [conductor.as_slice(), notes.as_slice()] {
            data.extend_from_slice(b"MTrk");
            data.extend_from_slice(&(track.len() as u32).to_be_bytes());
            data.extend_from_slice(track);
        }
        let i = parse(&data, "Tempo").unwrap();
        assert_eq!(i.song.notes.len(), 2);
        assert_eq!(i.song.notes[0].duration, 0.5);
        assert_eq!(i.song.notes[1].duration, 0.75);
        assert_eq!(i.song.notes[0].channel, 0);
        assert_eq!(i.song.notes[1].channel, 1);
        assert_eq!(i.song.tempo_map, vec![[0.0, 120.0], [0.5, 240.0]]);
    }
    #[test]
    fn tempo_velocity_zero_running_status_and_fifo_pairing() {
        let data = file(&[
            0, 0xff, 0x51, 3, 7, 0xa1, 0x20, 0, 0x90, 60, 90, 0x81, 0x70, 60, 80, 0x81, 0x70, 60,
            0, 0x81, 0x70, 0x80, 60, 0, 0, 0xff, 0x2f, 0,
        ]);
        let i = parse(&data, "Test").unwrap();
        assert_eq!(i.song.notes.len(), 2);
        assert_eq!(i.song.notes[0].duration, 0.5);
        assert_eq!(i.song.notes[1].start, 0.25);
        assert_eq!(i.song.notes[1].duration, 0.5);
        assert_eq!(i.song.bpm, 120.0);
    }
    #[test]
    fn rejects_corrupt_empty_type2_smpte_and_warns_unclosed() {
        assert!(parse(b"garbage", "x").is_err());
        assert!(parse(&file(&[0, 0xff, 0x2f, 0]), "x").is_err());
        let mut data = file(&[0, 0x90, 60, 90, 0, 0xff, 0x2f, 0]);
        assert_eq!(parse(&data, "x").unwrap().song.notes[0].duration, 0.1);
        data[9] = 2;
        assert!(parse(&data, "x").is_err());
        data[9] = 0;
        data[12] = 0xe7;
        assert!(parse(&data, "x").is_err());
    }
}
