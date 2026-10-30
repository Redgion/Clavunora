// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Bounded, non-network MusicXML partwise import for one piano part.
use crate::{
    midi_file::Import,
    score::{Note, Song},
};
use anyhow::{Context, Result, ensure};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Default)]
struct Node {
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<Node>,
}
impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.name == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.child(name).map(|n| n.text.trim())
    }
    fn number(&self, name: &str, default: f64) -> Result<f64> {
        let v = self
            .value(name)
            .map(str::parse::<f64>)
            .transpose()?
            .unwrap_or(default);
        ensure!(v.is_finite(), "Некорректное число MusicXML");
        Ok(v)
    }
    fn find(&self, name: &str) -> Option<&Node> {
        if self.name == name {
            Some(self)
        } else {
            self.children.iter().find_map(|c| c.find(name))
        }
    }
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(|s| s.trim())
    }
}
fn tree(raw: &[u8]) -> Result<Node> {
    ensure!(raw.len() <= MAX_BYTES, "MusicXML больше 16 МБ");
    let xml = std::str::from_utf8(raw).context("MusicXML должен быть в UTF-8")?;
    ensure!(
        !xml.to_ascii_uppercase().contains("<!ENTITY"),
        "XML с объявлениями сущностей не поддерживается"
    );
    let mut reader = Reader::from_str(xml);
    let mut stack = vec![Node::default()];
    let mut count = 0;
    loop {
        let event = reader.read_event().context("Некорректный XML")?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                count += 1;
                ensure!(
                    count <= 500000 && stack.len() < 128,
                    "MusicXML слишком сложный"
                );
                let mut n = Node {
                    name: String::from_utf8(e.local_name().as_ref().to_vec())?,
                    ..Default::default()
                };
                for a in e.attributes() {
                    let a = a?;
                    n.attrs.insert(
                        String::from_utf8(a.key.as_ref().to_vec())?,
                        a.decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )?
                        .into_owned(),
                    );
                }
                if empty {
                    stack.last_mut().unwrap().children.push(n);
                } else {
                    stack.push(n);
                }
            }
            Event::End(_) => {
                ensure!(stack.len() > 1, "Некорректная структура XML");
                let n = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(n);
            }
            Event::Text(e) => {
                stack
                    .last_mut()
                    .unwrap()
                    .text
                    .push_str(&quick_xml::escape::unescape(
                        &e.xml_content(quick_xml::XmlVersion::Implicit1_0)?,
                    )?);
            }
            Event::CData(e) => stack
                .last_mut()
                .unwrap()
                .text
                .push_str(&e.xml_content(quick_xml::XmlVersion::Implicit1_0)?),
            Event::GeneralRef(e) => {
                let text = format!("&{};", e.decode()?);
                stack
                    .last_mut()
                    .unwrap()
                    .text
                    .push_str(&quick_xml::escape::unescape(&text)?);
            }
            Event::Decl(e) => {
                ensure!(e.version()?.as_ref() == b"1.0", "Поддерживается XML 1.0");
                if let Some(encoding) = e.encoding() {
                    let encoding = encoding?;
                    ensure!(
                        encoding.eq_ignore_ascii_case(b"utf-8")
                            || encoding.eq_ignore_ascii_case(b"utf8"),
                        "MusicXML должен быть в UTF-8"
                    );
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure!(stack.len() == 1, "Незавершённый XML");
    let mut wrapper = stack.pop().unwrap();
    ensure!(
        wrapper.text.trim().is_empty(),
        "Текст за пределами корня XML"
    );
    ensure!(
        wrapper.children.len() == 1,
        "Нужен один корневой элемент XML"
    );
    Ok(wrapper.children.remove(0))
}
struct RawNote {
    pitch: u8,
    start: f64,
    duration: f64,
    staff: usize,
    finger: u8,
}
pub fn parse(raw: &[u8], fallback: &str) -> Result<Import> {
    let root = tree(raw)?;
    ensure!(
        root.name == "score-partwise",
        "Нужен несжатый MusicXML score-partwise (.musicxml или .xml)"
    );
    let parts: Vec<_> = root.children.iter().filter(|n| n.name == "part").collect();
    ensure!(
        parts.len() == 1,
        "Поддерживается одна партия фортепиано. Экспортируйте её отдельно"
    );
    for name in [
        "repeat",
        "ending",
        "grace",
        "transpose",
        "unpitched",
        "measure-repeat",
        "cue",
        "octave-shift",
        "senza-misura",
        "interchangeable",
    ] {
        ensure!(
            root.find(name).is_none(),
            "MusicXML: {name} пока не поддерживается; используйте развёрнутый MIDI"
        );
    }
    fn jumps(n: &Node) -> bool {
        (n.name == "sound"
            && ["dacapo", "dalsegno", "tocoda", "fine"]
                .iter()
                .any(|k| n.attr(k).is_some()))
            || n.children.iter().any(jumps)
    }
    ensure!(!jumps(&root), "Переходы D.C./D.S. пока не поддерживаются");
    let mut divisions = 1.0;
    let mut meter = [4, 4];
    let mut initial_meter = None;
    let mut measure_start = 0.0;
    let mut changes = vec![(0.0, 120.0)];
    let mut notes: Vec<RawNote> = vec![];
    let mut ties: BTreeMap<(String, usize, u8), usize> = BTreeMap::new();
    let mut max_staff = 1;
    for measure in parts[0].children.iter().filter(|n| n.name == "measure") {
        let mut cursor = measure_start;
        let mut furthest = cursor;
        let mut previous = None;
        for e in &measure.children {
            match e.name.as_str() {
                "attributes" => {
                    divisions = e.number("divisions", divisions)?;
                    ensure!(divisions > 0.0, "Некорректные divisions");
                    if let Some(t) = e.child("time") {
                        let n = t.number("beats", 4.0)?;
                        let d = t.number("beat-type", 4.0)?;
                        ensure!(
                            n >= 1.0
                                && n.fract() == 0.0
                                && n <= 128.0
                                && [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0].contains(&d),
                            "Неподдерживаемый размер"
                        );
                        meter = [n as u32, d as u32];
                        if let Some(old) = initial_meter {
                            ensure!(old == meter, "Смена размера пока не поддерживается");
                        } else {
                            initial_meter = Some(meter);
                        }
                    }
                }
                "direction" | "sound" => {
                    let sound = if e.name == "sound" {
                        Some(e)
                    } else {
                        e.child("sound")
                    };
                    let bpm = if let Some(value) = sound.and_then(|s| s.attr("tempo")) {
                        Some(value.parse::<f64>()?)
                    } else if let Some(m) = e.find("metronome") {
                        let unit = match m.value("beat-unit") {
                            Some("whole") => 4.0,
                            Some("half") => 2.0,
                            Some("quarter") => 1.0,
                            Some("eighth") => 0.5,
                            Some("16th") => 0.25,
                            _ => anyhow::bail!("Неподдерживаемая единица метронома"),
                        };
                        let dots = m
                            .children
                            .iter()
                            .filter(|n| n.name == "beat-unit-dot")
                            .count();
                        ensure!(dots <= 4, "Слишком много точек метронома");
                        Some(
                            m.number("per-minute", 0.0)?
                                * unit
                                * (0..=dots).map(|i| 0.5_f64.powi(i as i32)).sum::<f64>(),
                        )
                    } else {
                        None
                    };
                    if let Some(bpm) = bpm {
                        ensure!(bpm.is_finite() && bpm > 0.0, "Некорректный темп");
                        let at = cursor + e.number("offset", 0.0)? / divisions;
                        ensure!(at >= 0.0, "Некорректное смещение темпа");
                        changes.push((at, bpm));
                    }
                }
                "backup" | "forward" => {
                    let d = e.number("duration", 0.0)? / divisions;
                    ensure!(d > 0.0, "Некорректная длительность");
                    cursor += if e.name == "backup" { -d } else { d };
                    ensure!(
                        cursor + 1e-8 >= measure_start,
                        "Голос выходит за начало такта"
                    );
                    furthest = furthest.max(cursor);
                }
                "note" => {
                    let duration = e.number("duration", 0.0)? / divisions;
                    ensure!(duration > 0.0, "Нота без положительной длительности");
                    let start = if e.child("chord").is_some() {
                        previous.context("Аккорд без первой ноты")?
                    } else {
                        let start = cursor;
                        previous = Some(start);
                        cursor += duration;
                        start
                    };
                    furthest = furthest.max(start + duration).max(cursor);
                    if e.child("rest").is_some() {
                        continue;
                    }
                    let p = e.child("pitch").context("Нота без высоты")?;
                    let step = match p.value("step") {
                        Some("C") => 0,
                        Some("D") => 2,
                        Some("E") => 4,
                        Some("F") => 5,
                        Some("G") => 7,
                        Some("A") => 9,
                        Some("B") => 11,
                        _ => anyhow::bail!("Неизвестная высота ноты"),
                    };
                    let alter = p.number("alter", 0.0)?;
                    let octave = p.number("octave", f64::NAN)?;
                    ensure!(
                        alter.fract() == 0.0 && octave.fract() == 0.0,
                        "Микротональные ноты не поддерживаются"
                    );
                    let pitch = (octave + 1.0) * 12.0 + f64::from(step) + alter;
                    ensure!((0.0..=127.0).contains(&pitch), "Высота вне MIDI-диапазона");
                    let pitch = pitch as u8;
                    let staff = e.number("staff", 1.0)?;
                    ensure!(
                        (1.0..=16.0).contains(&staff) && staff.fract() == 0.0,
                        "Некорректный стан"
                    );
                    let staff = staff as usize;
                    max_staff = max_staff.max(staff);
                    let finger = e
                        .child("notations")
                        .and_then(|n| n.child("technical"))
                        .and_then(|n| n.value("fingering"))
                        .and_then(|f| f.parse::<u8>().ok())
                        .filter(|f| (1..=5).contains(f))
                        .unwrap_or(0);
                    let key = (e.value("voice").unwrap_or("1").to_string(), staff, pitch);
                    let stop = e
                        .children
                        .iter()
                        .any(|n| n.name == "tie" && n.attr("type") == Some("stop"));
                    let begin = e
                        .children
                        .iter()
                        .any(|n| n.name == "tie" && n.attr("type") == Some("start"));
                    if stop {
                        let index = *ties.get(&key).context("Лига без начала")?;
                        let old = &mut notes[index];
                        ensure!(
                            (old.start + old.duration - start).abs() < 1e-8,
                            "Несмежные связанные ноты"
                        );
                        old.duration += duration;
                        if !begin {
                            ties.remove(&key);
                        }
                    } else {
                        ensure!(
                            !ties.contains_key(&key),
                            "Повторное начало незавершённой лиги"
                        );
                        notes.push(RawNote {
                            pitch,
                            start,
                            duration,
                            staff,
                            finger,
                        });
                        if begin {
                            ties.insert(key, notes.len() - 1);
                        }
                    }
                    ensure!(notes.len() <= 100000, "Слишком много нот");
                }
                _ => {}
            }
        }
        let length = f64::from(meter[0]) * 4.0 / f64::from(meter[1]);
        measure_start = if measure.attr("implicit") == Some("yes") {
            furthest
        } else {
            furthest.max(measure_start + length)
        };
    }
    ensure!(ties.is_empty(), "Незавершённая лига");
    ensure!(!notes.is_empty(), "В файле нет нот");
    changes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut tempo: Vec<(f64, f64)> = vec![];
    for point in changes {
        if tempo.last().is_some_and(|p| (p.0 - point.0).abs() < 1e-10) {
            *tempo.last_mut().unwrap() = point;
        } else {
            tempo.push(point);
        }
    }
    let mut prefix = vec![0.0; tempo.len()];
    for i in 1..tempo.len() {
        prefix[i] = prefix[i - 1] + (tempo[i].0 - tempo[i - 1].0) * 60.0 / tempo[i - 1].1;
    }
    let seconds = |beat: f64| {
        if !beat.is_finite() {
            return f64::NAN;
        }
        let i = tempo.partition_point(|p| p.0 <= beat).saturating_sub(1);
        prefix[i] + (beat - tempo[i].0) * 60.0 / tempo[i].1
    };
    let mut output: Vec<_> = notes
        .into_iter()
        .map(|n| Note {
            pitch: n.pitch,
            start: seconds(n.start),
            duration: seconds(n.start + n.duration) - seconds(n.start),
            velocity: 88,
            track: n.staff - 1,
            channel: 0,
            hand: String::new(),
            finger: n.finger,
        })
        .collect();
    output.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.track.cmp(&b.track))
            .then(a.pitch.cmp(&b.pitch))
    });
    let title = root
        .child("work")
        .and_then(|n| n.value("work-title"))
        .or_else(|| root.value("movement-title"))
        .unwrap_or(fallback);
    let composer = root
        .child("identification")
        .and_then(|n| {
            n.children
                .iter()
                .find(|c| c.name == "creator" && c.attr("type") == Some("composer"))
        })
        .map_or("", |n| n.text.trim());
    let song = Song {
        title: title.chars().take(500).collect(),
        notes: output,
        bpm: tempo[0].1,
        time_signature: initial_meter.unwrap_or([4, 4]),
        track_names: (1..=max_staff).map(|i| format!("Стан {i}")).collect(),
        difficulty: 1,
        composer: composer.chars().take(100000).collect(),
        work: String::new(),
        description:
            "Импорт MusicXML. Сохраняются ноты и разметка, но не исходная вёрстка партитуры.".into(),
        source: "Пользовательский MusicXML".into(),
        tempo_map: tempo.iter().map(|(at, bpm)| [seconds(*at), *bpm]).collect(),
    };
    song.validate()?;
    Ok(Import{song,warnings:vec!["Станы сохранены как дорожки. Руки автоматически не назначаются: стан не всегда соответствует руке.".into(),"Динамика, артикуляция, педаль и исходная вёрстка не воспроизводятся. Повторы, украшения и переходы нужно предварительно развернуть.".into()]})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chords_voices_tempo_ties_staff_and_fingerings() {
        let i = parse(
            include_bytes!("../../../../tests/fixtures/musicxml-import.musicxml"),
            "x",
        )
        .unwrap();
        assert_eq!(i.song.title, "Study");
        assert_eq!(i.song.notes.len(), 3);
        assert_eq!(i.song.tempo_map, vec![[0.0, 120.0], [0.5, 60.0]]);
        let note = i.song.notes.iter().find(|n| n.pitch == 60).unwrap();
        assert_eq!(note.duration, 1.5);
        assert_eq!(note.finger, 1);
        assert!(note.hand.is_empty());
        assert_eq!(
            i.song.notes.iter().find(|n| n.pitch == 48).unwrap().track,
            1
        );
    }
    #[test]
    fn rejects_entities_repeats_broken_ties_and_malformed_xml() {
        for xml in [
            "<!DOCTYPE x [<!ENTITY a 'x'>]><score-partwise/>",
            "<score-partwise><part><repeat/></part></score-partwise>",
            "<score-partwise><part></score-partwise>",
            "<score-timewise/>",
            "<score-partwise><part/><part/></score-partwise>",
        ] {
            assert!(parse(xml.as_bytes(), "x").is_err(), "{xml}");
        }
        let fixture = include_str!("../../../../tests/fixtures/musicxml-import.musicxml")
            .replace("<tie type=\"start\"/>", "");
        assert!(parse(fixture.as_bytes(), "x").is_err());
    }
    #[test]
    fn implicit_measures_and_escaped_titles() {
        let note =
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>";
        for (attr, start) in [("", 2.0), (" implicit='yes'", 0.5)] {
            let xml = format!(
                "<score-partwise><movement-title>A &amp; B</movement-title><part><measure{attr}>{note}</measure><measure>{note}</measure></part></score-partwise>"
            );
            let i = parse(xml.as_bytes(), "x").unwrap();
            assert_eq!(i.song.title, "A & B");
            assert_eq!(i.song.notes[1].start, start);
        }
    }
}
