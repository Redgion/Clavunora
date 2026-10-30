// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Original, deterministic exercises. No third-party scores or fingering guesses.
use crate::score::{Note, Song};
fn line(hand: &str, pitches: &[u8], fingers: &[u8], beats: &[f64], step: f64) -> Vec<Note> {
    assert_eq!(pitches.len(), fingers.len());
    assert_eq!(pitches.len(), beats.len());
    pitches
        .iter()
        .zip(fingers)
        .zip(beats)
        .map(|((&pitch, &finger), &start)| Note {
            pitch,
            finger,
            start: start * step,
            duration: step * 0.82,
            velocity: 88,
            track: usize::from(hand == "R"),
            channel: 0,
            hand: hand.into(),
        })
        .collect()
}
fn regular(hand: &str, pitches: &[u8], fingers: &[u8], spacing: f64, step: f64) -> Vec<Note> {
    line(
        hand,
        pitches,
        fingers,
        &(0..pitches.len())
            .map(|i| i as f64 * spacing)
            .collect::<Vec<_>>(),
        step,
    )
}
fn chord(
    hand: &str,
    pitches: &[u8],
    fingers: &[u8],
    beat: f64,
    duration: f64,
    step: f64,
) -> Vec<Note> {
    let mut notes = line(hand, pitches, fingers, &vec![beat; pitches.len()], step);
    for n in &mut notes {
        n.duration = duration * step;
    }
    notes
}
pub fn lessons() -> Vec<(String, Song)> {
    let mut out = vec![];
    let mut add = |title: &str,
                   description: &str,
                   chapter: &str,
                   bpm: f64,
                   meter: [u32; 2],
                   level: u8,
                   mut notes: Vec<Note>| {
        notes.sort_by(|a, b| {
            a.start
                .total_cmp(&b.start)
                .then(a.track.cmp(&b.track))
                .then(a.pitch.cmp(&b.pitch))
        });
        out.push((
            format!("course-{}", out.len() + 7),
            Song {
                title: title.into(),
                description: description.into(),
                work: chapter.into(),
                notes,
                bpm,
                time_signature: meter,
                track_names: vec!["Левая".into(), "Правая".into()],
                difficulty: level,
                composer: String::new(),
                source: "Собственное учебное упражнение Clavunora".into(),
                tempo_map: vec![],
            },
        ));
    };
    let step = 0.75;
    add(
        "Восьмые ноты",
        "Правая рука в позиции до–соль. Считайте «раз-и, два-и»: две равные ноты на одну долю. После ожидания повторите в режиме ритма.",
        "Ритм и интервалы",
        80.0,
        [4, 4],
        1,
        regular(
            "R",
            &[60, 62, 64, 65, 67, 65, 64, 62, 60],
            &[1, 2, 3, 4, 5, 4, 3, 2, 1],
            0.5,
            step,
        )
        .into_iter()
        .map(|mut n| {
            n.duration = step * 0.4;
            n
        })
        .collect(),
    );
    let mut long = regular("R", &[60, 62, 64, 67, 60], &[1, 2, 3, 5, 1], 2.0, step);
    for n in &mut long {
        n.duration = step * 1.8;
    }
    add(
        "Держим длинные ноты",
        "Нажимайте раз в две доли и держите почти до следующего нажатия. Контроль отпускания доступен в режиме ритма; ожидание не проверяет длительность.",
        "Ритм и интервалы",
        80.0,
        [4, 4],
        1,
        long,
    );
    let mut thirds = vec![];
    for (i, (p, f)) in [
        ([60, 64], [1, 3]),
        ([62, 65], [2, 4]),
        ([64, 67], [3, 5]),
        ([62, 65], [2, 4]),
        ([60, 64], [1, 3]),
    ]
    .iter()
    .enumerate()
    {
        thirds.extend(chord("R", p, f, i as f64, 0.8, step));
    }
    add(
        "Терции правой рукой",
        "Поставьте пальцы 1–5 на до–соль. Играйте пары 1–3, 2–4, 3–5 одновременно, не меняя положения руки.",
        "Ритм и интервалы",
        80.0,
        [4, 4],
        2,
        thirds,
    );
    let mut fifths = vec![];
    for j in 0..4 {
        fifths.extend(chord("L", &[48, 55], &[5, 1], j as f64 * 2.0, 1.5, step));
    }
    add(
        "Квинта левой рукой",
        "Левая рука: мизинец на до, большой на соль. Нажимайте обе клавиши одновременно раз в две доли, затем отпускайте вместе.",
        "Ритм и интервалы",
        80.0,
        [4, 4],
        2,
        fifths,
    );
    let mut short = regular(
        "R",
        &[60, 62, 64, 65, 67, 65, 64, 62, 60],
        &[1, 2, 3, 4, 5, 4, 3, 2, 1],
        1.0,
        step,
    );
    for n in &mut short {
        n.duration = step * 0.25;
    }
    add(
        "Короткий штрих",
        "Играйте короткие отдельные звуки в позиции до–соль. Между нажатиями оставляйте тишину. Для проверки удержания включите режим ритма.",
        "Ритм и интервалы",
        80.0,
        [4, 4],
        1,
        short,
    );
    let mut lc = vec![];
    for j in 0..4 {
        lc.extend(chord(
            "L",
            &[48, 52, 55],
            &[5, 3, 1],
            j as f64 * 2.0,
            1.6,
            step,
        ));
    }
    add(
        "Аккорд левой рукой",
        "До–ми–соль левой рукой: пальцы 5–3–1. Сначала найдите положение без напряжения, затем сыграйте аккорд четыре раза.",
        "Аккорды и гаммы",
        80.0,
        [4, 4],
        2,
        lc,
    );
    let mut am = vec![];
    for j in 0..4 {
        am.extend(chord(
            "R",
            &[57, 60, 64],
            &[1, 3, 5],
            j as f64 * 2.0,
            1.6,
            step,
        ));
    }
    add(
        "Ля-минорный аккорд",
        "Переместите правую руку: большой на ля, средний на до, мизинец на ми. Сыграйте ля–до–ми одновременно.",
        "Аккорды и гаммы",
        80.0,
        [4, 4],
        2,
        am,
    );
    let scale = [60, 62, 64, 65, 67, 69, 71, 72, 71, 69, 67, 65, 64, 62, 60];
    add(
        "До мажор: правая рука",
        "Вверх: 1–2–3, подкладываем большой под фа, затем 1–2–3–4–5. Вниз после фа переносим третий палец на ми. Начните медленно и без напряжения.",
        "Аккорды и гаммы",
        60.0,
        [4, 4],
        2,
        regular(
            "R",
            &scale,
            &[1, 2, 3, 1, 2, 3, 4, 5, 4, 3, 2, 1, 3, 2, 1],
            1.0,
            1.0,
        ),
    );
    let left: Vec<_> = scale.iter().map(|p| p - 12).collect();
    add(
        "До мажор: левая рука",
        "Вверх: 5–4–3–2–1, переносим третий палец на ля, затем 3–2–1. Вниз после ля подкладываем большой на соль. Отработайте смену позиции отдельно.",
        "Аккорды и гаммы",
        60.0,
        [4, 4],
        2,
        regular(
            "L",
            &left,
            &[5, 4, 3, 2, 1, 3, 2, 1, 2, 3, 1, 2, 3, 4, 5],
            1.0,
            1.0,
        ),
    );
    add(
        "Ля минор без знаков",
        "Натуральный ля минор: только белые клавиши. Вверх: 1–2–3–1–2–3–4–5. Вниз после ре переносим третий палец на до.",
        "Аккорды и гаммы",
        60.0,
        [4, 4],
        2,
        regular(
            "R",
            &[57, 59, 60, 62, 64, 65, 67, 69, 67, 65, 64, 62, 60, 59, 57],
            &[1, 2, 3, 1, 2, 3, 4, 5, 4, 3, 2, 1, 3, 2, 1],
            1.0,
            1.0,
        ),
    );
    let mut alternate = regular("L", &[48, 50, 52, 53, 55], &[5, 4, 3, 2, 1], 2.0, step);
    alternate.extend(line(
        "R",
        &[60, 62, 64, 65, 67],
        &[1, 2, 3, 4, 5],
        &[1., 3., 5., 7., 9.],
        step,
    ));
    add(
        "Перекличка рук",
        "Сначала играет левая рука, на следующую долю отвечает правая. Каждая остаётся в позиции до–соль. Не нажимайте обе руки вместе.",
        "Координация рук",
        80.0,
        [4, 4],
        2,
        alternate,
    );
    let mut contrary = regular(
        "L",
        &[55, 53, 52, 50, 48, 50, 52, 53, 55],
        &[1, 2, 3, 4, 5, 4, 3, 2, 1],
        1.0,
        step,
    );
    contrary.extend(regular(
        "R",
        &[60, 62, 64, 65, 67, 65, 64, 62, 60],
        &[1, 2, 3, 4, 5, 4, 3, 2, 1],
        1.0,
        step,
    ));
    add(
        "Руки в разные стороны",
        "Правая идёт от до к соль, левая одновременно от соль к до, затем обратно. Нажимайте пары одновременно; обе руки остаются в своих пяти нотах.",
        "Координация рук",
        80.0,
        [4, 4],
        2,
        contrary,
    );
    let mut bass = regular("L", &[48, 48, 48, 48], &[5, 5, 5, 5], 2.0, step);
    for n in &mut bass {
        n.duration = step * 1.7;
    }
    bass.extend(regular(
        "R",
        &[60, 62, 64, 65, 67, 65, 64, 60],
        &[1, 2, 3, 4, 5, 4, 3, 1],
        1.0,
        step,
    ));
    add(
        "Мелодия над длинным басом",
        "Левая держит до две доли, правая играет две четверти. Сначала разучите каждую руку отдельно, затем соедините. Режим ритма проверяет отпускания.",
        "Координация рук",
        80.0,
        [4, 4],
        2,
        bass,
    );
    add(
        "Разложенный аккорд",
        "Правая рука: до–ми–соль–ми, пальцы 1–3–5–3. Повторите рисунок трижды ровными четвертями; это аккорд, сыгранный по одной ноте.",
        "Первые музыкальные формы",
        80.0,
        [4, 4],
        2,
        regular(
            "R",
            &[60, 64, 67, 64, 60, 64, 67, 64, 60, 64, 67, 60],
            &[1, 3, 5, 3, 1, 3, 5, 3, 1, 3, 5, 1],
            1.0,
            step,
        ),
    );
    let mut waltz = vec![];
    for bar in 0..4 {
        let b = bar as f64 * 3.;
        waltz.extend(chord("L", &[48], &[5], b, 0.7, step));
        for beat in [1., 2.] {
            waltz.extend(chord("R", &[60, 64, 67], &[1, 3, 5], b + beat, 0.65, step));
        }
    }
    add(
        "Первый вальс",
        "Размер 3/4: считайте раз–два–три. На раз — бас левой, на два и три — аккорд правой. Это чередование рук, а не одновременная игра.",
        "Первые музыкальные формы",
        80.0,
        [3, 4],
        2,
        waltz,
    );
    let mut changes = vec![];
    for (i, (p, f)) in [
        ([60, 64, 67], [1, 3, 5]),
        ([57, 60, 64], [1, 3, 5]),
        ([59, 62, 67], [1, 2, 5]),
        ([60, 64, 67], [1, 3, 5]),
    ]
    .iter()
    .enumerate()
    {
        changes.extend(chord("R", p, f, i as f64 * 4., 3., 1.));
    }
    add(
        "Смена трёх созвучий",
        "Четыре такта: до мажор, ля минор, соль мажор в обращении, до мажор. После каждого аккорда есть доля для подготовки следующей позиции. Сначала найдите все положения без спешки.",
        "Первые музыкальные формы",
        60.0,
        [4, 4],
        3,
        changes,
    );
    let mut sync = line(
        "R",
        &[60, 62, 64, 65, 67, 65, 64, 60],
        &[1, 2, 3, 4, 5, 4, 3, 1],
        &[0., 1.5, 2.5, 4., 5.5, 6.5, 8., 10.],
        step,
    );
    for n in &mut sync {
        n.duration = step * 0.4;
    }
    add(
        "Знакомство с синкопой",
        "Считайте вслух раз-и-два-и-три-и-четыре-и. Часть нот попадает между долями. Сначала послушайте пример, затем играйте в режиме ритма на небольшой скорости.",
        "Первые музыкальные формы",
        80.0,
        [4, 4],
        3,
        sync,
    );
    let mut final_notes = regular(
        "R",
        &[
            60, 62, 64, 67, 65, 64, 62, 60, 64, 65, 67, 64, 62, 64, 62, 60,
        ],
        &[1, 2, 3, 5, 4, 3, 2, 1, 3, 4, 5, 3, 2, 3, 2, 1],
        1.0,
        step,
    );
    for (bar, p) in [48, 53, 55, 48].into_iter().enumerate() {
        final_notes.extend(chord("L", &[p], &[5], bar as f64 * 4., 3.5, step));
    }
    add(
        "Итоговая миниатюра",
        "Соедините мелодию и длинный бас в четырёх тактах. Левая переносит мизинец на до, фа, соль и обратно на до; правая остаётся в позиции до–соль. Разучите руки отдельно, затем сыграйте целиком.",
        "Первые музыкальные формы",
        80.0,
        [4, 4],
        3,
        final_notes,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_new_lesson_can_be_completed_with_its_written_notes() {
        use crate::{
            midi::{MidiEvent, PianoState},
            performance::Options,
            session::{Mode, Session},
        };
        for (id, song) in lessons() {
            song.validate().unwrap();
            assert!(
                song.notes
                    .iter()
                    .all(|n| n.finger > 0 && !n.hand.is_empty())
            );
            let mut session =
                Session::new(&song, Mode::Waiting, Options::default(), id.clone()).unwrap();
            let mut state = PianoState::default();
            let mut groups = 0;
            while let Some(group) = session.expected() {
                let pitches = group.pitches.clone();
                let start = group.start;
                for &p in &pitches {
                    let e = MidiEvent::parse(&[0x90, p, 88], "test").unwrap();
                    state.apply(&e);
                    session.feed(&e, &state.pressed(), start, true).unwrap();
                }
                for p in pitches {
                    let e = MidiEvent::parse(&[0x80, p, 0], "test").unwrap();
                    state.apply(&e);
                    session
                        .feed(&e, &state.pressed(), start + 0.01, true)
                        .unwrap();
                }
                groups += 1;
                assert!(groups <= song.notes.len());
            }
            let report = session.finish(true);
            assert!(report.passed(), "{id}: {report:?}");
        }
    }
}
