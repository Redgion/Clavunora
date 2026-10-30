// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Reproduce Python's dataclass JSON encoding for score-version hashes in shared reports.
use crate::score::Song;
use anyhow::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};
const SONG_FIELDS: [&str; 11] = [
    "title",
    "notes",
    "bpm",
    "time_signature",
    "track_names",
    "difficulty",
    "composer",
    "work",
    "description",
    "source",
    "tempo_map",
];
const NOTE_FIELDS: [&str; 8] = [
    "pitch", "start", "duration", "velocity", "track", "channel", "hand", "finger",
];
fn json(value: &Value) -> String {
    match value {
        Value::Number(n) if n.is_f64() => {
            let text = format!("{:?}", n.as_f64().unwrap());
            if let Some((mantissa, exp)) = text.split_once('e') {
                let exp: i32 = exp.parse().unwrap();
                format!(
                    "{}e{exp:+03}",
                    mantissa.strip_suffix(".0").unwrap_or(mantissa)
                )
            } else {
                text
            }
        }
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(json).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap(), json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => serde_json::to_string(value).expect("JSON value is serializable"),
    }
}
pub fn python_hash(raw: &str) -> Result<String> {
    let song = Song::from_json(raw)?;
    let original: Value = serde_json::from_str(raw)?;
    let defaults = serde_json::to_value(song)?;
    let mut fields = vec![];
    for key in &SONG_FIELDS {
        let value = original.get(*key).unwrap_or(&defaults[*key]);
        let encoded = if *key == "notes" {
            let mut notes = vec![];
            for (i, note) in value.as_array().unwrap().iter().enumerate() {
                let parts = NOTE_FIELDS
                    .iter()
                    .map(|field| {
                        format!(
                            "{}: {}",
                            serde_json::to_string(field).unwrap(),
                            json(note.get(*field).unwrap_or(&defaults["notes"][i][*field]))
                        )
                    })
                    .collect::<Vec<_>>();
                notes.push(format!("{{{}}}", parts.join(", ")));
            }
            format!("[{}]", notes.join(", "))
        } else {
            json(value)
        };
        fields.push(format!("{}: {}", serde_json::to_string(key)?, encoded));
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(format!("{{{}}}", fields.join(", ")).as_bytes())
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_python_catalog_and_exercise_hashes() {
        let expected: std::collections::BTreeMap<String, String> =
            serde_json::from_str(include_str!("../tests/fixtures/python-score-hashes.json"))
                .unwrap();
        for (id, raw) in crate::catalog::BUNDLED_SCORES {
            assert_eq!(
                python_hash(&crate::catalog::score_raw(raw).unwrap()).unwrap(),
                expected[*id],
                "{id}"
            );
        }
        let exercises: Value =
            serde_json::from_str(include_str!("../tests/fixtures/python-exercises.json")).unwrap();
        for (id, song) in exercises.as_object().unwrap() {
            assert_eq!(
                python_hash(&song.to_string()).unwrap(),
                expected[id],
                "{id}"
            );
        }
    }
}
