// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
use anyhow::Result;
use clavunora_core::backup::{self, Preview, Summary};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

pub enum Outcome {
    Created(PathBuf, Summary),
    Preview(Preview),
    Restored(PathBuf),
}
pub enum Task {
    Create {
        source: PathBuf,
        destination: Option<PathBuf>,
    },
    Preview(Option<PathBuf>),
    Restore {
        target: PathBuf,
        preview: Preview,
        safety: PathBuf,
    },
}
#[derive(Default)]
pub struct Controls {
    pub path: String,
    pub info: String,
    pub preview: Option<Preview>,
    pub job: Option<Receiver<std::result::Result<Option<Outcome>, String>>>,
}
fn choose(save: bool) -> Result<Option<PathBuf>> {
    crate::dialogs::choose(if save {
        crate::dialogs::Kind::BackupSave
    } else {
        crate::dialogs::Kind::BackupOpen
    })
}

pub fn start(task: Task) -> Receiver<std::result::Result<Option<Outcome>, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<Option<Outcome>> {
            Ok(Some(match task {
                Task::Create {
                    source,
                    destination,
                } => {
                    let mut destination = match destination {
                        Some(p) => p,
                        None => {
                            let Some(p) = choose(true)? else {
                                return Ok(None);
                            };
                            p
                        }
                    };
                    if destination.extension().is_none() {
                        destination.set_extension("clavunora-backup");
                    }
                    let summary = backup::create(&source, &destination)?;
                    Outcome::Created(destination, summary)
                }
                Task::Preview(path) => {
                    let path = match path {
                        Some(p) => p,
                        None => {
                            let Some(p) = choose(false)? else {
                                return Ok(None);
                            };
                            p
                        }
                    };
                    Outcome::Preview(backup::preview(&path)?)
                }
                Task::Restore {
                    target,
                    preview,
                    safety,
                } => Outcome::Restored(backup::restore(&target, preview, &safety)?),
            }))
        })();
        let _ = sender.send(result.map_err(|e| format!("{e:#}")));
    });
    receiver
}
